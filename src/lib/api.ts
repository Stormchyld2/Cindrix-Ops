// Talks to the Rust core. In a plain browser (no Tauri) it falls back to in-memory samples,
// so the screens can be previewed and screenshotted.

export type Card = { id: string; title: string; kind: string; status: string; reviewer: string; reviewed: string; body: string };
export type Reply = { kind: 'Refused' | 'NotFound' | 'Answered' | 'Replaced'; text: string; sources: string[]; card: string | null };
export type Item = {
  id: string; category: string; name: string; location: string; quantity: number; unit: string;
  use_per_day: number; use_by: string; notes: string;
  /** The extra questions. All are optional text or numbers; an old item simply has them empty. */
  brand?: string; size?: string; for_whom?: string; bought?: string; target?: number; condition?: string;
  checked?: string; cost?: number; source?: string; serial?: string;
  status?: string; days_left?: number | null;
};
export type Need = { category: string; name: string; unit: string; amount: number; why: string; basis: string };
export type Cascade = {
  runs_out: { name: string; location: string; days: number }[];
  expiring: { name: string; months: number }[];
  water_days: number | null; food_days: number | null;
};
export type Suggestion = { level: 'urgent' | 'soon' | 'next' | 'good'; title: string; text: string; tab: string | null };
export type DashboardFacts = { maps: number; models: number; assistant_ready: boolean; archives: number; drills_done: number; drills_total: number };
export async function dashboardSuggestions(f: DashboardFacts): Promise<Suggestion[]> {
  if (inTauri) return call<Suggestion[]>('dashboard_suggestions', { maps: f.maps, models: f.models, assistantReady: f.assistant_ready, archives: f.archives, drillsDone: f.drills_done, drillsTotal: f.drills_total });
  return [
    { level: 'soon', title: 'Water lasts 9.0 days', text: 'A common goal is two weeks at home (for water, about a gallon per person per day).', tab: 'Inventory' },
    { level: 'next', title: 'Run your first drill', text: 'A drill finds the gaps while everything still works.', tab: 'Drills' },
  ];
}
export type FamilyStatus = { exists: boolean; locked: boolean };
export type Person = { name: string; conditions: string; medicines: string; allergies: string; doctor: string };
export type Family = { people: Person[]; contacts: string; plan: string };

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

// ---------- browser preview data ----------
const sampleItems: Item[] = [
  { id: '1', category: 'Water', name: 'Drinking water, 1 gal jugs', location: 'Pantry', quantity: 6, unit: 'gal', use_per_day: 2, use_by: '2027-11', notes: '' },
  { id: '2', category: 'Water', name: 'Drinking water, 1 gal jugs', location: 'Garage', quantity: 3, unit: 'gal', use_per_day: 0, use_by: '2027-03', notes: 'Rotate first' },
  { id: '3', category: 'Medical and diabetes', name: 'Test strips (Person 1)', location: 'Pantry', quantity: 120, unit: 'strips', use_per_day: 4, use_by: '2027-01', notes: '' },
  { id: '4', category: 'Medical and diabetes', name: 'Test strips (Person 2)', location: 'Pantry', quantity: 60, unit: 'strips', use_per_day: 4, use_by: '2026-12', notes: '' },
  { id: '5', category: 'Food', name: 'Canned and dry food', location: 'Pantry', quantity: 42, unit: 'meals', use_per_day: 6, use_by: '2027-03', notes: '' },
  { id: '6', category: 'Power and light', name: 'AA batteries', location: 'Basement', quantity: 24, unit: 'pcs', use_per_day: 0, use_by: '2030-01', notes: '' },
];
let memItems: Item[] = sampleItems.map((i) => ({ ...i }));
let memFamily: Family = { people: [{ name: 'Person 1', conditions: '', medicines: '', allergies: '', doctor: '' }], contacts: '', plan: '' };

function decorate(i: Item): Item {
  const d = i.use_per_day > 0 ? i.quantity / i.use_per_day : null;
  const status = d !== null && d < 7 ? 'act' : d !== null && d < 14 ? 'check' : 'ok';
  return { ...i, days_left: d, status };
}

// ---------- your own cards and plans ----------
export type OwnFolder = { key: 'cards' | 'plans' | 'seed-guides' | 'drills' | 'academy'; title: string; help: string; path: string; files: number; problems: { place: string; message: string }[] };
export async function ownContent(): Promise<OwnFolder[]> {
  if (inTauri) return call<OwnFolder[]>('own_content');
  return [
    { key: 'cards', title: 'Emergency cards', help: 'One text file per card.', path: 'user\\cards', files: 0, problems: [] },
    { key: 'plans', title: 'Plans and walkthroughs (including “keep it from failing”)', help: 'One text file per plan.', path: 'user\\plans', files: 0, problems: [] },
  { key: 'drills', title: 'Drills', help: 'One text file per drill.', path: 'user\\drills', files: 0, problems: [] },
  { key: 'academy', title: 'Academy courses (and a course made from a book)', help: 'A course is a folder.', path: 'user\\academy', files: 0, problems: [] },
  { key: 'seed-guides', title: 'Seed vault guides', help: 'One text file per guide.', path: 'user\\seed-guides', files: 0, problems: [] },
  ];
}
export async function openOwnFolder(key: string): Promise<string> {
  if (inTauri) return call<string>('open_own_folder', { key });
  return 'user\\' + key;
}

// ---------- calls ----------
export async function listCards(): Promise<Card[]> {
  if (inTauri) return call<Card[]>('list_cards');
  return [
    { id: 'low-blood-sugar', title: 'Low blood sugar', kind: 'emergency', status: 'draft', reviewer: '', reviewed: '',
      body: 'DRAFT. Sample text.\n\n1. If the person can swallow safely, give about 15 grams of fast-acting sugar.\n2. Wait 15 minutes and recheck.\n3. If confused or unconscious, call 911.' },
    { id: 'boil-water', title: 'Make water safe to drink', kind: 'emergency', status: 'draft', reviewer: '', reviewed: '', body: 'DRAFT. Sample text.' },
  ];
}

export type ModelStatus = { state: 'none' | 'loading' | 'ready' | 'error'; model: string; size: string; message: string; ram_gb: number };
export type ModelChoice = { file: string; size: string; recommended: boolean };
type StreamEvent = { replace: boolean; text: string };

/** Ask a question. onText is called as the answer arrives: replace=true means swap everything shown. */
export async function ask(question: string, onText: (text: string, replace: boolean) => void): Promise<Reply> {
  if (inTauri) {
    const { Channel } = await import('@tauri-apps/api/core');
    const ch = new Channel<StreamEvent>();
    ch.onmessage = (e) => onText(e.text, e.replace);
    return call<Reply>('ask_stream', { question, onEvent: ch });
  }
  const text = 'Browser preview only. Run the desktop app to ask the library.';
  onText(text, true);
  return { kind: 'NotFound', text, sources: [], card: null };
}

export async function modelStatus(): Promise<ModelStatus> {
  if (inTauri) return call<ModelStatus>('model_status');
  return { state: 'none', model: '', size: '', message: 'Browser preview: no model.', ram_gb: 0 };
}

export async function listModels(): Promise<ModelChoice[]> {
  if (inTauri) return call<ModelChoice[]>('list_models');
  return [];
}

export async function selectModel(file: string): Promise<void> {
  if (inTauri) await call<void>('select_model', { file });
}

export async function inventoryList(): Promise<Item[]> {
  if (inTauri) return call<Item[]>('inventory_list');
  return memItems.map(decorate);
}

export async function inventorySave(item: Item): Promise<Item[]> {
  if (inTauri) return call<Item[]>('inventory_save_item', { item });
  if (!item.id) item = { ...item, id: String(Math.max(0, ...memItems.map((i) => Number(i.id) || 0)) + 1) };
  const at = memItems.findIndex((i) => i.id === item.id);
  if (at >= 0) memItems[at] = item; else memItems.push(item);
  return memItems.map(decorate);
}

export async function inventoryDelete(ids: string[]): Promise<Item[]> {
  if (inTauri) return call<Item[]>('inventory_delete', { ids });
  memItems = memItems.filter((i) => !ids.includes(i.id));
  return memItems.map(decorate);
}

export async function whatFailsNext(): Promise<Cascade> {
  if (inTauri) return call<Cascade>('what_fails_next');
  const items = memItems;
  const groups = new Map<string, { q: number; per: number; places: Set<string>; cat: string }>();
  for (const i of items) {
    const g = groups.get(i.name) ?? { q: 0, per: 0, places: new Set<string>(), cat: i.category };
    g.q += i.quantity; g.per = Math.max(g.per, i.use_per_day); g.places.add(i.location);
    groups.set(i.name, g);
  }
  const used = [...groups].filter(([, g]) => g.per > 0);
  const runs_out = used.map(([name, g]) => ({ name, location: g.places.size > 1 ? 'several places' : [...g.places][0], days: g.q / g.per }))
    .sort((a, b) => a.days - b.days);
  const days = (cat: string) => {
    const d = used.filter(([, g]) => g.cat === cat).map(([, g]) => g.q / g.per);
    return d.length ? Math.min(...d) : null;
  };
  return { runs_out, expiring: [{ name: 'Test strips (Person 2)', months: 2 }], water_days: days('Water'), food_days: days('Food') };
}

export async function familyStatus(): Promise<FamilyStatus> {
  if (inTauri) return call<FamilyStatus>('family_status');
  return { exists: true, locked: false };
}

/** Rejects with the message "locked" when a passphrase is needed. */
export async function familyLoad(passphrase?: string): Promise<Family> {
  if (inTauri) {
    const text = await call<string>('family_load', { passphrase: passphrase ?? null });
    const parsed = JSON.parse(text) as Partial<Family>;
    return { people: parsed.people ?? [], contacts: parsed.contacts ?? '', plan: parsed.plan ?? '' };
  }
  return memFamily;
}

export async function familySave(family: Family, passphrase?: string): Promise<void> {
  if (inTauri) {
    await call<void>('family_save', { json: JSON.stringify(family, null, 2), passphrase: passphrase ?? null });
    return;
  }
  memFamily = family;
}

/** Save a readable copy as a plain text file in user/exports. Returns where it was saved. */
export async function exportText(name: string, text: string): Promise<string> {
  if (inTauri) return call<string>('export_text', { name, text });
  return 'user\\exports\\' + name;
}

// ---------- bookshelf ----------
export type Book = {
  id: string; title: string; author: string; category: string; kind: 'pdf' | 'md' | 'txt'; added: string; license: string;
  mine: boolean; searchable: boolean; last_page: number;
};
export type Mark = { kind: 'bookmark' | 'highlight' | 'note'; page: number; text: string };

const sampleText =
  '# Emergency toilet when the water is off\n\nDo not use a toilet that has no running water for long. Line a bucket or the toilet bowl with a heavy plastic bag. After each use, cover the waste with a scoop of cat litter, sawdust, ash or dry soil to cut smell and flies.\n\nTie the bag shut when it is about half full and keep it in a lidded container away from where you live and cook.\n\n# Handwashing without a tap\n\nWash hands with soap and clean water for 20 seconds after using the toilet, before cooking and before eating.';
const memBooks: { meta: Book; bytes: Uint8Array }[] = [
  { meta: { id: 'b-sample', title: 'Emergency Sanitation (sample)', author: 'Cindrix original', category: 'Food and water', kind: 'md', added: '', license: 'Cindrix original, draft', mine: false, searchable: true, last_page: 1 },
    bytes: new TextEncoder().encode(sampleText) },
];
const memMarks: ({ book: string } & Mark)[] = [];

export async function shelfList(): Promise<Book[]> {
  if (inTauri) return call<Book[]>('shelf_list');
  return memBooks.map((b) => b.meta);
}

export type DriveScan = { added: string[]; known: number; problems: string[]; folders: string[] };

/** Put the books found on the drive onto the shelf (each is added once). */
export async function shelfScanDrive(): Promise<DriveScan> {
  if (inTauri) return call<DriveScan>('shelf_scan_drive');
  return { added: [], known: 0, problems: [], folders: [] };
}

/** Add a book from a file the person picked. Throws a readable message if it is refused. */
export async function shelfAdd(file: File, category = ''): Promise<Book> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  if (inTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<Book>('shelf_add', bytes, { headers: { 'x-file-name': encodeURIComponent(file.name), 'x-category': encodeURIComponent(category) } });
  }
  const lower = file.name.toLowerCase();
  const kind = lower.endsWith('.pdf') ? 'pdf' : lower.endsWith('.md') ? 'md' : lower.endsWith('.txt') ? 'txt' : null;
  if (!kind) throw 'Only PDF, Markdown (.md) and text (.txt) books can be added.';
  if (kind === 'pdf' && String.fromCharCode(...bytes.slice(0, 5)) !== '%PDF-') throw 'That file says it is a PDF but does not look like one.';
  let title = file.name.replace(/\.[^.]+$/, '').replace(/[_-]+/g, ' ').replace(/\s+/g, ' ').trim() || 'Untitled';
  if (title === title.toLowerCase()) title = title.replace(/\b\w/g, (c) => c.toUpperCase());
  const meta: Book = { id: 'u' + (memBooks.length + 1), title, author: '', category: category || 'Mine', kind, added: new Date().toISOString().slice(0, 10), license: 'Added by you', mine: true, searchable: false, last_page: 1 };
  memBooks.push({ meta, bytes });
  return meta;
}

export async function shelfRead(id: string): Promise<Uint8Array> {
  if (inTauri) return new Uint8Array(await call<ArrayBuffer>('shelf_read', { id }));
  const b = memBooks.find((x) => x.meta.id === id);
  if (!b) throw 'No such book.';
  return b.bytes.slice();
}

export async function shelfUpdate(id: string, title: string, author: string, category: string): Promise<void> {
  if (inTauri) return call<void>('shelf_update', { id, title, author, category });
  const b = memBooks.find((x) => x.meta.id === id);
  if (b) { b.meta.title = title || b.meta.title; b.meta.author = author; b.meta.category = category || b.meta.category; }
}

export async function shelfRemove(id: string): Promise<void> {
  if (inTauri) return call<void>('shelf_remove', { id });
  const i = memBooks.findIndex((x) => x.meta.id === id);
  if (i >= 0) memBooks.splice(i, 1);
}

export async function shelfSaveText(id: string, pages: string[]): Promise<void> {
  if (inTauri) return call<void>('shelf_save_text', { id, pages });
  const b = memBooks.find((x) => x.meta.id === id);
  if (b) b.meta.searchable = true;
}

export async function marksList(book: string): Promise<Mark[]> {
  if (inTauri) return call<Mark[]>('marks_list', { book });
  return memMarks.filter((m) => m.book === book).map(({ kind, page, text }) => ({ kind, page, text }));
}
export async function marksAdd(book: string, m: Mark): Promise<void> {
  if (inTauri) return call<void>('marks_add', { book, ...m });
  if (!(m.kind === 'bookmark' && memMarks.some((x) => x.book === book && x.kind === 'bookmark' && x.page === m.page))) memMarks.push({ book, ...m });
}
export async function marksRemove(book: string, m: Mark): Promise<void> {
  if (inTauri) return call<void>('marks_remove', { book, ...m });
  const i = memMarks.findIndex((x) => x.book === book && x.kind === m.kind && x.page === m.page && x.text === m.text);
  if (i >= 0) memMarks.splice(i, 1);
}
export async function marksSetLast(book: string, page: number): Promise<void> {
  if (inTauri) return call<void>('marks_set_last', { book, page });
  const b = memBooks.find((x) => x.meta.id === book);
  if (b) b.meta.last_page = page;
}

// ---------- archives (ZIM files) and sharing ----------
export type ArchiveInfo = { file: string; mb: number; articles: number; saved_site: boolean; note: string; library: string; removable: boolean };
export type DriveInfo = { name: string; edition: string; path: string };
export type LibraryStatus = {
  archives: ArchiveInfo[];
  ready: boolean;
  url: string | null;
  shared: boolean;
  lan_url: string | null;
  drive: DriveInfo | null;
  folder: string;
  online: boolean;
};

export async function libraryStatus(): Promise<LibraryStatus> {
  if (inTauri) return call<LibraryStatus>('library_status');
  // Browser preview: pretend the library is ready so the reader screen can be shown with its sample archive.
  return { archives: [], ready: true, url: 'preview', shared: false, lan_url: null, drive: null, folder: '', online: false };
}

export async function libraryShare(enable: boolean): Promise<void> {
  if (inTauri) await call<void>('library_share', { enable });
}

// ---------- saved maps ----------
export type MapMeta = { id: string; name: string; has_picture: boolean; has_route: boolean };
export type LatLon = [number, number];
export type RouteStats = { km: number; miles: number };

let memMaps: { id: string; json: string; notes: string; name: string }[] = [];

function haversineKm(a: LatLon, b: LatLon): number {
  const r = (d: number) => (d * Math.PI) / 180;
  const h = Math.sin(r(b[0] - a[0]) / 2) ** 2 + Math.cos(r(a[0])) * Math.cos(r(b[0])) * Math.sin(r(b[1] - a[1]) / 2) ** 2;
  return 2 * 6371.0088 * Math.asin(Math.sqrt(h));
}

export async function mapsList(): Promise<MapMeta[]> {
  if (inTauri) return call<MapMeta[]>('maps_list');
  return memMaps.map((m) => ({ id: m.id, name: m.name, has_picture: false, has_route: false }));
}
export async function mapLoad(id: string): Promise<{ json: string; notes: string }> {
  if (inTauri) return call<{ json: string; notes: string }>('map_load', { id });
  const m = memMaps.find((x) => x.id === id);
  if (!m) throw 'There is no such saved map.';
  return { json: m.json, notes: m.notes };
}
export async function mapSave(id: string | null, name: string, json: string, notes: string): Promise<string> {
  if (inTauri) return call<string>('map_save', { id, name, json, notes });
  const nid = id ?? 'map-' + (memMaps.length + 1);
  const i = memMaps.findIndex((x) => x.id === nid);
  const row = { id: nid, json, notes, name };
  if (i >= 0) memMaps[i] = row; else memMaps.push(row);
  return nid;
}
export async function mapDelete(id: string): Promise<void> {
  if (inTauri) return call<void>('map_delete', { id });
  memMaps = memMaps.filter((x) => x.id !== id);
}
export async function mapExportGpx(id: string, name: string, points: LatLon[]): Promise<string> {
  if (inTauri) return call<string>('map_export_gpx', { id, name, points });
  return 'user/saved-maps/' + id + '/route.gpx';
}
export async function routeStats(points: LatLon[]): Promise<RouteStats> {
  if (inTauri) return call<RouteStats>('route_stats', { points });
  let km = 0;
  for (let i = 1; i < points.length; i++) km += haversineKm(points[i - 1], points[i]);
  return { km, miles: km * 0.621371192 };
}

// ---------- map packs (the street and topo tiles on the drive) ----------
export type Pack = { rel: string; name: string; kind: string; mb: number };

export async function mapPacks(): Promise<Pack[]> {
  if (inTauri) return call<Pack[]>('map_packs');
  return []; // browser preview: no packs, so the screen shows the plan view
}

// ---------- finding a place (a town, a ZIP code, a park, a latitude and longitude) ----------
export type PlaceHit = { name: string; state: string; kind: string; lat: number; lon: number };
export type PlaceResults = { places: PlaceHit[]; has_list: boolean };

export async function placeSearch(query: string): Promise<PlaceResults> {
  if (inTauri) return call<PlaceResults>('place_search', { query });
  // Browser preview: a few sample places.
  const sample: PlaceHit[] = [
    { name: 'Denver', state: 'CO', kind: 'city', lat: 39.7392, lon: -104.9903 },
    { name: 'Colorado', state: 'CO', kind: 'state', lat: 39.0, lon: -105.5 },
  ];
  const q = query.trim().toLowerCase();
  return { places: q ? sample.filter((p) => p.name.toLowerCase().startsWith(q)) : [], has_list: false };
}

/** The address the map library reads a pack from. Only the desktop app can serve it. */
export async function tileUrl(rel: string): Promise<string> {
  const { convertFileSrc } = await import('@tauri-apps/api/core');
  return convertFileSrc(rel, 'cindrix-tiles');
}

// ---------- which product this is (Go, Vault or Hub) ----------
export type ProductInfo = {
  id: 'go' | 'vault' | 'hub';
  name: string;
  budget_gb: number | null;
  assistant: boolean;
  library: boolean;
  maps: boolean;
  plan: boolean;
  web_saver: boolean;
  serve_archives: boolean;
  music: boolean;
};

export async function productInfo(): Promise<ProductInfo> {
  if (inTauri) return call<ProductInfo>('product_info');
  // Browser preview and screenshots: show the full app as Go.
  return { id: 'go', name: 'Cindrix Ops Go', budget_gb: 100, assistant: true, library: true, maps: true, plan: true, web_saver: true, serve_archives: false, music: true };
}

export async function driveRescan(): Promise<void> {
  if (inTauri) await call<void>('drive_rescan');
}

// ---------- libraries: the Cindrix drives plugged in, and what the person said about each ----------
export type LibraryState = 'own' | 'always' | 'once' | 'never' | 'ask';
export type LibraryEntry = {
  id: string; name: string; edition: string; path: string;
  state: LibraryState; enabled: boolean; present: boolean; suggest_always: boolean;
};

export async function librariesList(): Promise<LibraryEntry[]> {
  if (inTauri) return call<LibraryEntry[]>('libraries_list');
  return []; // browser preview: no drives
}

/** "always", "once" (just this time) or "never" for the drive at `path`. */
export async function libraryAnswer(path: string, answer: 'always' | 'once' | 'never'): Promise<void> {
  if (inTauri) await call<void>('library_answer', { path, answer });
}

/** Forget what was said about a drive, so it is asked about again. */
export async function libraryForget(id: string): Promise<void> {
  if (inTauri) await call<void>('library_forget', { id });
}

export type CrawlStatus = {
  running: boolean;
  phase: string;
  pages: number;
  files: number;
  mb: number;
  queued: number;
  current: string;
  failed: number;
  finished: boolean;
  ok: boolean;
  message: string;
  file: string;
  blocked: number;
  hit_limit: boolean;
};
const idleCrawl: CrawlStatus = { running: false, phase: '', pages: 0, files: 0, mb: 0, queued: 0, current: '', failed: 0, finished: false, ok: false, message: '', file: '', blocked: 0, hit_limit: false };

export async function crawlStart(url: string, name: string, section: boolean, maxPages: number, maxMb: number, fast = false): Promise<void> {
  if (!inTauri) throw 'Saving websites works in the desktop app, not in the browser preview.';
  await call<void>('crawl_start', { url, name, section, maxPages, maxMb, fast });
}
export async function crawlStatus(): Promise<CrawlStatus> {
  if (inTauri) return call<CrawlStatus>('crawl_status');
  return idleCrawl;
}
export async function crawlCancel(): Promise<void> {
  if (inTauri) await call<void>('crawl_cancel');
}

export type CopyStatus = { running: boolean; finished: boolean; ok: boolean; name: string; copied_mb: number; total_mb: number; message: string };
const idleCopy: CopyStatus = { running: false, finished: false, ok: false, name: '', copied_mb: 0, total_mb: 0, message: '' };

/** Ask for a .zim file and copy it into the library. Resolves with the file name, or null if cancelled. */
export async function archiveAdd(): Promise<string | null> {
  if (!inTauri) throw 'Adding archives works in the desktop app, not in the browser preview.';
  return call<string | null>('archive_add');
}
export async function archiveCopyStatus(): Promise<CopyStatus> {
  if (inTauri) return call<CopyStatus>('archive_copy_status');
  return idleCopy;
}
export async function archiveRemove(name: string): Promise<void> {
  if (inTauri) await call<void>('archive_remove', { name });
}

// ---------- drills ----------
export type DrillResult = { date: string; seconds: number; done: number; total: number; notes: string };
export type Drill = {
  id: string; title: string; minutes: number; place: string; summary: string; status: string; steps: string[];
  last: DrillResult | null; times_done: number;
};
const memDrills: Drill[] = [
  { id: '01-power-outage', title: '72-hour power outage', minutes: 25, place: 'at home', status: 'draft', times_done: 1,
    summary: 'Light, water, food, phones, medications and a plan for the first three days.',
    last: { date: '2026-10-12', seconds: 754, done: 7, total: 8, notes: 'No spare batteries for the big lantern.' },
    steps: ['Find flashlights, lanterns and spare batteries. Test each one.', 'Count drinking water for everyone, for three days', 'Charge phones and battery banks, then switch to low-power mode', 'Gather medications and glucose supplies in one place', 'Keep the refrigerator and freezer doors closed', 'Never run a generator, grill or camp stove indoors', 'Write down who to call and where to meet', 'Check food that needs no cooking'] },
  { id: '04-boil-water', title: 'Boil-water advisory', minutes: 15, place: 'at home', status: 'draft', times_done: 0, last: null,
    summary: 'Make water safe, store it, and set up a clean-water routine.',
    steps: ['Fill clean containers before the water is shut off', 'Bring water to a rolling boil for 1 minute', 'Store boiled water in clean, covered containers', 'Check you have a backup method and its label'] },
];

export async function drillsList(): Promise<Drill[]> {
  if (inTauri) return call<Drill[]>('drills_list');
  return memDrills;
}
export async function drillSave(drill: string, seconds: number, done: number, total: number, notes: string): Promise<void> {
  if (inTauri) return call<void>('drill_save', { drill, seconds, done, total, notes });
  const d = memDrills.find((x) => x.id === drill);
  if (d) { d.last = { date: new Date().toISOString().slice(0, 10), seconds, done, total, notes }; d.times_done++; }
}


// ---------- academy (the text course) ----------
export type AcademyLessonMeta = { id: string; title: string; minutes: number; status: string; source: string; questions: number; tries: number; best_score: number; total: number; last_date: string };
export type AcademyModule = { id: string; title: string; summary: string; course: string; course_title: string; course_summary: string; course_author: string; lessons: AcademyLessonMeta[] };
export type CourseProblem = { place: string; message: string };
export type AcademyBlock = { kind: 'heading' | 'para' | 'note' | 'bullets' | 'steps'; text: string; items: string[] };
export type AcademyQuestion = { text: string; options: string[]; correct: number; explain: string };
export type AcademyLesson = { id: string; title: string; minutes: number; status: string; source: string; blocks: AcademyBlock[]; quiz: AcademyQuestion[] };

const previewLesson: AcademyLesson = {
  id: '01-first-72-hours/01-water-first', title: 'Water comes first', minutes: 6, status: 'draft', source: 'Preview sample',
  blocks: [
    { kind: 'para', text: 'A person can last about three days without water, but thinking and judgment fail much sooner.', items: [] },
    { kind: 'heading', text: 'What to do', items: [] },
    { kind: 'bullets', text: '', items: ['Fill clean containers before the supply stops', 'Know where your backup water is'] },
    { kind: 'note', text: 'Plan on about one gallon per person per day.', items: [] },
  ],
  quiz: [{ text: 'How long can a person last without water?', options: ['About three weeks', 'About three days'], correct: 1, explain: 'The rule of threes: 3 minutes without air, 3 hours without shelter in harsh weather, 3 days without water, 3 weeks without food.' }],
};
let memAcademy: Record<string, { tries: number; best: number; total: number }> = {};

export async function academyList(person?: string): Promise<AcademyModule[]> {
  if (inTauri) return call<AcademyModule[]>('academy_list', { person: person ?? null });
  const r = memAcademy[previewLesson.id];
  return [{ id: 'survival-basics:01-first-72-hours', title: 'The first 72 hours', summary: 'Water, shelter and a plan for the first three days.', course: 'survival-basics', course_title: 'Survival basics', course_summary: 'The core course.', course_author: '', lessons: [{ id: previewLesson.id, title: previewLesson.title, minutes: previewLesson.minutes, status: 'draft', source: previewLesson.source, questions: previewLesson.quiz.length, tries: r?.tries ?? 0, best_score: r?.best ?? 0, total: r?.total ?? 0, last_date: r ? 'today' : '' }] }];
}
export async function academyProblems(): Promise<CourseProblem[]> {
  if (inTauri) return call<CourseProblem[]>('academy_problems');
  return [];
}
export async function academyLesson(id: string): Promise<AcademyLesson> {
  if (inTauri) return call<AcademyLesson>('academy_lesson', { id });
  return previewLesson;
}
// ---------- plans (what comes after the first walkthrough) ----------
export type PlanStep = { id: string; title: string; every_days: number | null; check: string; go: string; button: string; body: string[]; state: 'todo' | 'done' | 'due'; last_done: string; due_in: number | null };
export type PlanInfo = { id: string; title: string; summary: string; kind: 'checks' | 'learning' | 'scenario'; author: string; status: string; followed: boolean; steps: PlanStep[] };
const memPlans: PlanInfo[] = [
  { id: 'keep-it-fresh', title: 'Keep it fresh', summary: 'Short checks to repeat on a schedule, so your supplies and your plan stay true.', kind: 'checks', author: 'Cindrix', status: 'draft', followed: false, steps: [
    { id: 'turn-over-your-stored-water', title: 'Turn over your stored water', every_days: 180, check: '', go: 'Inventory', button: 'Open Inventory', body: ['Water you filled yourself should be replaced about every six months.'], state: 'todo', last_done: '', due_in: null },
    { id: 'test-the-smoke-alarms', title: 'Test the smoke and carbon monoxide alarms', every_days: 30, check: '', go: '', button: '', body: ['Press the test button on every alarm.'], state: 'todo', last_done: '', due_in: null },
  ] },
  { id: 'power-outage', title: 'Power outage plan', summary: 'What to have ready and what to do when the power is out for a day or a week.', kind: 'scenario', author: 'Cindrix', status: 'draft', followed: false, steps: [
    { id: 'decide-how-you-will-get-light', title: 'Decide how you will get light', every_days: null, check: '', go: 'Inventory', button: 'Add these to Inventory', body: ['Flashlights or headlamps are safer than candles.'], state: 'todo', last_done: '', due_in: null },
    { id: 'keep-a-radio', title: 'Keep a radio that does not need the wall', every_days: null, check: '', go: '', button: '', body: ['Battery or hand-crank.'], state: 'todo', last_done: '', due_in: null },
  ] },
];
export async function plansList(): Promise<PlanInfo[]> {
  if (inTauri) return call<PlanInfo[]>('plans_list');
  return JSON.parse(JSON.stringify(memPlans));
}
export async function plansProblems(): Promise<CourseProblem[]> {
  if (inTauri) return call<CourseProblem[]>('plans_problems');
  return [];
}
export async function planFollow(id: string, on: boolean): Promise<void> {
  if (inTauri) return call<void>('plan_follow', { id, on });
  const p = memPlans.find((x) => x.id === id);
  if (p) p.followed = on;
}
export async function planTick(plan: string, step: string, done: boolean): Promise<void> {
  if (inTauri) return call<void>('plan_tick', { plan, step, done });
  const s = memPlans.find((x) => x.id === plan)?.steps.find((x) => x.id === step);
  if (s) { s.state = done ? 'done' : 'todo'; s.last_done = done ? 'today' : ''; s.due_in = done && s.every_days ? s.every_days : null; }
}

export async function academySave(lesson: string, score: number, total: number, person?: string): Promise<void> {
  if (inTauri) return call<void>('academy_save', { lesson, score, total, person: person ?? null });
  const r = memAcademy[lesson] ?? { tries: 0, best: 0, total };
  r.tries++; r.best = Math.max(r.best, score); r.total = total;
  memAcademy[lesson] = r;
}

// ---------- seed vault ----------
export type Seed = {
  id: string; crop: string; variety: string; kind: string; packets: number; seeds_per_packet: number; year_packed: number;
  germination: number; tested: string; source: string; location: string; plant_when: string; days_to_harvest: number;
  target: number; notes: string; status?: string; age?: number | null; years_good?: number;
};
let memSeeds: Seed[] = [
  { id: '1', crop: 'Tomato', variety: 'Brandywine', kind: 'Heirloom', packets: 2, seeds_per_packet: 30, year_packed: 2025, germination: 0, tested: '', source: 'Local seed swap', location: 'Fridge door', plant_when: 'Spring, indoors 6 weeks before last frost', days_to_harvest: 80, target: 3, notes: '' },
  { id: '2', crop: 'Pole bean', variety: 'Kentucky Wonder', kind: 'Heirloom', packets: 1, seeds_per_packet: 60, year_packed: 2021, germination: 0, tested: '', source: '', location: 'Seed box', plant_when: 'Spring after last frost', days_to_harvest: 65, target: 0, notes: '' },
];
function seedDecorate(s: Seed): Seed {
  const life = /onion|leek|parsnip/i.test(s.crop) ? 1 : /corn|pepper/i.test(s.crop) ? 2 : /bean|carrot|pea/i.test(s.crop) ? 3 : /tomato|beet/i.test(s.crop) ? 4 : 3;
  const age = s.year_packed ? new Date().getFullYear() - s.year_packed : null;
  const status = age != null && age > life ? 'replace' : age != null && age >= life ? 'check' : s.target > 0 && s.packets < s.target ? 'check' : 'ok';
  return { ...s, status, age, years_good: life };
}
export async function seedsList(): Promise<Seed[]> {
  if (inTauri) return call<Seed[]>('seeds_list');
  return memSeeds.map(seedDecorate);
}
export async function seedSave(seed: Seed): Promise<Seed[]> {
  if (inTauri) return call<Seed[]>('seed_save', { seed });
  if (!seed.id) seed = { ...seed, id: String(Math.max(0, ...memSeeds.map((s) => Number(s.id) || 0)) + 1) };
  const at = memSeeds.findIndex((s) => s.id === seed.id);
  if (at >= 0) memSeeds[at] = seed; else memSeeds.push(seed);
  return memSeeds.map(seedDecorate);
}
export async function seedDelete(ids: string[]): Promise<Seed[]> {
  if (inTauri) return call<Seed[]>('seed_delete', { ids });
  memSeeds = memSeeds.filter((s) => !ids.includes(s.id));
  return memSeeds.map(seedDecorate);
}
export async function seedGuides(): Promise<Card[]> {
  if (inTauri) return call<Card[]>('seed_guides');
  return [
    { id: 'start-small', title: 'What to buy first', kind: 'buy', status: 'draft', reviewer: '', reviewed: '', body: '**Start with what you eat.**\n\n1. Beans, corn, squash, potatoes and greens.\n2. Buy open-pollinated seed.\n\nSource: sample.' },
  ];
}

// ---------- how much to have ----------
export async function stockNeeds(adults: number, children: number, babies: number, smallPets: number, largePets: number, days: number): Promise<Need[]> {
  if (inTauri) return call<Need[]>('stock_needs', { adults, children, babies, smallPets, largePets, days });
  const p = adults + children + babies;
  return [
    { category: 'Water', name: 'Drinking and cooking water', unit: 'gal', amount: p * days, why: '1 gallon per person per day.', basis: 'Red Cross' },
    { category: 'Food', name: 'Food that keeps (meals)', unit: 'meals', amount: (adults + children) * 3 * days, why: 'Three meals a day for each adult and child.', basis: 'Red Cross' },
  ];
}

// ---------- training sign-off ----------
export type TrainLesson = { id: string; title: string; module: string; minutes: number; questions: number; state: 'passed' | 'tried' | 'new' | 'noquiz'; best_score: number; total: number; last_date: string };
export type TrainCourse = { id: string; title: string; summary: string; chosen: boolean; signed_by: string; signed_date: string; passed: number; needed: number; minutes_left: number; complete: boolean; lessons: TrainLesson[] };
export type TrainSuggestion = { course: string; title: string; reason: string };
export type TrainReport = { person: string; passed: number; needed: number; signed: number; chosen: number; courses: TrainCourse[]; suggestions: TrainSuggestion[] };

const memTrain: Record<string, { chosen: boolean; signed_by: string; signed_date: string }> = {};
export async function trainingReports(people: string[]): Promise<TrainReport[]> {
  if (inTauri) return call<TrainReport[]>('training_reports', { people });
  return people.map((person) => {
    const k = person + '|survival-basics';
    const m = memTrain[k];
    const r = memAcademy[previewLesson.id];
    const passed = r && r.total > 0 && r.best * 100 >= 70 * r.total;
    const lessons: TrainLesson[] = [
      { id: previewLesson.id, title: previewLesson.title, module: 'The first 72 hours', minutes: previewLesson.minutes, questions: 1, state: passed ? 'passed' : r ? 'tried' : 'new', best_score: r?.best ?? 0, total: r?.total ?? 0, last_date: r ? 'today' : '' },
      { id: 'preview/two', title: 'Staying warm and sheltered', module: 'The first 72 hours', minutes: 10, questions: 3, state: 'new', best_score: 0, total: 0, last_date: '' },
    ];
    const course: TrainCourse = { id: 'survival-basics', title: 'Survival basics', summary: 'The core course.', chosen: !!m?.chosen || !!r, signed_by: m?.signed_by ?? '', signed_date: m?.signed_date ?? '', passed: lessons.filter((l) => l.state === 'passed').length, needed: 2, minutes_left: passed ? 10 : 18, complete: false, lessons };
    const other: TrainCourse = { id: 'field-first-aid', title: 'Field first aid', summary: 'Bleeding, burns, breaks and shock.', chosen: false, signed_by: '', signed_date: '', passed: 0, needed: 4, minutes_left: 40, complete: false, lessons: [] };
    return { person, passed: course.passed, needed: course.chosen ? course.needed : 0, signed: course.signed_date ? 1 : 0, chosen: course.chosen ? 1 : 0, courses: [course, other], suggestions: course.chosen ? [{ course: course.id, title: course.title, reason: 'Keep going: 1 lesson left, about 10 minutes.' }, { course: other.id, title: other.title, reason: 'Next in the usual order.' }] : [{ course: course.id, title: course.title, reason: 'A good place to start.' }] };
  });
}
export async function trainingEnrol(person: string, course: string, on: boolean): Promise<void> {
  if (inTauri) return call<void>('training_enrol', { person, course, on });
  const k = person + '|' + course;
  memTrain[k] = { ...(memTrain[k] ?? { signed_by: '', signed_date: '' }), chosen: on };
}
export async function trainingSignoff(person: string, course: string, by: string, on: boolean): Promise<void> {
  if (inTauri) return call<void>('training_signoff', { person, course, by, on });
  const k = person + '|' + course;
  memTrain[k] = { chosen: true, signed_by: on ? by : '', signed_date: on ? 'today' : '' };
}

// ---------- sources and licenses ----------
export type SourceItem = { name: string; license: string; status: string; note: string };
export type SourceGroup = { title: string; items: SourceItem[] };
export type Sources = { note?: string; groups: SourceGroup[] };

export async function sourcesList(): Promise<Sources> {
  if (inTauri) return JSON.parse(await call<string>('sources_list')) as Sources;
  return { groups: [{ title: 'Program and tools', items: [{ name: 'llama.cpp (llama-server)', license: 'MIT', status: 'in this build', note: 'Runs the model on this computer only.' }] }] };
}


// ---------- music ----------
export type MusicTrack = { rel: string; title: string; artist: string; album: string; track: number; year: number; ext: string; convert: boolean };
export type MusicStatus = {
  folder_exists: boolean; scanning: boolean; scanned_once: boolean; seen: number; count: number; read: number; reused: number; skipped: number;
  converter: 'opus' | 'flac' | 'none' | 'unknown'; needs_converter: boolean; cache_bytes: number;
};
export type MusicQuery = { text?: string; artist?: string | null; album?: string | null; folder?: string | null; direct?: boolean };
export type MusicFacet = { name: string; sub: string; folder: string; count: number };
export type MusicEntry = { path: string; title: string; artist: string; album: string };
export type PlaylistMeta = { name: string; count: number };

// A tiny made-up library for the browser preview and the screenshots: each track is a generated tone.
const sampleTracks: MusicTrack[] = [
  { rel: 'Sample Artist/First Light/01 - Dawn.mp3', title: 'Dawn', artist: 'Sample Artist', album: 'First Light', track: 1, year: 2020, ext: 'mp3', convert: false },
  { rel: 'Sample Artist/First Light/02 - Noon.flac', title: 'Noon', artist: 'Sample Artist', album: 'First Light', track: 2, year: 2020, ext: 'flac', convert: false },
  { rel: 'Sample Artist/First Light/03 - Dusk.wma', title: 'Dusk', artist: 'Sample Artist', album: 'First Light', track: 3, year: 2020, ext: 'wma', convert: true },
  { rel: 'Other Band/Night Shift/01 - Lamp.ogg', title: 'Lamp', artist: 'Other Band', album: 'Night Shift', track: 1, year: 2018, ext: 'ogg', convert: false },
  { rel: 'Other Band/Night Shift/02 - Moth.wav', title: 'Moth', artist: 'Other Band', album: 'Night Shift', track: 2, year: 2018, ext: 'wav', convert: false },
  { rel: 'Loose song.m4a', title: 'Loose song', artist: '', album: '', track: 0, year: 0, ext: 'm4a', convert: false },
];
const memPlaylists: Record<string, MusicEntry[]> = {};
let memMusicState = '{}';

function previewMatch(t: MusicTrack, q: MusicQuery): boolean {
  const words = (q.text ?? '').toLowerCase().split(/\s+/).filter(Boolean);
  const hay = `${t.title} ${t.artist} ${t.album} ${t.rel}`.toLowerCase();
  if (!words.every((w) => hay.includes(w))) return false;
  if (q.artist != null && t.artist !== q.artist) return false;
  if (q.album != null && t.album !== q.album) return false;
  if (q.folder != null) {
    const dir = t.rel.includes('/') ? t.rel.slice(0, t.rel.lastIndexOf('/')) : '';
    if (q.direct ? dir !== q.folder : !(q.folder === '' || dir === q.folder || dir.startsWith(q.folder + '/'))) return false;
  }
  return true;
}

export async function musicStatus(): Promise<MusicStatus> {
  if (inTauri) return call<MusicStatus>('music_status');
  return { folder_exists: true, scanning: false, scanned_once: true, seen: sampleTracks.length, count: sampleTracks.length, read: sampleTracks.length, reused: 0, skipped: 0, converter: 'opus', needs_converter: true, cache_bytes: 0 };
}
export async function musicScan(): Promise<void> {
  if (inTauri) await call<void>('music_scan');
}
export async function musicQuery(q: MusicQuery, offset: number, limit: number): Promise<{ total: number; tracks: MusicTrack[] }> {
  if (inTauri) return call('music_query', { args: q, offset, limit });
  const all = sampleTracks.filter((t) => previewMatch(t, q));
  return { total: all.length, tracks: all.slice(offset, offset + limit) };
}
export async function musicFacets(kind: 'artists' | 'albums' | 'folders', prefix?: string): Promise<MusicFacet[]> {
  if (inTauri) return call<MusicFacet[]>('music_facets', { kind, prefix: prefix ?? null });
  const out = new Map<string, MusicFacet>();
  for (const t of sampleTracks) {
    if (kind === 'artists') { const k = t.artist; const f = out.get(k) ?? { name: k, sub: '', folder: '', count: 0 }; f.count++; out.set(k, f); }
    else if (kind === 'albums') { if (!t.album) continue; const k = t.artist + '|' + t.album; const f = out.get(k) ?? { name: t.album, sub: t.artist, folder: '', count: 0 }; f.count++; out.set(k, f); }
    else {
      const dir = t.rel.includes('/') ? t.rel.slice(0, t.rel.lastIndexOf('/')) : '';
      const pre = prefix ? prefix + '/' : '';
      if (!dir.startsWith(pre) || dir === (prefix ?? '')) continue;
      const first = dir.slice(pre.length).split('/')[0];
      const k = pre + first; const f = out.get(k) ?? { name: first, sub: '', folder: k, count: 0 }; f.count++; out.set(k, f);
    }
  }
  return [...out.values()];
}
export async function musicResolve(rels: string[]): Promise<MusicTrack[]> {
  if (inTauri) return call<MusicTrack[]>('music_resolve', { rels });
  return rels.map((r) => sampleTracks.find((t) => t.rel === r)).filter((t): t is MusicTrack => !!t);
}

function wavTone(freq: number, seconds: number): string {
  const rate = 8000, n = rate * seconds, buf = new ArrayBuffer(44 + n * 2), v = new DataView(buf);
  const str = (o: number, t: string) => { for (let i = 0; i < t.length; i++) v.setUint8(o + i, t.charCodeAt(i)); };
  str(0, 'RIFF'); v.setUint32(4, 36 + n * 2, true); str(8, 'WAVEfmt '); v.setUint32(16, 16, true); v.setUint16(20, 1, true); v.setUint16(22, 1, true);
  v.setUint32(24, rate, true); v.setUint32(28, rate * 2, true); v.setUint16(32, 2, true); v.setUint16(34, 16, true); str(36, 'data'); v.setUint32(40, n * 2, true);
  for (let i = 0; i < n; i++) v.setInt16(44 + i * 2, Math.round(Math.sin((2 * Math.PI * freq * i) / rate) * 6000), true);
  return URL.createObjectURL(new Blob([buf], { type: 'audio/wav' }));
}
const toneCache = new Map<string, string>();

/** An address the audio element can play. Converts the file first if the web view cannot play it. */
export async function musicSource(rel: string): Promise<string> {
  if (!inTauri) {
    let u = toneCache.get(rel);
    if (!u) { u = wavTone(220 + (sampleTracks.findIndex((t) => t.rel === rel) + 1) * 55, 30); toneCache.set(rel, u); }
    return u;
  }
  const path = await call<string>('music_prepare', { rel });
  const { convertFileSrc } = await import('@tauri-apps/api/core');
  // The program hands back `t/<path in the music folder>` or `c/<name>` as plain text; convertFileSrc encodes it once.
  return convertFileSrc(path, 'cindrix-music');
}
/** The cover picture for a track (embedded or in its folder), or null in the browser preview. */
export async function musicArtUrl(rel: string): Promise<string | null> {
  if (!inTauri) return null;
  const { convertFileSrc } = await import('@tauri-apps/api/core');
  return convertFileSrc('a/' + rel, 'cindrix-music');
}

export async function musicPlaylists(): Promise<PlaylistMeta[]> {
  if (inTauri) return call<PlaylistMeta[]>('music_playlists');
  return Object.entries(memPlaylists).map(([name, e]) => ({ name, count: e.length })).sort((a, b) => a.name.localeCompare(b.name));
}
export async function musicPlaylistLoad(name: string): Promise<MusicEntry[]> {
  if (inTauri) return call<MusicEntry[]>('music_playlist_load', { name });
  if (!memPlaylists[name]) throw 'There is no such playlist.';
  return memPlaylists[name];
}
export async function musicPlaylistSave(name: string, entries: MusicEntry[]): Promise<void> {
  if (inTauri) return call<void>('music_playlist_save', { name, entries });
  memPlaylists[name] = entries;
}
export async function musicPlaylistDelete(name: string): Promise<void> {
  if (inTauri) return call<void>('music_playlist_delete', { name });
  delete memPlaylists[name];
}
export async function musicStateLoad(): Promise<string> {
  if (inTauri) return call<string>('music_state_load');
  return memMusicState;
}
export async function musicStateSave(json: string): Promise<void> {
  if (inTauri) return call<void>('music_state_save', { json });
  memMusicState = json;
}
export async function musicClearCache(): Promise<number> {
  if (inTauri) return call<number>('music_clear_cache');
  return 0;
}


// ---------- the in-app archive reader ----------
export type ZimBook = { name: string; title: string; description: string; articles: number; mb: number };
export type ZimHit = { book: string; book_title: string; title: string; path: string };
export type ZimBookmark = { book: string; path: string; title: string };
export type ZimSuggestion = { title: string; path: string };

const previewPages: Record<string, { title: string; body: string }> = {
  '/content/sample/A/Main': { title: 'Sample library', body: '<h1>Sample library</h1><p>This is a made-up archive for the browser preview.</p><ul><li><a href="/content/sample/A/Water">Water</a></li><li><a href="/content/sample/A/Fire">Fire</a></li></ul><p><a href="https://example.org/outside">An outside link</a></p>' },
  '/content/sample/A/Water': { title: 'Water', body: '<h1>Water</h1><p>Boil water for one minute to make it safe to drink.</p><p>Back to <a href="/content/sample/A/Main">the main page</a>.</p>' },
  '/content/sample/A/Fire': { title: 'Fire', body: '<h1>Fire</h1><p>Keep fuel and tinder dry. Never use a fire or stove indoors.</p><p><a href="/content/sample/A/Water">Water</a></p>' },
};
const previewScript = `<script>document.addEventListener('click',function(e){var a=e.target.closest&&e.target.closest('a[href]');if(!a)return;e.preventDefault();var h=a.getAttribute('href');if(h.indexOf('/content/')===0)parent.postMessage({cx:'nav',path:h},'*');else parent.postMessage({cx:'external',url:h},'*')},true);addEventListener('message',function(e){if(e.data&&e.data.cx==='zoom')document.documentElement.style.zoom=e.data.v});<\/script>`;
let memZimBookmarks = '{}';

export async function zimBooks(): Promise<ZimBook[]> {
  if (inTauri) return call<ZimBook[]>('zim_books');
  return [{ name: 'sample', title: 'Sample library (preview)', description: 'A made-up archive for the browser preview.', articles: 3, mb: 1 }];
}
/** Titles matching the words in every archive at once (best of each archive first). */
export async function zimSearchAll(term: string): Promise<ZimHit[]> {
  if (inTauri) return call<ZimHit[]>('zim_search_all', { term });
  const t = term.toLowerCase();
  return Object.entries(previewPages).filter(([, v]) => v.title.toLowerCase().includes(t)).map(([p, v]) => ({ book: 'sample', book_title: 'Sample library (preview)', title: v.title, path: p }));
}
/** What to put in the reader's frame for a library path: an address (desktop app) or the page itself (browser preview). */
export async function zimFrame(path: string): Promise<{ src?: string; srcdoc?: string; title: string }> {
  if (inTauri) {
    const { convertFileSrc } = await import('@tauri-apps/api/core');
    // convertFileSrc encodes the slashes, so take the address of the root and add the path to it
    const origin = convertFileSrc('x', 'cindrix-zim').slice(0, -1);
    return { src: origin + path, title: '' };
  }
  const raw = path.split('?')[0];
  const key = raw === '/content/sample' || raw === '/content/sample/' ? '/content/sample/A/Main' : raw;
  const pg = previewPages[key] ?? { title: 'Not found', body: '<p>No such page in the preview.</p>' };
  return { srcdoc: `<!doctype html><html><head><meta charset="utf-8"><title>${pg.title}</title><style>body{font:16px system-ui;margin:16px;color:#111}</style>${previewScript}</head><body>${pg.body}</body></html>`, title: pg.title };
}
/** The address the reader sits at, to turn a page address from the frame back into a library path. */
export async function zimOrigin(): Promise<string> {
  if (!inTauri) return '';
  const { convertFileSrc } = await import('@tauri-apps/api/core');
  return convertFileSrc('x', 'cindrix-zim').slice(0, -1);
}
export async function zimSuggest(book: string, term: string): Promise<ZimSuggestion[]> {
  if (!inTauri) {
    const t = term.toLowerCase();
    return Object.entries(previewPages).filter(([, v]) => v.title.toLowerCase().includes(t)).map(([p, v]) => ({ title: v.title, path: p }));
  }
  const origin = await zimOrigin();
  const r = await fetch(`${origin}/suggest?content=${encodeURIComponent(book)}&term=${encodeURIComponent(term)}`);
  if (!r.ok) return [];
  const j = (await r.json()) as { value?: string; label?: string; path?: string }[];
  return j
    .filter((x) => x.path)
    .slice(0, 12)
    .map((x) => ({ title: (x.value || x.label || x.path || '').replace(/<[^>]+>/g, ''), path: `/content/${encodeURIComponent(book)}/${(x.path as string).split('/').map(encodeURIComponent).join('/')}` }));
}
export async function zimBookmarksLoad(): Promise<string> {
  if (inTauri) return call<string>('zim_bookmarks_load');
  return memZimBookmarks;
}
export async function zimBookmarksSave(json: string): Promise<void> {
  if (inTauri) return call<void>('zim_bookmarks_save', { json });
  memZimBookmarks = json;
}


// ---------- settings ----------
let memSettings = '{}';
/** The settings file's text ("{}" when nothing is saved yet). */
export async function settingsLoad(): Promise<string> {
  if (inTauri) return call<string>('settings_load');
  return memSettings;
}
export async function settingsSave(json: string): Promise<void> {
  if (inTauri) return call<void>('settings_save', { json });
  memSettings = json;
}

/** Let the owner pick a folder on this computer. Resolves with its path, or null if cancelled. */
export async function pickFolder(): Promise<string | null> {
  if (!inTauri) throw 'Choosing a folder works in the desktop app, not in the browser preview.';
  return call<string | null>('pick_folder');
}
