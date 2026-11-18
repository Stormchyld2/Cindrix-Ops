// The video player: one video element for the whole program (shown by VideoPlayer.svelte), so a video keeps
// playing while you move between screens, and remembers where you left off in every video (on the drive,
// through the host; guests have no saved places).
import { videoSource, videosProgressLoad, videosProgressSave } from './api';
import { player as music } from './player.svelte';
import { podPlayer } from './podplayer.svelte';

/// Within this many seconds of the end counts as finished.
const FINISHED_WITHIN = 30;
const SAVE_EVERY_MS = 5_000;

export type VideoRef = { rel: string; title: string; /** name of the playable copy, when the file was converted */ converted?: string | null };

class VideoPlayer {
  current: VideoRef | null = $state(null);
  src = $state('');
  /** True: the big window. False: the small one (still playing). */
  big = $state(true);
  error = $state('');
  playing = $state(false);
  /** Seconds and finished flag by video path. */
  progress: Record<string, { seconds: number; done: boolean }> = $state({});

  private el: HTMLVideoElement | null = null;
  private lastSave = 0;
  private loaded = false;
  private token = 0;

  /** Read the saved places once. Safe to call again. */
  async init(): Promise<void> {
    if (this.loaded) return;
    this.loaded = true;
    try {
      const list = await videosProgressLoad();
      const next: Record<string, { seconds: number; done: boolean }> = {};
      for (const p of list) next[p.rel] = { seconds: p.seconds, done: p.done };
      this.progress = next;
    } catch { /* guests and the browser preview have none */ }
  }

  /** The video element registers itself here when it is on the page. */
  attach(el: HTMLVideoElement | null): void { this.el = el; }

  async play(v: VideoRef): Promise<void> {
    void this.init();
    this.save(true);
    const my = ++this.token;
    this.error = '';
    // only one thing makes sound at a time
    music.pause();
    podPlayer.pause();
    const src = v.converted ? await videoSource(v.converted, true) : await videoSource(v.rel);
    if (my !== this.token) return;
    this.current = v;
    this.src = src;
    this.big = true;
  }

  /** The element has its first data: pick up where it was left off, unless it was finished, then play. */
  loadedMeta(): void {
    const el = this.el;
    const cur = this.current;
    if (!el || !cur) return;
    const p = this.progress[cur.rel];
    if (p && !p.done && p.seconds > 5 && p.seconds < (el.duration || 0) - 10) el.currentTime = p.seconds;
    void el.play().catch(() => { /* needs a click: the controls are there */ });
  }

  failed(): void {
    this.playing = false;
    this.error = 'This video could not be played. It may be damaged, or not a kind of MP4 the player can read.';
  }

  onTime(): void {
    if (Date.now() - this.lastSave > SAVE_EVERY_MS) this.save(false);
  }

  /** Remember the place (or that it was finished). `force` saves even if it was saved a moment ago. */
  save(finished: boolean, force = true): void {
    const el = this.el;
    const cur = this.current;
    if (!el || !cur) return;
    const secs = Math.max(0, Math.floor(el.currentTime));
    const len = Math.floor(el.duration || 0);
    const done = finished || (len > 0 && secs + FINISHED_WITHIN >= len);
    if (!force && Date.now() - this.lastSave < SAVE_EVERY_MS) return;
    this.progress[cur.rel] = { seconds: done ? 0 : secs, done };
    this.lastSave = Date.now();
    void videosProgressSave(cur.rel, done ? 0 : secs, done).catch(() => {});
  }

  markDone(rel: string, done: boolean): void {
    this.progress[rel] = { seconds: 0, done };
    void videosProgressSave(rel, 0, done).catch(() => {});
  }

  pause(): void { this.el?.pause(); }

  close(): void {
    this.save(false);
    this.token++;
    this.el?.pause();
    this.current = null;
    this.src = '';
    this.playing = false;
    this.error = '';
  }
}

export const videoPlayer = new VideoPlayer();
