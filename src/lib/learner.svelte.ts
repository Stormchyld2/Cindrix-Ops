// Who is learning in the Academy right now, and the names on the Family page they can be chosen from.
// The Family page may be locked with a passphrase; then the names stay hidden until it is unlocked here.
import { familyStatus, familyLoad } from './api';

const KEY = 'cindrix.learner';

function remembered(): string {
  try { return localStorage.getItem(KEY) ?? ''; } catch { return ''; }
}

export const learner = $state({ name: remembered(), people: [] as string[], locked: false, loaded: false });

export function chooseLearner(name: string) {
  learner.name = name;
  try { localStorage.setItem(KEY, name); } catch { /* the choice just is not remembered */ }
}

function names(people: { name: string }[]): string[] {
  return [...new Set(people.map((p) => p.name.trim()).filter((n) => n))];
}

/** Read the names. Returns false when the Family page is locked and no (or a wrong) passphrase was given. */
export async function loadPeople(passphrase?: string): Promise<boolean> {
  try {
    const st = await familyStatus();
    if (st.exists && st.locked && !passphrase) {
      learner.locked = true;
      learner.loaded = true;
      return false;
    }
    const f = st.exists ? await familyLoad(passphrase) : { people: [] };
    learner.people = names(f.people);
    learner.locked = false;
    learner.loaded = true;
    if (learner.name && !learner.people.includes(learner.name)) chooseLearner('');
    return true;
  } catch {
    learner.locked = true;
    learner.loaded = true;
    return false;
  }
}
