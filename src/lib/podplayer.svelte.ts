// The podcast player: its own audio element, so an episode keeps playing while you move between
// screens, and it remembers where you left off in every episode (on the drive, via the host).
import { podcastSource, podcastsProgressLoad, podcastsProgressSave, type PodEpisode } from './api';
import { player as music } from './player.svelte';

export const SPEEDS = [0.75, 1, 1.25, 1.5, 1.75, 2];
const BACK = 15;
const FORWARD = 30;
/// Within this many seconds of the end counts as finished.
const FINISHED_WITHIN = 45;
const SAVE_EVERY_MS = 10_000;

class PodPlayer {
  queue: PodEpisode[] = $state([]);
  index = $state(-1);
  show = $state('');
  playing = $state(false);
  loading = $state(false);
  time = $state(0);
  duration = $state(0);
  rate = $state(1);
  error = $state('');
  /** Seconds and finished flag by episode path. */
  progress: Record<string, { seconds: number; done: boolean }> = $state({});

  private el: HTMLAudioElement | null = null;
  private token = 0;
  private lastSave = 0;
  private loaded = false;

  get current(): PodEpisode | null { return this.index >= 0 ? this.queue[this.index] ?? null : null; }

  /** Read the saved places once. Safe to call again. */
  async init(): Promise<void> {
    if (this.loaded) return;
    this.loaded = true;
    try {
      const list = await podcastsProgressLoad();
      const next: Record<string, { seconds: number; done: boolean }> = {};
      for (const p of list) next[p.rel] = { seconds: p.seconds, done: p.done };
      this.progress = next;
    } catch { /* guests and the browser preview have none */ }
  }

  private audio(): HTMLAudioElement {
    if (this.el) return this.el;
    const a = new Audio();
    a.preload = 'metadata';
    a.addEventListener('play', () => { this.playing = true; });
    a.addEventListener('pause', () => { this.playing = false; this.save(true); });
    a.addEventListener('timeupdate', () => { this.time = a.currentTime; this.save(false); });
    a.addEventListener('durationchange', () => { this.duration = isFinite(a.duration) ? a.duration : 0; });
    a.addEventListener('waiting', () => { this.loading = true; });
    a.addEventListener('playing', () => { this.loading = false; this.error = ''; });
    a.addEventListener('ended', () => { this.finish(); });
    a.addEventListener('error', () => {
      this.loading = false;
      this.playing = false;
      this.error = 'This episode could not be played. The file may be damaged.';
    });
    this.el = a;
    return a;
  }

  /** Start an episode from a list (the show's episodes, newest first). Continues where it was left off. */
  async open(episodes: PodEpisode[], index: number, show: string): Promise<void> {
    await this.init();
    this.save(true);
    this.queue = episodes;
    this.index = index;
    this.show = show;
    await this.load();
  }

  private async load(): Promise<void> {
    const ep = this.current;
    if (!ep) return;
    const mine = ++this.token;
    this.error = '';
    this.loading = true;
    this.time = 0;
    this.duration = 0;
    const a = this.audio();
    try {
      const src = await podcastSource(ep.rel);
      if (mine !== this.token) return;
      a.src = src;
      a.playbackRate = this.rate;
      const saved = this.progress[ep.rel];
      // A finished episode starts again from the top.
      const from = saved && !saved.done ? saved.seconds : 0;
      if (from > 2) {
        const seekWhenReady = () => { a.currentTime = from; a.removeEventListener('loadedmetadata', seekWhenReady); };
        a.addEventListener('loadedmetadata', seekWhenReady);
      }
      music.pause();
      await a.play();
    } catch (e) {
      const name = (e as DOMException)?.name;
      if (name === 'AbortError' || mine !== this.token) return;
      this.loading = false;
      this.error = 'This episode could not be played.';
    }
  }

  async play(): Promise<void> {
    const a = this.audio();
    if (!a.src && this.current) { await this.load(); return; }
    music.pause();
    try { await a.play(); } catch { this.error = 'This episode could not be played.'; }
  }
  pause() { this.el?.pause(); }
  toggle() { if (this.playing) this.pause(); else void this.play(); }

  seek(s: number) {
    const a = this.audio();
    a.currentTime = Math.max(0, Math.min(s, this.duration || s));
    this.time = a.currentTime;
    this.save(true);
  }
  back() { this.seek(this.time - BACK); }
  forward() { this.seek(this.time + FORWARD); }
  setRate(r: number) {
    this.rate = r;
    if (this.el) this.el.playbackRate = r;
  }

  /** Plays the next (older) episode of the show, if there is one. */
  async next(): Promise<void> {
    if (this.index + 1 < this.queue.length) { this.save(true); this.index++; await this.load(); }
  }
  async prev(): Promise<void> {
    if (this.time > 5) { this.seek(0); return; }
    if (this.index > 0) { this.save(true); this.index--; await this.load(); }
  }

  /** The episode reached its end: mark it finished and go on to the next one. */
  private finish() {
    const ep = this.current;
    if (ep) this.remember(ep.rel, 0, true);
    if (this.index + 1 < this.queue.length) void this.next();
    else this.playing = false;
  }

  markDone(rel: string, done: boolean) { this.remember(rel, 0, done); }

  private remember(rel: string, seconds: number, done: boolean) {
    this.progress[rel] = { seconds, done };
    void podcastsProgressSave(rel, Math.max(0, Math.floor(seconds)), done).catch(() => {});
  }

  private save(force: boolean) {
    const ep = this.current;
    const a = this.el;
    if (!ep || !a || !this.duration) return;
    const now = Date.now();
    if (!force && now - this.lastSave < SAVE_EVERY_MS) return;
    this.lastSave = now;
    const nearEnd = this.duration - a.currentTime <= FINISHED_WITHIN;
    if (nearEnd && a.currentTime > 60) this.remember(ep.rel, 0, true);
    else this.remember(ep.rel, a.currentTime, false);
  }

  /** Stop and let go of the file. */
  stop() {
    this.save(true);
    this.token++;
    if (this.el) { this.el.pause(); this.el.removeAttribute('src'); this.el.load(); }
    this.queue = [];
    this.index = -1;
    this.playing = false;
    this.time = 0;
    this.duration = 0;
  }
}

export const podPlayer = new PodPlayer();

export function fmtLong(s: number): string {
  if (!isFinite(s) || s < 0) s = 0;
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = Math.floor(s % 60);
  return h > 0 ? `${h}:${m.toString().padStart(2, '0')}:${sec.toString().padStart(2, '0')}` : `${m}:${sec.toString().padStart(2, '0')}`;
}
