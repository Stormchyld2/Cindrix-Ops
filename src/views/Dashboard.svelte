<script lang="ts">
  import { onMount } from 'svelte';
  import { whatFailsNext, inventoryList, drillsList, mapPacks, modelStatus, listModels, libraryStatus, dashboardSuggestions, plansList, academyList, familyStatus, type Cascade, type Suggestion, type PlanInfo, type AcademyModule } from '../lib/api';
  import { gatherFacts, planCounts, nextStep, stepStatus, KIND_NAME, type PlanFacts } from '../lib/planfacts';
  import { categories } from '../lib/categories';
  import { settings } from '../lib/settings.svelte';

  let { go }: { go: (tab: string) => void } = $props();

  let data: Cascade | null = $state(null);
  let items = $state(0);
  let tips: Suggestion[] = $state([]);
  let drillsDone = $state(0);
  let drillsTotal = $state(0);
  let error = $state('');
  let loaded = $state(false);
  // more of the picture: plans being followed, learning, supplies by kind, and whether the drive is ready
  let plans: PlanInfo[] = $state([]);
  let facts: PlanFacts = $state({ checks: {}, lessons: new Set<string>() });
  let academy: AcademyModule[] = $state([]);
  let cats: Record<string, number> = $state({});
  let ready: { label: string; ok: boolean; tab: string; text: string }[] = $state([]);

  onMount(async () => {
    try {
      data = await whatFailsNext();
      items = (await inventoryList()).length;
      const dr = await drillsList();
      drillsTotal = dr.length;
      drillsDone = dr.filter((d) => d.times_done > 0).length;
      let maps = 0, models = 0, ready = false, archives = 0;
      try { maps = (await mapPacks()).length; } catch { /* counts as none */ }
      try { models = (await listModels()).length; ready = (await modelStatus()).state === 'ready'; } catch { /* counts as none */ }
      try { archives = (await libraryStatus()).archives.length; } catch { /* counts as none */ }
      tips = await dashboardSuggestions({ maps, models, assistant_ready: ready, archives, drills_done: drillsDone, drills_total: drillsTotal });
    } catch (e) {
      error = String(e);
    }
    // These never block the rest of the page: a failure just leaves that panel empty.
    try { plans = await plansList(); facts = await gatherFacts(); } catch { /* no plans panel content */ }
    try { academy = await academyList(); } catch { /* none */ }
    try {
      const c: Record<string, number> = {};
      for (const i of await inventoryList()) c[i.category] = (c[i.category] ?? 0) + 1;
      cats = c;
    } catch { /* none */ }
    try {
      const f = (await familyStatus()).exists;
      ready = [
        { label: 'Family page', ok: f, tab: 'Family', text: f ? 'Filled in' : 'Not filled in yet' },
        { label: 'Assistant', ok: facts.checks.model, tab: 'Settings', text: facts.checks.model ? 'A model file is ready' : 'No model file yet' },
        { label: 'Library', ok: facts.checks.library, tab: 'Library', text: facts.checks.library ? 'Archives found' : 'No archives found yet' },
        { label: 'Maps', ok: facts.checks.maps, tab: 'Maps', text: facts.checks.maps ? 'A map file is on the drive' : 'No map file yet' },
      ];
    } catch { /* none */ }
    loaded = true;
  });
  const followed = $derived(plans.filter((p) => p.followed));
  const checksDue = $derived(followed.reduce((n, p) => n + p.steps.filter((s) => stepStatus(s, facts) === 'due').length, 0));
  const lessons = $derived(academy.flatMap((m) => m.lessons));
  const lessonsDone = $derived(lessons.filter((l) => l.tries > 0).length);
  const nextLesson = $derived(lessons.find((l) => l.tries === 0));
  const emptyCats = $derived(categories.filter((c) => !cats[c]));

  function fmt(d: number | null | undefined): string {
    return d == null ? 'Not set' : d < 10 ? d.toFixed(1) + ' days' : Math.round(d) + ' days';
  }
  function tone(d: number): string {
    return d < settings.values.planActDays ? 'act' : d < settings.values.planCheckDays ? 'check' : 'ok';
  }
  const low = $derived.by(() => (data?.runs_out ?? []).filter((r) => r.days < settings.values.planCheckDays).slice(0, 6));
  const past = $derived.by(() => (data?.expiring ?? []).filter((e) => e.months < 0).length);
  const label: Record<string, string> = { urgent: 'Do now', soon: 'Soon', next: 'Next', good: 'All good' };
  const today = new Date().toLocaleDateString(undefined, { weekday: 'long', month: 'long', day: 'numeric' });
</script>

<section>
  <div class="row"><h1>Dashboard</h1><span class="muted">{today}</span></div>
  <p class="sub">Where you stand today, worked out from what you have filled in. It is an estimate, not a prediction.</p>
</section>

{#if error}<div class="warn" role="alert">{error}</div>{/if}

{#if data}
  <div class="tiles">
    <button class="tile" onclick={() => go('Inventory')}><div class="label">WATER</div><div class="big {data.water_days == null ? '' : tone(data.water_days)}">{fmt(data.water_days)}</div></button>
    <button class="tile" onclick={() => go('Inventory')}><div class="label">FOOD</div><div class="big {data.food_days == null ? '' : tone(data.food_days)}">{fmt(data.food_days)}</div></button>
    <button class="tile" onclick={() => go('Inventory')}><div class="label">RUNNING LOW</div><div class="big {low.length ? 'check' : 'ok'}">{low.length} item{low.length === 1 ? '' : 's'}</div></button>
    <button class="tile" onclick={() => go('Inventory')}><div class="label">PAST OR NEAR THEIR DATE</div><div class="big {past ? 'act' : data.expiring.length ? 'check' : 'ok'}">{data.expiring.length} item{data.expiring.length === 1 ? '' : 's'}</div></button>
    <button class="tile" onclick={() => go('What next')}><div class="label">PLAN CHECKS DUE</div><div class="big {checksDue ? 'check' : followed.length ? 'ok' : ''}">{followed.length ? checksDue : 'None followed'}</div></button>
    <button class="tile" onclick={() => go('Drills')}><div class="label">DRILLS DONE</div><div class="big {drillsDone ? 'ok' : 'check'}">{drillsDone} of {drillsTotal}</div></button>
  </div>
{/if}

<div class="cols">
  <section class="panel">
    <div class="label">WHAT TO DO NEXT</div>
    {#if loaded && !tips.length}<p class="muted">Nothing to suggest right now.</p>{/if}
    {#each tips as t}
      <div class="tip">
        <span class="chip {t.level}">{label[t.level]}</span>
        <div class="grow">
          <strong>{t.title}</strong>
          <div class="muted small">{t.text}</div>
        </div>
        {#if t.tab}<button class="btn" onclick={() => go(t.tab!)}>Open</button>{/if}
      </div>
    {/each}
    <p class="muted small foot">These come from what is on this device. In an emergency, call 911.</p>
  </section>

  <section class="panel">
    <div class="label">RUNNING LOW</div>
    {#if data && items === 0}<p class="muted">The Inventory is empty. <button class="link" onclick={() => go('Inventory')}>Add your supplies</button>.</p>
    {:else if data && data.runs_out.length === 0}<p class="muted">No item has a "used per day" amount yet, so nothing can be counted down.</p>
    {:else if data && low.length === 0}<p class="muted">Nothing runs out within {settings.values.planCheckDays} days.</p>{/if}
    {#each low as r}
      <div class="line">
        <div class="grow"><strong>{r.name}</strong><div class="muted small">{r.location}</div></div>
        <div class={tone(r.days)}>{fmt(r.days)}</div>
      </div>
    {/each}
    {#if data && data.expiring.length}
      <div class="label gap">PAST OR NEAR THEIR DATE</div>
      {#each data.expiring.slice(0, 5) as e}
        <div class="line">
          <div class="grow"><strong>{e.name}</strong></div>
          <div class={e.months < 0 ? 'act' : 'check'}>{e.months < 0 ? 'Past its date' : e.months === 0 ? 'This month' : 'In ' + e.months + ' month' + (e.months === 1 ? '' : 's')}</div>
        </div>
      {/each}
    {/if}
    <div class="row gap"><button class="btn" onclick={() => go('Inventory')}>Open Inventory</button><button class="btn" onclick={() => go('What fails')}>What fails next</button></div>
  </section>
</div>

<div class="cols more">
  <section class="panel">
    <div class="label">YOUR PLANS</div>
    {#if followed.length === 0}
      <p class="muted">You are not following any plan yet. Plans keep going after the first walkthrough: repeating checks, learning and emergency plans.</p>
    {/if}
    {#each followed as p (p.id)}
      {@const c = planCounts(p, facts)}
      {@const n = nextStep(p, facts)}
      <div class="line">
        <div class="grow">
          <strong>{p.title}</strong> <span class="muted small">{KIND_NAME[p.kind] ?? p.kind}</span>
          <div class="muted small">{c.done} of {c.total} done{c.due ? ` · ${c.due} due again` : ''}{n ? ` · next: ${n.title}` : ' · all done for now'}</div>
        </div>
        <button class="btn" onclick={() => go('What next')}>Open</button>
      </div>
    {/each}
    <div class="row gap"><button class="btn" onclick={() => go('What next')}>{followed.length ? 'All plans' : 'Choose plans to follow'}</button><button class="btn" onclick={() => go('Start')}>The first walkthrough</button></div>
  </section>

  <section class="panel">
    <div class="label">LEARNING</div>
    {#if lessons.length === 0}<p class="muted">No lessons found.</p>
    {:else}
      <p>{lessonsDone} of {lessons.length} lessons done.</p>
      <div class="meter"><span style="width:{(lessonsDone / lessons.length) * 100}%"></span></div>
      {#if nextLesson}<p class="muted small">Next lesson: <strong>{nextLesson.title}</strong> ({nextLesson.minutes} min)</p>{/if}
    {/if}
    <div class="row gap"><button class="btn" onclick={() => go('Academy')}>Open the Academy</button></div>
  </section>

  <section class="panel">
    <div class="label">SUPPLIES BY KIND</div>
    {#if items === 0}<p class="muted">Nothing counted yet.</p>{/if}
    <div class="catgrid">
      {#each categories as c}
        <div class="catcell" class:none={!cats[c]}><span>{c}</span><span class="muted">{cats[c] ?? 0}</span></div>
      {/each}
    </div>
    {#if items > 0 && emptyCats.length}<p class="muted small">Nothing yet in: {emptyCats.slice(0, 5).join(', ')}{emptyCats.length > 5 ? ' and more' : ''}.</p>{/if}
    <div class="row gap"><button class="btn" onclick={() => go('Inventory')}>Open Inventory</button></div>
  </section>

  <section class="panel">
    <div class="label">IS THE DRIVE READY?</div>
    {#each ready as r}
      <div class="line">
        <span class="dotmark" class:ok={r.ok}>{r.ok ? '✓' : '!'}</span>
        <div class="grow"><strong>{r.label}</strong><div class="muted small">{r.text}</div></div>
        {#if !r.ok}<button class="btn" onclick={() => go(r.tab)}>Fix</button>{/if}
      </div>
    {/each}
    <div class="label gap">GO TO</div>
    <div class="row quick">
      {#each ['Ask', 'Library', 'Intel', 'Maps', 'Music', 'Binder'] as t}<button class="btn" aria-label={'Go to ' + t} onclick={() => go(t)}>{t}</button>{/each}
    </div>
  </section>
</div>

<style>
  .more { margin-top: 12px; }
  .meter { height: 8px; border-radius: 4px; background: var(--line); overflow: hidden; }
  .meter span { display: block; height: 100%; background: var(--blue); }
  .catgrid { display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); gap: 6px 14px; }
  .catcell { display: flex; justify-content: space-between; gap: 8px; padding: 4px 0; border-bottom: 1px solid var(--line); font-size: 13px; }
  .catcell.none span:first-child { color: var(--muted); }
  .dotmark { flex: 0 0 24px; height: 24px; border-radius: 50%; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--line2); color: var(--accent); font-size: 12px; }
  .dotmark.ok { background: #1f2a1d; border-color: #34502f; color: var(--ok); }
  .quick { flex-wrap: wrap; gap: 8px; }
  .tiles { display: flex; flex-wrap: wrap; gap: 12px; margin-bottom: 12px; }
  .tile { flex: 1 1 170px; padding: 14px; text-align: left; background: var(--panel); border: 1px solid var(--line); border-radius: 12px; }
  .tile:hover { border-color: var(--blue-line); }
  .big { font-size: 24px; font-weight: 600; margin-top: 2px; }
  .cols { display: grid; grid-template-columns: repeat(auto-fit, minmax(340px, 1fr)); gap: 12px; align-items: start; }
  .tip { display: flex; flex-wrap: wrap; gap: 8px 12px; align-items: center; padding: 12px 0; border-top: 1px solid var(--line); margin-top: 6px; }
  .chip { flex: 0 0 auto; min-width: 66px; text-align: center; padding: 2px 8px; border-radius: 999px; font-family: var(--mono); font-size: 11px; text-transform: uppercase; letter-spacing: .08em; border: 1px solid var(--line2); }
  .chip.urgent { color: var(--danger); border-color: #6b2a20; background: #241512; }
  .chip.soon { color: var(--accent); border-color: var(--accent-line); background: var(--accent-soft); }
  .chip.next { color: var(--blue); border-color: var(--blue-line); background: var(--blue-soft); }
  .chip.good { color: var(--ok); border-color: #34502f; background: #1f2a1d; }
  .line { display: flex; gap: 12px; padding: 10px 0; border-top: 1px solid var(--line); align-items: center; }
  .grow { flex: 1 1 200px; }
  .small { font-size: 13px; }
  .foot { margin-bottom: 0; }
  .gap { margin-top: 12px; }
  .link { background: none; border: 0; color: var(--blue); padding: 0; text-decoration: underline; }
</style>
