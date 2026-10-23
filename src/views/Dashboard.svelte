<script lang="ts">
  import { onMount } from 'svelte';
  import { inShare, whatFailsNext, inventoryList, drillsList, mapPacks, modelStatus, listModels, libraryStatus, shareInfo, shareSet, dashboardSuggestions, plansList, academyList, familyStatus, productInfo, type ProductInfo, type ShareInfo, type Cascade, type Suggestion, type PlanInfo, type AcademyModule } from '../lib/api';
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
  let product: ProductInfo | null = $state(null);
  let sharing: ShareInfo = $state({ on: false, auto: true, reason: '', pin: '', port: 8765, addresses: [], hotspot_up: false, signed_in: 0, trusted: 0 });
  // the Sharing panel follows the real state, so it changes by itself when a hotspot starts
  let shareBusy = $state(false);
  let shareError = $state('');
  async function refreshSharing() { try { sharing = await shareInfo(); } catch { /* the panel just shows Off */ } }
  async function toggleSharing() {
    if (shareBusy) return;
    shareBusy = true; shareError = '';
    try { sharing = await shareSet(!sharing.on); } catch (e) { shareError = String(e); }
    shareBusy = false;
  }
  onMount(() => {
    void refreshSharing();
    const timer = setInterval(refreshSharing, 4000);
    return () => clearInterval(timer);
  });
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
    try { product = await productInfo(); } catch { /* every tile is shown */ }
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
      let drive: { name: string; edition: string } | null = null;
      try { drive = (await libraryStatus()).drive ?? null; } catch { /* shown as not found */ }
      ready = [
        { label: 'Drive', ok: !!drive, tab: 'Settings', text: drive ? drive.name + ' (' + drive.edition + ')' : 'No Cindrix drive found, so the program folder is being used' },
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

  function fmt(d: number | null | undefined): string {
    return d == null ? 'Not set' : d < 10 ? d.toFixed(1) + ' days' : Math.round(d) + ' days';
  }
  function tone(d: number): string {
    return d < settings.values.planActDays ? 'act' : d < settings.values.planCheckDays ? 'check' : 'ok';
  }
  // plans with checks due come first; the panel has room for three
  const topPlans = $derived([...followed].sort((a, b) => planCounts(b, facts).due - planCounts(a, facts).due).slice(0, 3));
  // what is wrong comes first
  const readyFirst = $derived([...ready].sort((a, b) => Number(a.ok) - Number(b.ok)));
  const low = $derived.by(() => (data?.runs_out ?? []).filter((r) => r.days < settings.values.planCheckDays).slice(0, 6));
  const past = $derived.by(() => (data?.expiring ?? []).filter((e) => e.months < 0).length);
  // the pop-up behind each status tile: what the number is made of, and where to fix it
  type Detail = 'water' | 'food' | 'low' | 'date' | 'checks' | 'drills';
  let detail: Detail | null = $state(null);
  const lowAll = $derived.by(() => (data?.runs_out ?? []).filter((r) => r.days < settings.values.planCheckDays));
  const dueSteps = $derived(followed.flatMap((p) => p.steps.filter((st) => stepStatus(st, facts) === 'due').map((st) => ({ plan: p.title, step: st.title }))));
  const detailTitle: Record<Detail, string> = { water: 'Water', food: 'Food', low: 'Running low', date: 'Past or near their date', checks: 'Plan checks due', drills: 'Drills done' };
  const detailGo: Record<Detail, { tab: string; text: string }> = {
    water: { tab: 'Inventory', text: 'Open Inventory' }, food: { tab: 'Inventory', text: 'Open Inventory' },
    low: { tab: 'Inventory', text: 'Open Inventory' }, date: { tab: 'Inventory', text: 'Open Inventory' },
    checks: { tab: 'What next', text: 'Open Plans' }, drills: { tab: 'Drills', text: 'Open Drills' },
  };
  function openDetail(d: Detail) { detail = d; }
  function goDetail() { const d = detail; detail = null; if (d) go(detailGo[d].tab); }
  const label: Record<string, string> = { urgent: 'Do now', soon: 'Soon', next: 'Next', good: 'All good' };
  // The shortcut tiles. Every one leads to a screen that exists; a tile is left out when this edition does not have that part.
  const ICONS: Record<string, string> = {
    library: 'M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2V5zM4 19a2 2 0 0 1 2-2h13',
    maps: 'M9 4 3 6v14l6-2 6 2 6-2V4l-6 2-6-2zM9 4v14M15 6v14',
    ask: 'M4 5h16v11H9l-5 4V5z',
    intel: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18',
    academy: 'M2 9l10-5 10 5-10 5L2 9zM6 11v5c3 2 9 2 12 0v-5',
    plans: 'M9 6h11M9 12h11M9 18h11M3.5 6l1 1 2-2M3.5 12l1 1 2-2M3.5 18l1 1 2-2',
    drills: 'M12 8v5l3 2M9 2h6M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16z',
    inventory: 'M3 7l9-4 9 4v10l-9 4-9-4V7zM3 7l9 4 9-4M12 11v10',
    medical: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 8v8M8 12h8',
    family: 'M8 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM2 20c0-3 3-5 6-5s6 2 6 5M17 11a2.5 2.5 0 1 0 0-5M16 15c3 0 6 1.5 6 5',
    binder: 'M6 3h12v18H6zM9 3v18M12 8h4M12 12h4',
    seeds: 'M12 21V10M12 10c0-4 3-6 7-6 0 4-3 6-7 6zM12 14c0-3-2-5-6-5 0 3 2 5 6 5z',
    agro: 'M12 21V9M12 9c0-4 3-6 7-6 0 4-3 6-7 6zM12 13c0-3-2-5-6-5 0 3 2 5 6 5zM5 21h14',
    comms: 'M5 12a7 7 0 0 1 14 0M8 12a4 4 0 0 1 8 0M12 12v8M9 20h6',
    music: 'M9 18V6l11-2v12M9 18a3 3 0 1 1-3-3 3 3 0 0 1 3 3zM20 16a3 3 0 1 1-3-3 3 3 0 0 1 3 3z',
  };
  const tiles = $derived.by(() => {
    const p = product;
    const all: { icon: string; label: string; tab: string; show: boolean }[] = [
      { icon: 'library', label: 'Library', tab: 'Shelf', show: p ? p.library : true },
      { icon: 'maps', label: 'Maps', tab: 'Maps', show: p ? p.maps : true },
      { icon: 'ask', label: 'Ask', tab: 'Ask', show: p ? p.assistant : true },
      { icon: 'intel', label: 'Intel', tab: 'Intel', show: p ? p.library : true },
      { icon: 'academy', label: 'Academy', tab: 'Academy', show: true },
      { icon: 'medical', label: 'Emergency cards', tab: 'Cards', show: p ? p.library : true },
      { icon: 'plans', label: 'Plans', tab: 'What next', show: p ? p.plan : true },
      { icon: 'drills', label: 'Drills', tab: 'Drills', show: p ? p.plan : true },
      { icon: 'inventory', label: 'Inventory', tab: 'Inventory', show: p ? p.plan : true },
      { icon: 'family', label: 'Family', tab: 'Family', show: p ? p.plan : true },
      { icon: 'agro', label: 'Agro', tab: 'guide:crop-guides', show: p ? p.plan : true },
      { icon: 'comms', label: 'Comms', tab: 'CommsPlan', show: !inShare },
      { icon: 'seeds', label: 'Seed vault', tab: 'Seed vault', show: p ? p.plan : true },
      { icon: 'binder', label: 'Binder', tab: 'Binder', show: p ? p.plan : true },
      { icon: 'music', label: 'Music', tab: 'Music', show: p ? p.music : true },
    ];
    return all.filter((t) => t.show);
  });
  const today = new Date().toLocaleDateString(undefined, { weekday: 'long', month: 'long', day: 'numeric' });
</script>

<section class="hero">
  <div class="herotop">
    <img class="herologo" src="/cindrix-logo.png" alt="" width="64" height="64" />
    <div class="grow">
      <h1>Dashboard</h1>
      <div class="muted small">{product?.name ?? 'Cindrix Ops'} · {today}</div>
    </div>
  </div>
  <nav class="shortcuts" aria-label="Shortcuts">
    {#each tiles as t (t.label)}
      <button class="sc" aria-label={'Open ' + t.label} title={'Open ' + t.label} onclick={() => go(t.tab)}>
        <svg viewBox="0 0 24 24" width="30" height="30" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={ICONS[t.icon]} /></svg>
        <span>{t.label}</span>
      </button>
    {/each}
  </nav>
</section>
<p class="sub">Where you stand today, worked out from what you have filled in. It is an estimate, not a prediction.</p>

{#if error}<div class="warn" role="alert">{error}</div>{/if}

{#if data}
  <div class="tiles">
    <button class="tile" style="--c: var(--cat-intel)" onclick={() => openDetail('water')} aria-haspopup="dialog"><div class="label">WATER</div><div class="big {data.water_days == null ? '' : tone(data.water_days)}">{fmt(data.water_days)}</div></button>
    <button class="tile" style="--c: var(--cat-ops)" onclick={() => openDetail('food')} aria-haspopup="dialog"><div class="label">FOOD</div><div class="big {data.food_days == null ? '' : tone(data.food_days)}">{fmt(data.food_days)}</div></button>
    <button class="tile" style="--c: var(--cat-ops)" onclick={() => openDetail('low')} aria-haspopup="dialog"><div class="label">RUNNING LOW</div><div class="big {low.length ? 'check' : 'ok'}">{low.length} item{low.length === 1 ? '' : 's'}</div></button>
    <button class="tile" style="--c: var(--cat-ops)" onclick={() => openDetail('date')} aria-haspopup="dialog"><div class="label">PAST OR NEAR DATE</div><div class="big {past ? 'act' : data.expiring.length ? 'check' : 'ok'}">{data.expiring.length} item{data.expiring.length === 1 ? '' : 's'}</div></button>
    <button class="tile" style="--c: var(--cat-ops)" onclick={() => openDetail('checks')} aria-haspopup="dialog"><div class="label">PLAN CHECKS DUE</div><div class="big {checksDue ? 'check' : followed.length ? 'ok' : ''}">{followed.length ? checksDue : 'None'}</div></button>
    <button class="tile" style="--c: var(--cat-ops)" onclick={() => openDetail('drills')} aria-haspopup="dialog"><div class="label">DRILLS DONE</div><div class="big {drillsDone ? 'ok' : 'check'}">{drillsDone} of {drillsTotal}</div></button>
  </div>
{/if}

<div class="dgrid">
  <section class="panel {inShare ? 's12' : 's8'} h1" style="--c: var(--accent)">
    <div class="label">NEEDS YOUR ATTENTION</div>
    <div class="scroll">
      {#if loaded && !tips.length}<p class="muted">Nothing to suggest right now.</p>{/if}
      {#each tips.slice(0, 3) as t}
        <div class="tip">
          <span class="chip {t.level}">{label[t.level]}</span>
          <div class="grow">
            <strong>{t.title}</strong>
            <div class="muted small">{t.text}</div>
          </div>
          {#if t.tab}<button class="btn" onclick={() => go(t.tab!)}>Open</button>{/if}
        </div>
      {/each}
      {#if tips.length > 3}<p class="muted small">{tips.length - 3} more come after these. Do these first.</p>{/if}
    </div>
    <p class="muted small foot">These come from what is on this device. In an emergency, call 911.</p>
  </section>

  {#if !inShare}
  <section class="panel s4 h1 sharing" style="--c: var(--cat-comms)">
    <div class="label">SHARING</div>
    <div class="scroll">
      <div class="line"><div class="grow">Sharing with other devices</div><span class="chip {sharing.on ? 'good' : 'soon'}">{sharing.on ? 'On' : 'Off'}</span></div>
      <div class="line"><div class="grow">Hotspot</div><span class="chip {sharing.hotspot_up ? 'good' : 'soon'}">{sharing.hotspot_up ? 'On' : 'Off'}</span></div>
      <div class="line"><div class="grow">Guests signed in</div><span>{sharing.signed_in}</span></div>
      <div class="line"><div class="grow">Family devices</div><span>{sharing.trusted}</span></div>
    </div>
    {#if shareError}<p class="warn small" role="alert">{shareError}</p>{/if}
    <div class="row gap">
      <button class="btn primary" disabled={shareBusy} onclick={toggleSharing} title={sharing.on ? 'Stop sharing. Other devices lose the connection.' : 'Start sharing now, if it did not start by itself'}>{sharing.on ? 'Turn sharing off' : 'Turn sharing on'}</button>
      <button class="btn" onclick={() => go('settings:Sharing')} title="The PIN, the code to scan and family invites">PIN and codes</button>
    </div>
  </section>
  {/if}

  <section class="panel s6 h2" style="--c: var(--cat-ops)">
    <div class="label">INVENTORY</div>
    {#if data && items === 0}<p class="muted small">The Inventory is empty. <button class="link" onclick={() => go('Inventory')}>Add your supplies</button>.</p>
    {:else if data && data.runs_out.length === 0}<p class="muted small">No item has a "used per day" amount yet, so nothing can be counted down.</p>
    {:else if data && low.length === 0}<p class="muted small">Nothing runs out within {settings.values.planCheckDays} days.</p>
    {:else if data}<p class="small"><strong>{low.length} item{low.length === 1 ? '' : 's'}</strong> run low within {settings.values.planCheckDays} days. Click Running low above to see which.</p>{/if}
    <div class="scroll">
      <div class="catgrid">
        {#each categories as c}
          <div class="catcell" class:none={!cats[c]} title={c}><span class="cn">{c}</span><span class="cc">{cats[c] ?? 0}</span></div>
        {/each}
      </div>
    </div>
    <div class="row gap"><button class="btn" onclick={() => go('Inventory')}>Open Inventory</button><button class="btn" onclick={() => go('What fails')}>What fails next</button></div>
  </section>

  <section class="panel s6 h2" style="--c: var(--cat-ops)">
    <div class="label">PLANS YOU FOLLOW</div>
    <div class="scroll">
      {#if followed.length === 0}
        <p class="muted">You are not following any plan yet. Plans keep going after the first walkthrough: repeating checks, learning and emergency plans.</p>
      {/if}
      {#each topPlans as p (p.id)}
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
      {#if followed.length > topPlans.length}<p class="muted small">{followed.length - topPlans.length} more plan{followed.length - topPlans.length === 1 ? '' : 's'} in All plans. The ones with checks due come first.</p>{/if}
    </div>
    <div class="row gap"><button class="btn" onclick={() => go('What next')}>{followed.length ? 'All plans' : 'Choose plans to follow'}</button><button class="btn" onclick={() => go('Start')}>The first walkthrough</button></div>
  </section>

  <section class="panel s6 h3" style="--c: var(--cat-academy)">
    <div class="label">ACADEMY</div>
    <div class="scroll">
      {#if lessons.length === 0}<p class="muted">No lessons found.</p>
      {:else}
        <p>{lessonsDone} of {lessons.length} lessons done.</p>
        <div class="meter"><span style="width:{(lessonsDone / lessons.length) * 100}%"></span></div>
        {#if nextLesson}<p class="muted small">Next lesson: <strong>{nextLesson.title}</strong> ({nextLesson.minutes} min)</p>{/if}
      {/if}
    </div>
    <div class="row gap"><button class="btn" onclick={() => go('Academy')}>Open the Academy</button></div>
  </section>

  <section class="panel s6 h3" style="--c: var(--cat-intel)">
    <div class="label">IS THIS DRIVE READY?</div>
    <div class="scroll">
      {#each readyFirst as r}
        <div class="line">
          <span class="dotmark" class:ok={r.ok}>{r.ok ? '✓' : '!'}</span>
          <div class="grow"><strong>{r.label}</strong><div class="muted small">{r.text}</div></div>
          {#if !r.ok}<button class="btn" onclick={() => go(r.tab)}>Fix</button>{/if}
        </div>
      {/each}
    </div>
  </section>
</div>

<svelte:window onkeydown={(e) => { if (e.key === 'Escape') detail = null; }} />

{#if detail}
  <dialog open aria-label={detailTitle[detail]}>
    <div class="dlg">
      <h2>{detailTitle[detail]}</h2>
      {#if detail === 'water' || detail === 'food'}
        {@const d = detail === 'water' ? data?.water_days : data?.food_days}
        <p class="big {d == null ? '' : tone(d)}">{fmt(d)}</p>
        {#if d == null}
          <p>Nothing can be counted yet. Add your {detail} to the Inventory, and how much of it you use each day, and this fills in.</p>
        {:else}
          <p>This is your {detail} in the Inventory divided by what you use each day, added up for everyone. It is an estimate: if more people come, or you use more, it gets shorter.</p>
          <p class="muted small">Red means under {settings.values.planActDays} days. Amber means under {settings.values.planCheckDays} days. You can change these numbers in Settings, Plan.</p>
        {/if}
      {:else if detail === 'low'}
        {#if lowAll.length === 0}
          <p>Nothing runs out within {settings.values.planCheckDays} days. Only items with a "used per day" amount are counted.</p>
        {:else}
          <p>These run out within {settings.values.planCheckDays} days at the amount you use:</p>
          {#each lowAll as r}
            <div class="line"><div class="grow"><strong>{r.name}</strong>{#if r.location}<div class="muted small">{r.location}</div>{/if}</div><span class="chip {r.days < settings.values.planActDays ? 'urgent' : 'soon'}">{fmt(r.days)}</span></div>
          {/each}
        {/if}
      {:else if detail === 'date'}
        {#if !data || data.expiring.length === 0}
          <p>Nothing is past its date or close to it.</p>
        {:else}
          <p>Items that are past their date, or will be soon:</p>
          {#each data.expiring as e}
            <div class="line"><div class="grow"><strong>{e.name}</strong></div><span class="chip {e.months < 0 ? 'urgent' : 'soon'}">{e.months < 0 ? Math.abs(e.months) + ' months past' : e.months + ' months left'}</span></div>
          {/each}
        {/if}
      {:else if detail === 'checks'}
        {#if followed.length === 0}
          <p>You are not following any plan yet. Choose a plan and its repeating checks will show up here when they are due.</p>
        {:else if dueSteps.length === 0}
          <p>Every check in the plans you follow is up to date.</p>
        {:else}
          <p>These checks are due again:</p>
          {#each dueSteps as d}
            <div class="line"><div class="grow"><strong>{d.step}</strong><div class="muted small">{d.plan}</div></div><span class="chip soon">Due</span></div>
          {/each}
        {/if}
      {:else}
        <p class="big {drillsDone ? 'ok' : 'check'}">{drillsDone} of {drillsTotal}</p>
        <p>A drill is a practice run, such as living without power for a day. It finds the gaps while everything still works. This counts the drills you have done at least once.</p>
      {/if}
      <div class="row end">
        <button class="btn" onclick={() => (detail = null)}>Close</button>
        <button class="btn primary" onclick={goDetail}>{detailGo[detail].text}</button>
      </div>
    </div>
  </dialog>
{/if}

<style>
  .dlg { padding: 18px; display: flex; flex-direction: column; gap: 10px; }
  .dlg h2 { margin: 0; }
  .dlg p { margin: 0; }
  .dlg .end { justify-content: flex-end; }
  .hero { padding: 14px; border: 1px solid var(--line); border-top: 2px solid var(--line2); border-radius: 12px; background: var(--panel); margin-bottom: 10px; }
  .herotop { display: flex; align-items: center; gap: 12px; margin-bottom: 12px; }
  .herologo { width: 52px; height: 52px; object-fit: contain; }
  .hero h1 { margin: 0; }
  .shortcuts { display: grid; grid-template-columns: repeat(auto-fit, minmax(88px, 1fr)); gap: 8px; }
  .sc { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; min-height: 76px; padding: 10px 6px; text-align: center; color: var(--text); background: var(--panel2, #14110e); border: 1px solid var(--line2); border-radius: 12px; font-size: 12.5px; letter-spacing: .02em; }
  .sc svg { color: var(--accent); }
  .sc:hover { border-color: var(--blue-line); }
  .sc:active { transform: scale(.97); }
  @media (max-width: 600px) { .shortcuts { grid-template-columns: repeat(3, 1fr); } .sc { min-height: 84px; } }
  /* a colour stripe on the left of each panel and tile, and a condensed heading (mockups/Navigation.html) */
  .panel { border-left: 3px solid var(--c, var(--line2)); }
  .panel > .label { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; font-family: 'Barlow Condensed', 'Arial Narrow', var(--display); font-stretch: condensed; font-size: 19px; font-weight: 700; letter-spacing: .06em; color: var(--text); }
  .panel > .label::before { content: '\25C6'; font-size: 11px; color: var(--c, var(--muted)); }
  .meter { height: 8px; border-radius: 4px; background: var(--line); overflow: hidden; }
  .meter span { display: block; height: 100%; background: var(--blue); }
  .dotmark { flex: 0 0 24px; height: 24px; border-radius: 50%; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--line2); color: var(--accent); font-size: 12px; }
  .dotmark.ok { background: #1f2a1d; border-color: #34502f; color: var(--ok); }
  .tiles { display: grid; grid-template-columns: repeat(6, 1fr); gap: 12px; margin-bottom: 12px; }
  @media (max-width: 1100px) { .tiles { grid-template-columns: repeat(3, 1fr); } }
  @media (max-width: 560px) { .tiles { grid-template-columns: repeat(2, 1fr); } }
  .tile { display: flex; flex-direction: column; justify-content: space-between; gap: 8px; min-height: 84px; padding: 14px; text-align: left; background: var(--panel); border: 1px solid var(--line); border-left: 3px solid var(--c, var(--line)); border-radius: 12px; }
  .tile:hover { border-color: var(--blue-line); }
  .big { font-size: 24px; font-weight: 600; margin-top: 2px; }
  /* every block has one set size, and shows only the most important lines; the rest scrolls inside it */
  .dgrid { display: grid; grid-template-columns: repeat(12, 1fr); grid-template-rows: 330px 440px 300px; gap: 12px; align-items: stretch; }
  .dgrid .panel { display: flex; flex-direction: column; min-height: 0; overflow: hidden; }
  .dgrid .scroll { flex: 1; min-height: 0; overflow-y: auto; }
  .dgrid .panel > .row.gap, .dgrid .panel > .foot { flex: none; }
  .catgrid { display: grid; grid-template-columns: 1fr 1fr; gap: 0 18px; }
  .catcell { display: flex; gap: 8px; justify-content: space-between; padding: 6px 0; border-top: 1px solid var(--line); font-size: 13.5px; }
  .catcell .cn { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .catcell .cc { font-family: var(--mono); }
  .catcell.none { color: var(--muted); }
  .s12 { grid-column: span 12; }
  .s4 { grid-column: span 4; } .s6 { grid-column: span 6; } .s8 { grid-column: span 8; }
  @media (max-width: 860px) {
    .s4, .s6, .s8, .s12 { grid-column: span 12; }
    .dgrid { grid-template-rows: none; }
    .dgrid .panel { overflow: visible; }
    .dgrid .scroll { overflow: visible; }
    .dgrid .sharing { order: -1; }
    .catgrid { grid-template-columns: 1fr; }
  }
  .line.cat { padding: 8px 0; }
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
