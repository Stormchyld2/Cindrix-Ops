<script lang="ts">
  import { onMount } from 'svelte';
  import PageHeader from '../lib/PageHeader.svelte';
  import Icon from '../lib/Icon.svelte';
  import { learner, loadPeople } from '../lib/learner.svelte';
  import { plansList, plansProblems, planFollow, planTick, plansExport, planCreate, type PlanInfo, type PlanStep, type CourseProblem, type NewPlanStep } from '../lib/api';
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
  let showKind = $state('all');
  // favourites: a star on a plan card. Kept for this window (it is only a way to find plans again).
  const FAV_KEY = 'cindrix.plans.fav';
  let favs: string[] = $state((() => { try { return JSON.parse(localStorage.getItem(FAV_KEY) || '[]'); } catch { return []; } })());
  function toggleFav(id: string) {
    favs = favs.includes(id) ? favs.filter((x) => x !== id) : [...favs, id];
    try { localStorage.setItem(FAV_KEY, JSON.stringify(favs)); } catch { /* fine */ }
  }
  // the repeating checks that are due again, in the plans being followed
  const dueChecks = $derived(plans.filter((p) => p.followed).flatMap((p) => p.steps.filter((st) => stepStatus(st, facts) === 'due').map((st) => ({ plan: p, step: st }))));
  let dueDlg: HTMLDialogElement;
  // write a plan
  let mkDlg: HTMLDialogElement;
  let mk = $state<{ title: string; summary: string; kind: string; author: string; steps: { title: string; every: string; body: string }[] }>({ title: '', summary: '', kind: 'checks', author: '', steps: [{ title: '', every: '', body: '' }] });
  let mkNote = $state('');
  async function createPlan() {
    mkNote = '';
    const steps: NewPlanStep[] = mk.steps.filter((x) => x.title.trim()).map((x) => ({ title: x.title, every_days: Number(x.every) > 0 ? Math.floor(Number(x.every)) : null, body: x.body.split('\n') }));
    try {
      await planCreate(mk.title, mk.summary, mk.kind, mk.author, steps);
      mkDlg.close();
      mk = { title: '', summary: '', kind: 'checks', author: mk.author, steps: [{ title: '', every: '', body: '' }] };
      await load();
      showKind = 'mine';
    } catch (e) { mkNote = String(e); }
  }
  let planQ = $state('');
  const libraryList = $derived(ordered.filter((p) => (showKind === 'all' || (showKind === 'following' ? p.followed : showKind === 'mine' ? p.mine : showKind === 'favorites' ? favs.includes(p.id) : p.kind === showKind)) && (!planQ.trim() || (p.title + ' ' + (p.summary ?? '')).toLowerCase().includes(planQ.trim().toLowerCase()))));
  const planStatus = $derived.by(() => {
    let complete = 0, prog = 0, none = 0;
    for (const p of plans) { const c = planCounts(p, facts); if (c.total > 0 && c.done === c.total) complete++; else if (c.done > 0) prog++; else none++; }
    return { complete, prog, none };
  });
  const followedTotals = $derived.by(() => {
    let done = 0, total = 0, due = 0;
    for (const p of plans.filter((x) => x.followed)) { const c = planCounts(p, facts); done += c.done; total += c.total; due += c.due; }
    return { done, total, due };
  });
  const dueTotal = $derived(followedTotals.due);
  const overallPct = $derived(followedTotals.total ? Math.round((followedTotals.done / followedTotals.total) * 100) : 0);

  let packNote = $state('');
  async function sharePack() {
    packNote = '';
    try { packNote = 'Saved: ' + (await plansExport()) + '. Give this file to someone: they put it in their user\\plans folder and the plans appear.'; }
    catch (e) { packNote = String(e); }
  }
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
  <PageHeader title="Plans" tagline="Build • Prepare • Sustain • Recover" />
  <section class="intro">
    <span class="introic"><Icon name="plans" size={34} /></span>
    <p>The first walkthrough gets you ready. These plans keep going after that: checks that come back on a schedule, things to learn, a plan for each kind of emergency, a guide to keep each thing from failing, and a guide to how much to have. Follow as many as you like at the same time.</p>
    <button class="btn ico big2" onclick={sharePack} title="Put the plans you wrote into one file to give to another household"><Icon name="folder" size={22} /> Share my plans as a pack</button>
  </section>
  {#if error}<div class="warn err" role="alert">{error}</div>{/if}
  {@render whoPick()}
  {#if packNote}<p class="muted small">{packNote}</p>{/if}

  <section class="dbl progp" style="--c: var(--glow-blue)">
    <div class="pg1"><div class="pnl-h nosl">Plan progress</div><div class="bar"><span style="width:{plans.length ? ((planStatus.complete + planStatus.prog) / plans.length) * 100 : 0}%"></span></div></div>
    <div class="pg2">{planStatus.prog} of {plans.length} plans in progress <b>|</b> {planStatus.complete} completed</div>
    <div class="pg3"><Icon name="chart" size={40} /><span class="pnl-h nosl">Plan status</span></div>
    <div class="sb ok"><b>{planStatus.complete}</b><i>Complete</i></div>
    <div class="sb"><b>{planStatus.prog}</b><i>In Progress</i></div>
    <div class="sb grey"><b>{planStatus.none}</b><i>Not Started</i></div>
  </section>
  <div class="libbar">
    <div class="pnl-h nosl"><i class="sl3" aria-hidden="true"></i> Plan library</div>
    <label class="show">Show: <select class="input" bind:value={showKind} aria-label="Show"><option value="all">All Plans</option><option value="following">Following</option><option value="mine">My plans</option><option value="favorites">Favorites</option>{#each GROUPS as [k, t]}<option value={k}>{t.charAt(0) + t.slice(1).toLowerCase()}</option>{/each}</select></label>
    <label class="srch"><Icon name="search" size={18} /><input type="search" placeholder="Search plans…" aria-label="Search plans" bind:value={planQ} /></label>
  </div>
  <div class="cards">
    {#if showKind === 'all' && !planQ.trim()}
    <button class="pc base" onclick={() => go('Start')} aria-label="Open the first walkthrough, Get ready">
      <span class="pthumb" style="background-image:url(/ui/plan-get-ready.jpg)"></span>
      <span class="pbody">
        <span class="ptop"><span class="kind">THE FIRST STEPS</span><span class="pill">Start here</span></span>
        <span class="ctitle">Get ready</span>
        <span class="csum">The nine-step walkthrough: water, food, health, power, where to go, a binder and a drill. Do this first.</span>
        <span class="pfoot"><span class="meta">Opens the walkthrough</span><span class="btn sm2">Open plan <Icon name="chevron" size={14} /></span></span>
      </span>
    </button>
    {/if}
    {#each libraryList as p (p.id)}
      {@const c = planCounts(p, facts)}
      {@const st = c.total > 0 && c.done === c.total ? 'complete' : c.done > 0 ? 'prog' : 'none'}
      <button class="pc" class:on={p.followed} class:okc={st === 'complete'} onclick={() => (openId = p.id)} aria-label={`Open the plan ${p.title}`}>
        <span class="pthumb" style="background-image:url(/ui/plan-{p.id}.jpg), url(/ui/banner.jpg)"></span>
        <span class="pbody">
          <span class="ptop"><span class="kind">{KIND_NAME[p.kind] ?? p.kind}{p.followed ? ' · FOLLOWING' : ''}{p.mine ? ' · MINE' : ''}</span><span class="star" class:on={favs.includes(p.id)} role="button" tabindex="0" aria-label={favs.includes(p.id) ? 'Remove from favorites' : 'Add to favorites'} title="Favorite" onclick={(e) => { e.stopPropagation(); toggleFav(p.id); }} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); toggleFav(p.id); } }}>{favs.includes(p.id) ? '★' : '☆'}</span><span class="pill {st === 'complete' ? 'on' : st === 'none' ? 'off' : ''}">{st === 'complete' ? '✓ Complete' : st === 'prog' ? 'In progress' : 'Not started'}</span></span>
          <span class="ctitle">{p.title}</span>
          {#if p.summary}<span class="csum">{p.summary}</span>{/if}
          <span class="track" class:g={st === 'complete'}><span style="width:{c.total ? (c.done / c.total) * 100 : 0}%"></span></span>
          <span class="pfoot"><span class="meta">{c.done} of {c.total} done{c.due ? ` · ${c.due} due again` : ''}{p.status !== 'reviewed' ? ' · not yet reviewed' : ' · reviewed'}</span><span class="btn sm2">Open plan <Icon name="chevron" size={14} /></span></span>
        </span>
      </button>
    {/each}
  </div>
  <section class="dbl qa" style="--c: var(--glow-blue)">
    <div class="pnl-h nosl"><Icon name="gear" size={28} /> Quick actions</div>
    <button class="btn ico" onclick={() => { mkNote = ''; mkDlg.showModal(); }} title="Write your own plan"><Icon name="plus" size={18} /> Create new plan</button>
    <button class="btn ico" onclick={() => (showKind = 'mine')} title="The plans you wrote"><Icon name="folder" size={18} /> My plans</button>
    <button class="btn ico" onclick={() => go('Calendar')} title="The Calendar shows when plan checks come due"><Icon name="cal" size={18} /> Plan calendar</button>
    <button class="btn ico" onclick={() => dueDlg.showModal()} title="Repeating checks that are due again"><Icon name="check" size={18} /> Plan checks{dueChecks.length ? ` (${dueChecks.length})` : ''}</button>
    <button class="btn ico" onclick={() => (showKind = 'favorites')} title="Plans you starred"><Icon name="star" size={18} /> Favorites</button>
  </section>
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

<dialog bind:this={dueDlg} aria-label="Plan checks">
  <div class="mkform">
    <h2>Plan checks due</h2>
    {#if dueChecks.length === 0}<p>Every repeating check in the plans you follow is up to date.</p>{:else}<p class="muted small">These repeating checks are due again.</p>{/if}
    {#each dueChecks as d (d.plan.id + d.step.id)}
      <div class="duerow"><div><strong>{d.step.title}</strong><div class="muted small">{d.plan.title} · {dueText(d.step)}</div></div><button class="btn" onclick={() => { openId = d.plan.id; dueDlg.close(); }}>Open</button></div>
    {/each}
    <div class="row end"><button class="btn" onclick={() => dueDlg.close()}>Close</button></div>
  </div>
</dialog>

<dialog bind:this={mkDlg} aria-label="Create a plan">
  <form class="mkform" onsubmit={(e) => { e.preventDefault(); createPlan(); }}>
    <h2>Create a plan</h2>
    <p class="muted small">A plan is a list of steps. A step can come back on a schedule. It is saved as a plain file in your own plans folder, and it shows in the library with the others.</p>
    <label>Name<input class="input" bind:value={mk.title} required placeholder="Cabin ready for winter" /></label>
    <label>What it is for<input class="input" bind:value={mk.summary} placeholder="One sentence" /></label>
    <div class="row"><label class="half">Kind<select class="input" bind:value={mk.kind}><option value="checks">{KIND_NAME.checks ?? 'Repeating checks'}</option><option value="learning">{KIND_NAME.learning ?? 'Learning'}</option><option value="scenario">{KIND_NAME.scenario ?? 'Emergency plan'}</option></select></label><label class="half">Your name<input class="input" bind:value={mk.author} /></label></div>
    {#each mk.steps as st, n}
      <fieldset class="mkstep">
        <legend>Step {n + 1}</legend>
        <label>Title<input class="input" bind:value={st.title} /></label>
        <label>Comes back every how many days? (leave empty for a one-time step)<input class="input" type="number" min="1" bind:value={st.every} /></label>
        <label>Notes, one line each<textarea class="input" rows="2" bind:value={st.body}></textarea></label>
        {#if mk.steps.length > 1}<button class="btn" type="button" onclick={() => (mk.steps = mk.steps.filter((_, i) => i !== n))}>Remove this step</button>{/if}
      </fieldset>
    {/each}
    <button class="btn" type="button" onclick={() => (mk.steps = [...mk.steps, { title: '', every: '', body: '' }])}>Add a step</button>
    <p class="muted small">Do not put medicine doses in a plan. Follow your doctor's written plan.</p>
    {#if mkNote}<div class="warn" role="alert">{mkNote}</div>{/if}
    <div class="row end"><button class="btn" type="button" onclick={() => mkDlg.close()}>Cancel</button><button class="btn primary" type="submit">Save plan</button></div>
  </form>
</dialog>

<style>
  .star { font-size: 20px; line-height: 1; color: #8da2b8; cursor: pointer; padding: 0 4px; }
  .star.on { color: var(--glow-amber); text-shadow: 0 0 8px var(--glow-amber); }
  .mkform { display: flex; flex-direction: column; gap: 10px; padding: 18px; min-width: min(560px, 92vw); max-height: 86vh; overflow: auto; }
  .mkform label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--muted); }
  .mkform h2 { margin: 0; }
  .mkstep { border: 1px solid #24456b; border-radius: 10px; padding: 10px 12px; display: flex; flex-direction: column; gap: 8px; margin: 0; }
  .mkstep legend { color: var(--glow-blue); padding: 0 6px; }
  .half { flex: 1 1 140px; }
  .end { justify-content: flex-end; }
  .duerow { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 10px 0; border-top: 1px solid #1d3550; }
  .intro { display: flex; align-items: center; gap: 16px; }
  .introic { flex: none; width: 54px; height: 54px; display: inline-flex; align-items: center; justify-content: center; border: 1.5px solid #2a4a70; border-radius: 12px; color: var(--glow-blue); background: #0b1520; }
  .intro p { margin: 0; flex: 1; color: #e6edf5; }
  .big2 { padding: 14px 20px; border-color: var(--glow-blue); box-shadow: 0 0 14px rgba(61, 155, 255, .3); }
  .progp { display: grid; grid-template-columns: 1.4fr auto auto repeat(3, 108px); gap: 14px; align-items: center; padding: 14px 18px; }
  .progp .pnl-h { margin: 0 0 6px; }
  .bar { height: 12px; border-radius: 7px; background: #1b2735; overflow: hidden; }
  .bar span { display: block; height: 100%; background: linear-gradient(90deg, #2f86e8, #4db0ff); box-shadow: 0 0 10px #3d9bff; }
  .pg2 { text-align: right; color: #e6edf5; }
  .pg2 b { color: #4a6a8a; margin: 0 8px; font-weight: 400; }
  .pg3 { display: flex; align-items: center; gap: 10px; padding-left: 18px; border-left: 1px solid #24456b; color: var(--glow-blue); }
  .sb { text-align: center; padding: 8px 4px; border: 1.5px solid #4db0ff; border-radius: 10px; background: #0b121b; }
  .sb.ok { border-color: var(--glow-green); background: rgba(67, 224, 107, .08); }
  .sb.grey { border-color: #8da2b8; }
  .sb b { display: block; font-size: 28px; line-height: 1; }
  .sb i { font-style: normal; font-size: 13px; color: #e6edf5; }
  @media (max-width: 1000px) { .progp { grid-template-columns: repeat(3, 1fr); } .pg1, .pg2, .pg3 { grid-column: span 3; text-align: left; border: 0; padding: 0; } }
  .libbar { display: flex; flex-wrap: wrap; align-items: center; gap: 14px; }
  .libbar .pnl-h { flex: 1 1 200px; margin: 0; }
  .sl3 { display: inline-block; width: 54px; height: 16px; background: repeating-linear-gradient(115deg, #2f86e8 0 10px, transparent 10px 15px); }
  .show { display: inline-flex; align-items: center; gap: 8px; color: #e6edf5; }
  .srch { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 40px; border: 1px solid #2a4a70; border-radius: 10px; background: #0a121b; color: #9fb4c9; min-width: 280px; }
  .srch input { flex: 1; background: transparent; border: 0; outline: 0; color: var(--text); font: inherit; }
  .pc { display: flex; min-height: 172px; text-align: left; padding: 0; border: 1.5px solid #24456b; border-radius: 12px; overflow: hidden; background: linear-gradient(135deg, #0e1824, #0a1018); color: var(--text); box-shadow: 0 0 14px rgba(61, 155, 255, .12); }
  .pc:hover { border-color: #4db0ff; }
  .pc.on, .pc.base { border-color: #4db0ff; box-shadow: 0 0 20px rgba(61, 155, 255, .4); }
  .pc.okc { border-color: #2c8a4a; }
  .pthumb { flex: 0 0 36%; max-width: 150px; background-size: cover; background-position: center; border-right: 1px solid #1d3550; }
  .pbody { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 6px; padding: 10px 14px 12px; }
  .ptop { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
  .ptop .pill { min-width: 0; padding: 2px 10px; font-size: 10px; }
  .pfoot { display: flex; justify-content: space-between; align-items: center; gap: 8px; margin-top: auto; }
  .sm2 { display: inline-flex; align-items: center; gap: 4px; padding: 6px 12px; border: 1.5px solid #cfd8e3; border-radius: 9px; font-family: var(--mono); font-size: 11px; letter-spacing: .08em; text-transform: uppercase; color: #fff; white-space: nowrap; }
  .track.g span { background: #43e06b; box-shadow: 0 0 8px #43e06b; }
  .cards { grid-template-columns: repeat(auto-fill, minmax(380px, 1fr)); }
  .qa { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; padding: 10px 16px; }
  .qa .pnl-h { margin: 0 14px 0 0; }
  .qa .pnl-h::after { display: none; }
  .qa .btn { flex: 1 1 150px; justify-content: center; border-color: #3a6fa8; }
  .forwho { display: inline-flex; gap: 8px; align-items: center; margin: 4px 0; }
  .gh { margin-top: 18px; font-family: 'Barlow Condensed', 'Arial Narrow', var(--display); font-size: 19px; letter-spacing: .08em; color: var(--text); }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 14px; margin-top: 12px; }
  .card { display: flex; flex-direction: column; gap: 8px; text-align: left; padding: 16px; background: var(--panel); border: 1px solid var(--line); border-top: 2px solid var(--line2); border-radius: 12px; }
  .card { box-shadow: 0 0 18px rgba(103, 180, 255, .06); min-height: 170px; }
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
  .step.due { border-top-color: var(--warn); }
  .steptop { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px; }
  .stitle { flex: 1 1 200px; font-size: 17px; letter-spacing: .05em; margin: 0; }
  .box { flex: 0 0 34px; height: 34px; border-radius: 50%; border: 1px solid var(--line2); background: transparent; color: var(--muted); font-size: 14px; }
  .box:hover:not(:disabled) { border-color: var(--blue); color: var(--blue); }
  .box.done { background: var(--ok-bg); border-color: var(--ok-line); color: var(--ok); }
  .box.due { background: var(--warn-soft); border-color: var(--warn-line); color: var(--warn); }
  .duetext { color: var(--warn); margin: 8px 0 0; }
  .body { margin: 8px 0 0; }
  .li { margin: 4px 0 0 18px; }
  .li::before { content: '• '; color: var(--muted); }
  .small { font-size: 13px; }
  .err { border-color: var(--danger-line); background: var(--danger-bg); color: var(--danger-text); }
  .probs { margin-top: 14px; }
</style>
