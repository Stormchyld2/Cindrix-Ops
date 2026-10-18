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

export async function ask(question: string): Promise<Reply> {
  if (inTauri) return call<Reply>('ask', { question });
  return { kind: 'NotFound', text: 'Browser preview only. Run the desktop app to ask the library.', sources: [], card: null };
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
  const groups = new Map<string, { q: number; per: number; places: Set<string> }>();
  for (const i of items.filter((x) => x.use_per_day > 0)) {
    const g = groups.get(i.name) ?? { q: 0, per: 0, places: new Set<string>() };
    g.q += i.quantity; g.per = Math.max(g.per, i.use_per_day); g.places.add(i.location);
    groups.set(i.name, g);
  }
  const runs_out = [...groups].map(([name, g]) => ({ name, location: g.places.size > 1 ? 'several places' : [...g.places][0], days: g.q / g.per }))
    .sort((a, b) => a.days - b.days);
  const days = (cat: string) => {
    const rows = items.filter((i) => i.category === cat && i.use_per_day > 0);
    const per = rows.reduce((s, r) => s + r.use_per_day, 0);
    return per > 0 ? rows.reduce((s, r) => s + r.quantity, 0) / per : null;
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
