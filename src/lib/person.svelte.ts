// Whose journal, photos and comms plan are open. The names come from the Family page (see learner.svelte.ts).
// With no Family page yet, there is one person: "Me". The choice is remembered on this device.
import { learner, loadPeople } from './learner.svelte';

const KEY = 'cindrix.person';
const ME = 'Me';

function remembered(): string {
  try { return localStorage.getItem(KEY) || ME; } catch { return ME; }
}

export const who = $state({ name: remembered() });

export function choosePerson(name: string) {
  who.name = name || ME;
  try { localStorage.setItem(KEY, who.name); } catch { /* the choice just is not remembered */ }
}

export function everyone(): string[] {
  return [ME, ...learner.people.filter((p) => p !== ME)];
}

export async function loadPersonList() {
  if (!learner.loaded) await loadPeople();
  if (!everyone().includes(who.name)) choosePerson(ME);
}
