// The program's settings. One list, with each setting's default and its allowed values, kept in
// `user/settings.json` on the drive. The Settings screen edits it; screens read from `settings.values`.
// The few settings the Rust side acts on (what the assistant searches, the library server, the
// stock thresholds, the music scan) are read by it from the same file, so the defaults here must
// match src-tauri/src/settings.rs and the calls in src-tauri/src/lib.rs.
import { settingsLoad, settingsSave } from './api';

export const SCREENS = ['Dashboard', 'Ask', 'Library', 'Intel', 'Academy', 'Maps', 'Music', 'Plan', 'Inventory'] as const;
export type Screen = (typeof SCREENS)[number];

export type Settings = {
  // General
  uiScale: number;
  startScreen: Screen;
  /** The Dashboard became the opening screen; this remembers that the switch was done once. */
  dashboardDefaultDone: boolean;
  showStatusBar: boolean;
  // Assistant
  searchBooks: boolean;
  searchArchives: boolean;
  searchFiles: boolean;
  searchSavedSites: boolean;
  answerSources: number;
  answerLength: 'short' | 'normal' | 'long';
  searchSpeed: 'quick' | 'normal' | 'thorough';
  showSourceChips: boolean;
  extraFolders: string[];
  // Library
  autoScanShelf: boolean;
  autoRestartLibrary: boolean;
  // Intel
  intelStart: 'archives' | 'first';
  intelReader: 'server' | 'builtin';
  intelRestoreTabs: boolean;
  // Music
  showMiniPlayer: boolean;
  scanMusicOnStart: boolean;
  // Maps
  mapUnits: 'imperial' | 'metric';
  // Plan
  planActDays: number;
  planCheckDays: number;
  planExpiryMonths: number;
};

export const DEFAULTS: Settings = {
  uiScale: 100,
  startScreen: 'Dashboard',
  dashboardDefaultDone: false,
  showStatusBar: true,
  searchBooks: true,
  searchArchives: true,
  searchFiles: true,
  searchSavedSites: true,
  answerSources: 3,
  answerLength: 'normal',
  searchSpeed: 'normal',
  showSourceChips: true,
  extraFolders: [],
  autoScanShelf: true,
  autoRestartLibrary: true,
  intelStart: 'archives',
  intelReader: 'server',
  intelRestoreTabs: false,
  showMiniPlayer: true,
  scanMusicOnStart: true,
  mapUnits: 'imperial',
  planActDays: 7,
  planCheckDays: 14,
  planExpiryMonths: 3,
};

/** Allowed range for each number. */
export const RANGES: Record<string, [number, number]> = {
  uiScale: [85, 150],
  answerSources: [1, 6],
  planActDays: [1, 60],
  planCheckDays: [2, 120],
  planExpiryMonths: [0, 24],
};

/** Allowed words for each choice. */
export const CHOICES: Record<string, readonly string[]> = {
  startScreen: SCREENS,
  answerLength: ['short', 'normal', 'long'],
  searchSpeed: ['quick', 'normal', 'thorough'],
  intelStart: ['archives', 'first'],
  intelReader: ['server', 'builtin'],
  mapUnits: ['imperial', 'metric'],
};

/** Turn whatever was in the file into a complete, valid list of settings. Anything missing or
 *  of the wrong kind gets its default. */
export function sanitize(raw: unknown): Settings {
  const out: Record<string, unknown> = {};
  const r = raw && typeof raw === 'object' ? { ...(raw as Record<string, unknown>) } : {};
  // Intel was called Ghostnet at first: carry over what was already saved under the old name.
  for (const [oldKey, newKey] of [['ghostnetStart', 'intelStart'], ['ghostnetRestoreTabs', 'intelRestoreTabs']]) {
    if (r[newKey] === undefined && r[oldKey] !== undefined) r[newKey] = r[oldKey];
  }
  if (r.startScreen === 'Ghostnet') r.startScreen = 'Intel';
  for (const key of Object.keys(DEFAULTS)) {
    const d = (DEFAULTS as Record<string, unknown>)[key];
    const v = r[key];
    if (Array.isArray(d)) {
      out[key] = Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string').map((x) => x.trim()).filter((x) => x !== '').slice(0, 30) : [];
    } else if (typeof d === 'boolean') {
      out[key] = typeof v === 'boolean' ? v : d;
    } else if (typeof d === 'number') {
      const [lo, hi] = RANGES[key] ?? [Number.MIN_SAFE_INTEGER, Number.MAX_SAFE_INTEGER];
      out[key] = typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, Math.round(v))) : d;
    } else {
      out[key] = typeof v === 'string' && (CHOICES[key] ?? []).includes(v) ? v : d;
    }
  }
  // "check" never comes before "act now"
  if ((out.planCheckDays as number) < (out.planActDays as number)) out.planCheckDays = out.planActDays;
  return out as Settings;
}

class Store {
  values: Settings = $state(sanitize(null));
  loaded = $state(false);
  saveError = $state('');
  private timer: ReturnType<typeof setTimeout> | undefined;

  async load(): Promise<void> {
    try {
      this.values = sanitize(JSON.parse(await settingsLoad()));
    } catch {
      this.values = sanitize(null);
    }
    this.loaded = true;
  }

  set<K extends keyof Settings>(key: K, value: Settings[K]): void {
    this.values = sanitize({ ...this.values, [key]: value });
    this.queueSave();
  }

  /** Put these settings (or all of them) back to what the program ships with. */
  reset(keys?: (keyof Settings)[]): void {
    const next: Record<string, unknown> = { ...this.values };
    for (const k of keys ?? (Object.keys(DEFAULTS) as (keyof Settings)[])) {
      const d = DEFAULTS[k];
      next[k] = Array.isArray(d) ? [] : d;
    }
    this.values = sanitize(next);
    this.queueSave();
  }

  /** Saved a moment after the last change, so dragging a slider does not write a file per step. */
  private queueSave(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      settingsSave(JSON.stringify(this.values)).then(
        () => { this.saveError = ''; },
        (e) => { this.saveError = 'The settings could not be saved: ' + String(e); },
      );
    }, 400);
  }
}

export const settings = new Store();

/** Make the whole interface bigger or smaller. Uses the web view's own zoom in the desktop app
 *  (layout stays correct), and the page's zoom in a plain browser. */
export async function applyUiScale(percent: number): Promise<void> {
  const f = percent / 100;
  try {
    if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
      const { getCurrentWebview } = await import('@tauri-apps/api/webview');
      await getCurrentWebview().setZoom(f);
      document.documentElement.style.removeProperty('zoom');
      return;
    }
  } catch { /* fall through to the page zoom */ }
  if (typeof document !== 'undefined') document.documentElement.style.setProperty('zoom', String(f));
}
