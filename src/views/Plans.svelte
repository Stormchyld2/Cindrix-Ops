<script lang="ts">
  import { onMount } from 'svelte';
  import { learner, loadPeople } from '../lib/learner.svelte';
  import { plansList, plansProblems, planFollow, planTick, type PlanInfo, type PlanStep, type CourseProblem } from '../lib/api';
  import { gatherFacts, stepStatus, planCounts, nextStep, autoDone, KIND_NAME, type PlanFacts } from '../lib/planfacts';

  let { go }: { go: (tab: string) => void } = $props();

  let plans: PlanInfo[] = $state([]);
  let problems: CourseProblem[] = $state([]);
  let facts: PlanFacts = $state({ checks: {}, lessons: new Set<string>() });
  let loaded = $state(false);
  let error = $state('');
  let openId: string | null = $state(null);
  // '' is the whole household; a name keeps that person's own ticks and plans
  let forWho = $state('');

  async function load() {
    try {
      plans = await plansList(forWho);
      problems = await plansProblems();
      facts = await gatherFacts();
      error = '';
    } catch (e) {
      error = 'The plans could not be read: ' + String(e);
    }
    loaded = true;
  }
  onMount(async () => {
    void loadPeople();
    await load();
    try {
      const want = sessionStorage.getItem('cindrix.openPlan');
      if (want) { sessionStorage.removeItem('cindrix.openPlan'); if (plans.some((p) => p.id === want)) openId = want; }
    } catch { /* nothing asked for */ }
  });

  const open = $derived(openId ? plans.find((p) => p.id === openId) : undefined);
  // the plans being followed come first
  const ordered = $derived([...plans].sort((a, b) => Number(b.followed) - Number(a.followed)));
  const GROUPS: [string, string][] = [
    ['planning', 'HOW MUCH TO HAVE'],
    ['prevent', 'KEEP IT FROM FAILING'],
    ['scenario', 'EMERGENCY PLANS'],
    ['checks', 'REPEATING CHECKS'],
    ['learning', 'LEARNING'],
  ];
  const groups = $derived.by(() => {
    const out: { title: string; list: PlanInfo[] }[] = [];
    const mine = ordered.filter((p) => p.followed);
    if (mine.length) out.push({ title: 'PLANS YOU ARE FOLLOWING', list: mine });
    for (const [k, t] of GROUPS) {
      const l = ordered.filter((p) => p.kind === k && !p.followed).sort((a, b) => a.title.localeCompare(b.title));
      if (l.length) out.push({ title: t, list: l });
    }
    const known = GROUPS.map(([k]) => k);
    const rest = ordered.filter((p) => !p.followed && !known.includes(p.kind));
    if (rest.length) out.push({ title: 'OTHER', list: rest });
    return out;
  });
  const followedCount = $derived(plans.filter((p) => p.followed).length);

  async function follow(p: PlanInfo, on: boolean) {
    try { await planFollow(p.id, on, forWho); await load(); } catch (e) { error = String(e); }
  }
  async function tick(p: PlanInfo, s: PlanStep, done: boolean) {
    try { await planTick(p.id, s.id, done, forWho); await load(); } catch (e) { error = String(e); }
  }
  function dueText(s: PlanStep): string {
    if (s.every_days == null) return '';
    if (s.state === 'todo') return `Repeats every ${s.every_days} days.`;
    const left = s.due_in ?? 0;
    if (left < 0) return `Overdue by ${-left} day${left === -1 ? '' : 's'}. Repeats every ${s.every_days} days.`;
    if (left === 0) return `Due today. Repeats every ${s.every_days} days.`;
    return `Done ${s.last_done}. Due again in ${left} day${left === 1 ? '' : 's'}.`;
  }
</script>

{#snippet whoPick()}
  {#if learner.people.length}
    <label class="forwho">Plans for:
      <select class="input" bind:value={forWho} onchange={load} aria-label="Whose plans">
        <option value="">Our household</option>
        {#each learner.people as n}<option value={n}>{n}</option>{/each}
      </select>
    </label>
    {#if forWho}<p class="muted small">Following and ticking here is only for {forWho}. The household's own plans and the Dashboard are not changed.</p>{/if}
  {/if}
{/snippet}

{#if !open}
  <section>
    <h1>Plans</h1>
    <p class="sub">The first walkthrough gets you ready. These plans keep going after that: checks that come back on a schedule, things to learn, a plan for each kind of emergency, a guide to keep each thing from failing, and a guide to how much to have. Follow as many as you like at the same time. Choose a plan to go inside it.</p>
  </section>
  {#if error}<div class="warn err" role="alert">{error}</div>{/if}
  {@render whoPick()}

  <div class="cards">
    <button class="card base" onclick={() => go('Start')} aria-label="Open the first walkthrough, Get ready">
      <span class="kind">THE FIRST STEPS</span>
      <span class="ctitle">Get ready</span>
      <span class="csum">The nine-step walkthrough: water, food, health, power, where to go, a binder and a drill. Do this first.</span>
      <span class="meta">Opens the walkthrough →</span>
    </button>
  </div>
  {#each groups as g (g.title)}
    <div class="label gh">{g.title}</div>
    <div class="cards">
        {#each g.list as p (p.id)}
      {@const c = planCounts(p, facts)}
      <button class="card" class:on={p.followed} onclick={() => (openId = p.id)} aria-label={`Open the plan ${p.title}`}>
        <span class="kind">{KIND_NAME[p.kind] ?? p.kind}{p.followed ? ' · FOLLOWING' : ''}</span>
        <span class="ctitle">{p.title}</span>
        {#if p.summary}<span class="csum">{p.summary}</span>{/if}
        <span class="track"><span style="width:{c.total ? (c.done / c.total) * 100 : 0}%"></span></span>
        <span class="meta">{c.done} of {c.total} done{c.due ? ` · ${c.due} due again` : ''}{p.status !== 'reviewed' ? ' · not yet reviewed' : ''}</span>
      </button>
        {/each}
    </div>
  {/each}
  {#if loaded && plans.length === 0 && !error}<p class="muted">No plans found. A plan is a text file in a <code>plans</code> folder.</p>{/if}
  {#if followedCount === 0 && plans.length}<p class="muted">You are not following any plan yet. Open one and press "Follow this plan" to keep track of it on the Dashboard.</p>{/if}
  {#if problems.length}
    <details class="panel probs">
      <summary>{problems.length} plan file{problems.length === 1 ? ' was' : 's were'} left out</summary>
      <p class="muted small">The other plans still work. Fix the file and it appears next time.</p>
      <ul>{#each problems as q}<li><code>{q.place}</code>: {q.message}</li>{/each}</ul>
    </details>
  {/if}
{:else}
  {@const c = planCounts(open, facts)}
  {@const nxt = nextStep(open, facts)}
  {@render whoPick()}
  <div class="row between">
    <button class="btn" onclick={() => (openId = null)}>← All plans</button>
    <button class="btn" title="Open the Binder with this plan ticked, ready to print" onclick={() => { try { sessionStorage.setItem('cindrix.binderPlan', open.id); } catch { /* fine */ } go('Binder'); }}>Print in Binder</button>
    {#if open.followed}
      <button class="btn" title="Stop following this plan (your ticks are kept)" onclick={() => follow(open, false)}>Following ✓ · stop</button>
    {:else}
      <button class="btn primary" title="Keep track of this plan and show it on the Dashboard" onclick={() => follow(open, true)}>Follow this plan</button>
    {/if}
  </div>
  <header class="course">
    <div class="kind">{KIND_NAME[open.kind] ?? open.kind}{open.author ? ' · by ' + open.author : ''}</div>
    <h1>{open.title}</h1>
    {#if open.summary}<p class="muted">{open.summary}</p>{/if}
    {#if open.status !== 'reviewed'}<p class="muted small">Not yet reviewed by a qualified person. Treat it as a starting point.</p>{/if}
  </header>
  {#if error}<div class="warn err" role="alert">{error}</div>{/if}
  <div class="panel prog">
    <span class="label">PROGRESS</span>
    <span>{c.done} of {c.total} done{c.due ? ` · ${c.due} due again` : ''}{nxt ? ` · next: ${nxt.title}` : ' · all done for now'}</span>
    <div class="track"><span style="width:{c.total ? (c.done / c.total) * 100 : 0}%"></span></div>
  </div>

  {#each open.steps as s, n (s.id)}
    {@const st = stepStatus(s, facts)}
    {@const auto = autoDone(s, facts)}
    <section class="panel step" class:due={st === 'due'}>
      <div class="steptop">
        <button
          class="box" class:done={st === 'done'} class:due={st === 'due'}
          disabled={auto}
          aria-pressed={st === 'done'}
          aria-label={`${st === 'done' ? 'Mark not done' : 'Mark done'}: ${s.title}`}
          title={auto ? 'This ticks itself from what you have set up' : st === 'done' ? 'Tap to say this is not done' : 'Tap when you have done this'}
          onclick={() => tick(open, s, st !== 'done')}
        >{st === 'done' ? '✓' : st === 'due' ? '!' : n + 1}</button>
        <h2 class="stitle">{s.title}</h2>
        {#if s.go}<button class="btn" class:primary={st !== 'done'} onclick={() => go(s.go)}>{s.button || 'Open'}</button>{/if}
      </div>
      {#if st === 'due'}<p class="duetext">Due again. {dueText(s)}</p>
      {:else if s.every_days != null}<p class="muted small">{dueText(s)}</p>
      {:else if auto}<p class="muted small">Done — worked out from what you have set up.</p>{/if}
      {#each s.body as line}
        {#if line.startsWith('- ')}<p class="li">{line.slice(2)}</p>{:else}<p class="body">{line}</p>{/if}
      {/each}
    </section>
  {/each}
{/if}

<style>
  .forwho { display: inline-flex; gap: 8px; align-items: center; margin: 4px 0; }
  .gh { margin-top: 18px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; margin-top: 12px; }
  .card { display: flex; flex-direction: column; gap: 8px; text-align: left; padding: 16px; background: var(--panel); border: 1px solid var(--line); border-top: 2px solid var(--line2); border-radius: 12px; }
  .card:hover { border-color: var(--blue-line); }
  .card.on { border-top-color: var(--blue); }
  .card.base { border-style: dashed; }
  .kind { font-family: var(--mono); font-size: 11px; letter-spacing: .1em; color: var(--blue); }
  .ctitle { font-size: 18px; letter-spacing: .06em; text-transform: uppercase; }
  .csum { color: var(--muted); }
  .meta { font-family: var(--mono); font-size: 11px; letter-spacing: .06em; color: var(--muted); }
  .track { height: 8px; border-radius: 4px; background: var(--line); overflow: hidden; display: block; }
  .track span { display: block; height: 100%; background: var(--blue); }
  .between { justify-content: space-between; }
  .course { margin: 14px 0 10px; }
  .course h1 { margin: 4px 0; }
  .prog { display: flex; flex-direction: column; gap: 6px; }
  .step { margin-top: 12px; }
  .step.due { border-top-color: #ff9a1f; }
  .steptop { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px; }
  .stitle { flex: 1 1 200px; font-size: 17px; letter-spacing: .05em; margin: 0; }
  .box { flex: 0 0 34px; height: 34px; border-radius: 50%; border: 1px solid var(--line2); background: transparent; color: var(--muted); font-size: 14px; }
  .box:hover:not(:disabled) { border-color: var(--blue); color: var(--blue); }
  .box.done { background: #1f2a1d; border-color: #34502f; color: var(--ok); }
  .box.due { background: #2a2210; border-color: #7a5a1c; color: #ffc247; }
  .duetext { color: #ffc247; margin: 8px 0 0; }
  .body { margin: 8px 0 0; }
  .li { margin: 4px 0 0 18px; }
  .li::before { content: '• '; color: var(--muted); }
  .small { font-size: 13px; }
  .err { border-color: #7a2e22; background: #2a1511; color: #ffb4a6; }
  .probs { margin-top: 14px; }
</style>
