// Talks to the Rust core. In a plain browser (no Tauri) it falls back to in-memory samples,
// so the screens can be previewed and screenshotted.

export type Card = { id: string; title: string; kind: string; status: string; reviewer: string; reviewed: string; body: string };
export type Reply = { kind: 'Refused' | 'NotFound' | 'Answered' | 'Replaced'; text: string; sources: string[]; card: string | null };
export type Item = {
  id: string; category: string; name: string; location: string; quantity: number; unit: string;
  use_per_day: number; use_by: string; notes: string; status?: string; days_left?: number | null;
};
export type Cascade = {
  runs_out: { name: string; location: string; days: number }[];
  expiring: { name: string; months: number }[];
  water_days: number | null; food_days: number | null;
};
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
export type ArchiveInfo = { file: string; mb: number; articles: number; saved_site: boolean; note: string };
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
  return { archives: [], ready: false, url: null, shared: false, lan_url: null, drive: null, folder: '', online: false };
}

export async function libraryShare(enable: boolean): Promise<void> {
  if (inTauri) await call<void>('library_share', { enable });
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
};

export async function productInfo(): Promise<ProductInfo> {
  if (inTauri) return call<ProductInfo>('product_info');
  // Browser preview and screenshots: show the full app as Go.
  return { id: 'go', name: 'Cindrix Ops Go', budget_gb: 100, assistant: true, library: true, maps: true, plan: true, web_saver: true, serve_archives: false };
}

export async function driveRescan(): Promise<void> {
  if (inTauri) await call<void>('drive_rescan');
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

export async function crawlStart(url: string, name: string, section: boolean, maxPages: number, maxMb: number): Promise<void> {
  if (!inTauri) throw 'Saving websites works in the desktop app, not in the browser preview.';
  await call<void>('crawl_start', { url, name, section, maxPages, maxMb });
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

// ---------- sources and licenses ----------
export type SourceItem = { name: string; license: string; status: string; note: string };
export type SourceGroup = { title: string; items: SourceItem[] };
export type Sources = { note?: string; groups: SourceGroup[] };

export async function sourcesList(): Promise<Sources> {
  if (inTauri) return JSON.parse(await call<string>('sources_list')) as Sources;
  return { groups: [{ title: 'Program and tools', items: [{ name: 'llama.cpp (llama-server)', license: 'MIT', status: 'in this build', note: 'Runs the model on this computer only.' }] }] };
}
