// The music player: one audio element for the whole app, so music keeps playing while you
// move between screens. Everything here is plain browser audio; the equalizer (Web Audio) is
// only switched on the first time it is used, so normal playback has no extra processing.
import { musicSource, musicArtUrl, musicResolve, musicStateLoad, musicStateSave, type MusicTrack } from './api';

export type Repeat = 'off' | 'all' | 'one';

export const EQ_BANDS = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
export const EQ_PRESETS: Record<string, number[]> = {
  Flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  'Bass boost': [6, 5, 4, 2, 0, 0, 0, 0, 0, 0],
  'Treble boost': [0, 0, 0, 0, 0, 1, 3, 5, 6, 6],
  Vocal: [-2, -2, -1, 1, 3, 4, 3, 1, 0, -1],
  Rock: [5, 4, 2, -1, -2, 1, 3, 4, 5, 5],
  Classical: [4, 3, 2, 1, -1, -1, 0, 2, 3, 4],
  Loudness: [6, 4, 0, 0, -1, 0, 0, -1, 4, 2],
  'Late night': [-3, -2, 0, 1, 2, 2, 1, 0, -2, -4],
};

const MAX_FAILS_IN_A_ROW = 3;

class Player {
  queue: MusicTrack[] = $state([]);
  /** Position in `order`. */
  pos = $state(-1);
  order: number[] = $state([]);
  playing = $state(false);
  loading = $state(false);
  time = $state(0);
  duration = $state(0);
  error = $state('');
  volume = $state(0.8);
  rate = $state(1);
  shuffle = $state(false);
  repeat: Repeat = $state('off');
  eq: number[] = $state(EQ_PRESETS.Flat.slice());
  eqPreset = $state('Flat');
  eqOn = $state(true);
  /** Quieter songs are left alone; songs whose file says they are loud (ReplayGain) are turned down to match. */
  level = $state(true);
  art: string | null = $state(null);
  sleepLeft = $state(0); // seconds; 0 = off

  private el: HTMLAudioElement | null = null;
  private ctx: AudioContext | null = null;
  private filters: BiquadFilterNode[] = [];
  private fails = 0;
  private token = 0;
  private sleepEnd = 0;
  private sleepTimer: ReturnType<typeof setInterval> | null = null;
  private saveTimer: ReturnType<typeof setTimeout> | null = null;
  private lastSave = 0;
  private ready = false;
  private resumeAt = 0;

  get index(): number { return this.pos >= 0 ? (this.order[this.pos] ?? -1) : -1; }
  get current(): MusicTrack | null { const i = this.index; return i >= 0 ? this.queue[i] ?? null : null; }

  private audio(): HTMLAudioElement {
    if (this.el) return this.el;
    const a = new Audio();
    a.preload = 'auto';
    // Needed so the equalizer can read the sound; the music address allows it.
    a.crossOrigin = 'anonymous';
    a.volume = this.volume * this.volume;
    a.playbackRate = this.rate;
    a.addEventListener('timeupdate', () => { this.time = a.currentTime; this.scheduleSave(false); });
    a.addEventListener('durationchange', () => { this.duration = isFinite(a.duration) ? a.duration : 0; });
    a.addEventListener('play', () => { this.playing = true; this.sync(); });
    a.addEventListener('pause', () => { this.playing = false; this.sync(); this.scheduleSave(true); });
    a.addEventListener('playing', () => { this.loading = false; this.fails = 0; });
    a.addEventListener('waiting', () => { this.loading = true; });
    a.addEventListener('ended', () => this.ended());
    a.addEventListener('error', () => this.failed());
    this.el = a;
    this.mediaSession();
    return a;
  }

  // ----- restoring what was playing -----
  async init(): Promise<void> {
    if (this.ready) return;
    this.ready = true;
    try {
      const s = JSON.parse(await musicStateLoad()) as Record<string, unknown>;
      if (typeof s.volume === 'number') this.volume = Math.min(1, Math.max(0, s.volume));
      if (typeof s.rate === 'number') this.rate = Math.min(2, Math.max(0.5, s.rate));
      if (typeof s.shuffle === 'boolean') this.shuffle = s.shuffle;
      if (s.repeat === 'off' || s.repeat === 'all' || s.repeat === 'one') this.repeat = s.repeat;
      if (typeof s.eqOn === 'boolean') this.eqOn = s.eqOn;
      if (typeof s.level === 'boolean') this.level = s.level;
      if (typeof s.eqPreset === 'string') this.eqPreset = s.eqPreset;
      if (Array.isArray(s.eq) && s.eq.length === EQ_BANDS.length && s.eq.every((n) => typeof n === 'number')) this.eq = (s.eq as number[]).map((n) => Math.min(12, Math.max(-12, n)));
      if (Array.isArray(s.queue) && s.queue.every((r) => typeof r === 'string')) {
        const tracks = await musicResolve(s.queue as string[]);
        if (tracks.length && this.queue.length === 0) {
          this.queue = tracks;
          const want = typeof s.current === 'string' ? tracks.findIndex((t) => t.rel === s.current) : 0;
          this.rebuildOrder(want >= 0 ? want : 0);
          this.resumeAt = typeof s.time === 'number' ? s.time : 0;
          await this.load(false);
        }
      }
    } catch {
      /* nothing remembered, or unreadable: start fresh */
    }
    this.applyVolume();
  }

  private scheduleSave(now: boolean) {
    const t = Date.now();
    if (!now && t - this.lastSave < 5000) return;
    if (this.saveTimer) clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => this.save(), now ? 300 : 800);
  }
  save() {
    this.lastSave = Date.now();
    const state = {
      version: 1, volume: this.volume, rate: this.rate, shuffle: this.shuffle, repeat: this.repeat,
      eqOn: this.eqOn, level: this.level, eqPreset: this.eqPreset, eq: this.eq,
      queue: this.queue.slice(0, 5000).map((t) => t.rel), current: this.current?.rel ?? '', time: Math.floor(this.time),
    };
    musicStateSave(JSON.stringify(state)).catch(() => {});
  }

  // ----- the queue -----
  private rebuildOrder(keep: number) {
    const n = this.queue.length;
    const idx = Array.from({ length: n }, (_, i) => i);
    if (this.shuffle && n > 1) {
      const rest = idx.filter((i) => i !== keep);
      for (let i = rest.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); [rest[i], rest[j]] = [rest[j], rest[i]]; }
      this.order = keep >= 0 ? [keep, ...rest] : rest;
      this.pos = keep >= 0 ? 0 : -1;
    } else {
      this.order = idx;
      this.pos = keep >= 0 && keep < n ? keep : -1;
    }
  }

  async playTracks(tracks: MusicTrack[], start = 0): Promise<void> {
    if (!tracks.length) return;
    this.queue = tracks.slice(0, 20000);
    this.rebuildOrder(Math.min(Math.max(start, 0), this.queue.length - 1));
    this.resumeAt = 0;
    await this.load(true);
  }
  enqueue(tracks: MusicTrack[]) {
    if (!tracks.length) return;
    const keep = this.index;
    this.queue = [...this.queue, ...tracks].slice(0, 20000);
    this.rebuildOrder(keep);
    if (keep < 0) { this.pos = 0; void this.load(false); }
  }
  playNext(tracks: MusicTrack[]) {
    if (!tracks.length) return;
    const keep = this.index;
    if (keep < 0) { void this.playTracks(tracks, 0); return; }
    const q = this.queue.slice();
    q.splice(keep + 1, 0, ...tracks);
    this.queue = q;
    // keep the current track, and put the new ones right after it whatever the shuffle order
    const n = q.length;
    const added = tracks.map((_, i) => keep + 1 + i);
    const rest = this.order.slice(this.pos + 1).map((i) => (i > keep ? i + tracks.length : i));
    const before = this.order.slice(0, this.pos + 1).map((i) => (i > keep ? i + tracks.length : i));
    this.order = [...before, ...added, ...rest.filter((i) => i < n)];
  }
  removeAt(i: number) {
    if (i < 0 || i >= this.queue.length) return;
    const cur = this.index;
    const q = this.queue.slice();
    q.splice(i, 1);
    this.queue = q;
    if (i === cur) {
      const next = Math.min(i, q.length - 1);
      this.rebuildOrder(next);
      if (q.length) void this.load(this.playing); else this.stop();
    } else {
      this.rebuildOrder(i < cur ? cur - 1 : cur);
    }
  }
  move(i: number, delta: number) {
    const j = i + delta;
    if (i < 0 || j < 0 || i >= this.queue.length || j >= this.queue.length) return;
    const cur = this.index;
    const q = this.queue.slice();
    [q[i], q[j]] = [q[j], q[i]];
    this.queue = q;
    this.rebuildOrder(cur === i ? j : cur === j ? i : cur);
  }
  clear() {
    this.stop();
    this.queue = [];
    this.order = [];
    this.pos = -1;
    this.scheduleSave(true);
  }
  async jump(i: number): Promise<void> {
    if (i < 0 || i >= this.queue.length) return;
    const p = this.order.indexOf(i);
    if (p >= 0) this.pos = p; else this.rebuildOrder(i);
    this.resumeAt = 0;
    await this.load(true);
  }

  // ----- playing -----
  private async load(autoplay: boolean): Promise<void> {
    const t = this.current;
    const a = this.audio();
    const mine = ++this.token;
    if (!t) { this.stop(); return; }
    this.error = '';
    this.loading = true;
    this.time = 0;
    this.duration = 0;
    this.art = null;
    try {
      const src = await musicSource(t.rel);
      if (mine !== this.token) return;
      a.src = src;
      a.playbackRate = this.rate;
      this.applyVolume();
      if (this.resumeAt > 0) {
        const at = this.resumeAt;
        this.resumeAt = 0;
        a.addEventListener('loadedmetadata', () => { try { a.currentTime = at; } catch { /* not seekable yet */ } }, { once: true });
      }
      void musicArtUrl(t.rel).then((u) => { if (mine === this.token) { this.art = u; this.sync(); } });
      this.sync();
      if (autoplay) await this.play();
      else this.loading = false;
    } catch (e) {
      if (mine !== this.token) return;
      this.error = typeof e === 'string' ? e : (e as Error)?.message ?? 'Could not play this file.';
      this.loading = false;
      this.skipAfterFailure();
    }
  }

  async play(): Promise<void> {
    const a = this.audio();
    if (!a.src && this.current) { await this.load(true); return; }
    const mine = this.token;
    try {
      if (this.ctx && this.ctx.state === 'suspended') await this.ctx.resume();
      await a.play();
    } catch (e) {
      const name = (e as DOMException)?.name;
      // Another song, or the same one from a converted copy, has been loaded since: this answer is out of date.
      if (name === 'AbortError' || mine !== this.token) return;
      // The built-in decoder refused the file: the error event (see failed()) says so.
      if (name === 'NotSupportedError') return;
      // A user-gesture rule or a bad file. Leave it paused; the error shows.
      this.error = 'Could not start playing this file.';
    }
  }
  pause() { this.el?.pause(); }
  toggle() { if (this.playing) this.pause(); else void this.play(); }
  stop() {
    this.token++;
    if (this.el) { this.el.pause(); this.el.removeAttribute('src'); this.el.load(); }
    this.playing = false; this.loading = false; this.time = 0; this.duration = 0; this.art = null;
    this.sync();
  }
  seek(t: number) {
    const a = this.audio();
    if (isFinite(t)) { a.currentTime = Math.max(0, Math.min(t, this.duration || t)); this.time = a.currentTime; }
  }
  skip(seconds: number) { this.seek(this.time + seconds); }

  async next(): Promise<void> {
    if (!this.queue.length) return;
    if (this.pos + 1 < this.order.length) { this.pos++; }
    else if (this.repeat === 'all') { this.rebuildOrder(this.shuffle ? -1 : 0); if (this.pos < 0) this.pos = 0; }
    else { this.pause(); this.seek(0); return; }
    await this.load(true);
  }
  async prev(): Promise<void> {
    if (!this.queue.length) return;
    if (this.time > 3 || this.pos <= 0) { this.seek(0); return; }
    this.pos--;
    await this.load(true);
  }
  private ended() {
    if (this.repeat === 'one') { this.seek(0); void this.play(); return; }
    if (this.pos + 1 >= this.order.length && this.repeat === 'off') { this.pause(); this.seek(0); return; }
    void this.next();
  }
  private failed() {
    const a = this.el;
    if (!a || !a.src) return;
    this.loading = false;
    this.error = `Could not play “${this.current?.title ?? 'this file'}”. If it is a WAV with an unusual format, saving it as an MP3 will fix it.`;
    this.skipAfterFailure();
  }
  private skipAfterFailure() {
    this.fails++;
    if (this.fails >= MAX_FAILS_IN_A_ROW || this.queue.length < 2) { this.fails = 0; this.playing = false; return; }
    if (this.pos + 1 < this.order.length || this.repeat === 'all') void this.next();
  }

  // ----- sound -----
  /** 1 for no change; below 1 for a song whose file asks to be turned down. Never louder than the file. */
  private levelFactor(): number {
    const g = this.current?.gain_db ?? 0;
    return this.level && g < 0 ? Math.pow(10, Math.max(g, -24) / 20) : 1;
  }
  private applyVolume() { if (this.el) this.el.volume = this.volume * this.volume * this.levelFactor(); }
  setLevel(on: boolean) { this.level = on; this.applyVolume(); this.scheduleSave(true); }
  setVolume(v: number) { this.volume = Math.min(1, Math.max(0, v)); this.applyVolume(); this.scheduleSave(true); }
  setRate(r: number) { this.rate = Math.min(2, Math.max(0.5, r)); if (this.el) this.el.playbackRate = this.rate; this.scheduleSave(true); }
  setShuffle(on: boolean) { this.shuffle = on; this.rebuildOrder(this.index); this.scheduleSave(true); }
  cycleRepeat() { this.repeat = this.repeat === 'off' ? 'all' : this.repeat === 'all' ? 'one' : 'off'; this.scheduleSave(true); }

  /** Build the equalizer the first time it is wanted. After that the audio always passes through it. */
  private ensureGraph(): boolean {
    if (this.ctx) return true;
    try {
      const a = this.audio();
      const Ctx = window.AudioContext ?? (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
      const ctx = new Ctx();
      const src = ctx.createMediaElementSource(a);
      let node: AudioNode = src;
      this.filters = EQ_BANDS.map((f, i) => {
        const b = ctx.createBiquadFilter();
        b.type = i === 0 ? 'lowshelf' : i === EQ_BANDS.length - 1 ? 'highshelf' : 'peaking';
        b.frequency.value = f;
        b.Q.value = 1.1;
        b.gain.value = 0;
        node.connect(b);
        node = b;
        return b;
      });
      node.connect(ctx.destination);
      this.ctx = ctx;
      if (this.playing) void ctx.resume();
      return true;
    } catch {
      return false;
    }
  }
  private applyEq() {
    const flat = !this.eqOn || this.eq.every((g) => g === 0);
    if (!this.ctx && flat) return; // nothing to do, and no graph needed
    if (!this.ensureGraph()) return;
    this.filters.forEach((f, i) => { f.gain.value = flat ? 0 : this.eq[i]; });
  }
  setBand(i: number, gain: number) {
    const e = this.eq.slice();
    e[i] = Math.min(12, Math.max(-12, Math.round(gain * 2) / 2));
    this.eq = e;
    this.eqPreset = 'Custom';
    this.applyEq();
    this.scheduleSave(true);
  }
  setPreset(name: string) {
    const p = EQ_PRESETS[name];
    if (!p) return;
    this.eq = p.slice();
    this.eqPreset = name;
    this.applyEq();
    this.scheduleSave(true);
  }
  setEqOn(on: boolean) { this.eqOn = on; this.applyEq(); this.scheduleSave(true); }

  // ----- sleep timer: fades out, then pauses -----
  setSleep(minutes: number) {
    if (this.sleepTimer) { clearInterval(this.sleepTimer); this.sleepTimer = null; }
    this.applyVolume();
    if (!minutes) { this.sleepLeft = 0; return; }
    this.sleepEnd = Date.now() + minutes * 60_000;
    this.sleepLeft = minutes * 60;
    this.sleepTimer = setInterval(() => {
      const left = Math.max(0, Math.round((this.sleepEnd - Date.now()) / 1000));
      this.sleepLeft = left;
      if (left <= 15 && this.el) this.el.volume = this.volume * this.volume * (left / 15);
      if (left <= 0) {
        clearInterval(this.sleepTimer!); this.sleepTimer = null;
        this.pause(); this.applyVolume();
      }
    }, 500);
  }

  // ----- lock screen, headphone buttons, media keys -----
  private sync() {
    const t = this.current;
    if (!('mediaSession' in navigator)) return;
    navigator.mediaSession.playbackState = this.playing ? 'playing' : 'paused';
    if (!t) { navigator.mediaSession.metadata = null; return; }
    try {
      navigator.mediaSession.metadata = new MediaMetadata({ title: t.title, artist: t.artist, album: t.album, artwork: this.art ? [{ src: this.art }] : [] });
    } catch { /* unsupported artwork */ }
  }
  private mediaSession() {
    if (!('mediaSession' in navigator)) return;
    const ms = navigator.mediaSession;
    const set = (a: MediaSessionAction, h: MediaSessionActionHandler) => { try { ms.setActionHandler(a, h); } catch { /* not supported here */ } };
    set('play', () => void this.play());
    set('pause', () => this.pause());
    set('previoustrack', () => void this.prev());
    set('nexttrack', () => void this.next());
    set('seekto', (d) => { if (typeof d.seekTime === 'number') this.seek(d.seekTime); });
    set('seekbackward', () => this.skip(-10));
    set('seekforward', () => this.skip(10));
  }
}

export const player = new Player();

export function fmtTime(s: number): string {
  if (!isFinite(s) || s < 0) s = 0;
  const m = Math.floor(s / 60), sec = Math.floor(s % 60);
  return `${m}:${sec.toString().padStart(2, '0')}`;
}
