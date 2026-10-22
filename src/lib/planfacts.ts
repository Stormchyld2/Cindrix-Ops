// What the program can already tell about the person's setup, so plan steps can tick themselves.
import { familyStatus, inventoryList, drillsList, libraryStatus, modelStatus, listModels, mapPacks, whatFailsNext, academyList, type PlanStep, type PlanInfo } from './api';

export type PlanFacts = { checks: Record<string, boolean>; lessons: Set<string> };

export async function gatherFacts(): Promise<PlanFacts> {
  const checks: Record<string, boolean> = { water: false, food: false, family: false, inventory: false, drill: false, maps: false, model: false, library: false };
  const lessons = new Set<string>();
  const tryIt = async (f: () => Promise<void>) => { try { await f(); } catch { /* left as not done */ } };
  await Promise.all([
    tryIt(async () => { checks.family = (await familyStatus()).exists; }),
    tryIt(async () => { checks.inventory = (await inventoryList()).length > 0; }),
    tryIt(async () => { const c = await whatFailsNext(); checks.water = c.water_days != null; checks.food = c.food_days != null; }),
    tryIt(async () => { checks.drill = (await drillsList()).some((d) => d.times_done > 0); }),
    tryIt(async () => { checks.maps = (await mapPacks()).length > 0; }),
    tryIt(async () => { checks.model = (await modelStatus()).state === 'ready' || (await listModels()).length > 0; }),
    tryIt(async () => { checks.library = (await libraryStatus()).archives.length > 0; }),
    tryIt(async () => { for (const m of await academyList()) for (const l of m.lessons) if (l.tries > 0) lessons.add(l.id); }),
  ]);
  return { checks, lessons };
}

/** True when a one-time step ticks itself from what the person has set up. */
export function autoDone(s: PlanStep, f: PlanFacts): boolean {
  if (s.every_days || !s.check) return false;
  if (s.check.startsWith('lesson:')) return f.lessons.has(s.check.slice(7));
  return !!f.checks[s.check];
}

/** What the person sees for a step: done, due again, or still to do. */
export function stepStatus(s: PlanStep, f: PlanFacts): 'done' | 'due' | 'todo' {
  if (autoDone(s, f)) return 'done';
  return s.state;
}

export function planCounts(p: PlanInfo, f: PlanFacts) {
  let done = 0, due = 0;
  for (const s of p.steps) {
    const st = stepStatus(s, f);
    if (st === 'done') done++;
    else if (st === 'due') due++;
  }
  return { done, due, total: p.steps.length };
}

/** The first step still to do (an overdue repeat comes first). */
export function nextStep(p: PlanInfo, f: PlanFacts): PlanStep | undefined {
  return p.steps.find((s) => stepStatus(s, f) === 'due') ?? p.steps.find((s) => stepStatus(s, f) === 'todo');
}

export const KIND_NAME: Record<string, string> = { checks: 'Repeating checks', learning: 'Learning plan', scenario: 'Emergency plan' };
