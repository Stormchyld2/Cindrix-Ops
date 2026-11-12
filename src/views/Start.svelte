<script lang="ts">
  import Icon from '../lib/Icon.svelte';
  import Gauge from '../lib/Gauge.svelte';
  import PageHeader from '../lib/PageHeader.svelte';

  import { familyStatus, inventoryList, drillsList, libraryStatus, modelStatus, listModels, mapPacks, whatFailsNext, type LibraryStatus, type ModelStatus } from '../lib/api';
  import { onMount } from 'svelte';

  let { go }: { go: (tab: string) => void } = $props();

  type Step = { title: string; why: string; todo: string[]; goal?: string; done: boolean | null; tab?: string; button?: string };
  let waterSet = $state(false);
  let foodSet = $state(false);
  let familyDone = $state(false);
  let inventoryDone = $state(false);
  let drillDone = $state(false);

  // What the drive has, in plain words, so a missing model or library is obvious on day one.
  type Check = { label: string; ok: boolean; text: string };
  let checks = $state<Check[]>([]);
  let mapsDone = $state(false);

  async function setupCheck() {
    const out: Check[] = [];
    let lib: LibraryStatus | null = null;
    let ms: ModelStatus | null = null;
    let nModels = 0;
    let nPacks = 0;
    try { lib = await libraryStatus(); } catch { /* shown as unknown */ }
    try { ms = await modelStatus(); nModels = (await listModels()).length; } catch { /* shown as unknown */ }
    try { nPacks = (await mapPacks()).length; } catch { /* shown as unknown */ }
    mapsDone = nPacks > 0;
    if (lib) {
      out.push(lib.drive
        ? { label: 'Drive', ok: true, text: `${lib.drive.name} (${lib.drive.edition}).` }
        : { label: 'Drive', ok: false, text: 'No Cindrix drive marker found, so this is running from the program folder. Content comes from the folders next to the program.' });
      const n = lib.archives.length;
      out.push(n > 0
        ? { label: 'Library', ok: true, text: `${n} archive${n === 1 ? '' : 's'} found.` }
        : { label: 'Library', ok: false, text: `No .zim archives found in ${lib.folder}\\library (subfolders are searched too). Put them there (or run scripts\\build-drive.ps1), then press Look again on the Archives screen.` });
    }
    if (ms) {
      out.push(ms.state === 'ready'
        ? { label: 'Assistant', ok: true, text: `Ready: ${ms.model}.` }
        : ms.state === 'loading'
          ? { label: 'Assistant', ok: true, text: `Loading ${ms.model}…` }
          : nModels === 0
            ? { label: 'Assistant', ok: false, text: 'No model file in the models folder. Copy a Qwen3.5 .gguf file there. Until then the assistant only finds pages and cards; it cannot write a summary.' }
            : { label: 'Assistant', ok: false, text: ms.message || 'A model file is there but not running. Open Settings > Assistant.' });
    }
    out.push(nPacks > 0
      ? { label: 'Maps', ok: true, text: `${nPacks} map file${nPacks === 1 ? '' : 's'} found.` }
      : { label: 'Maps', ok: false, text: 'No map files yet. Run scripts\\get-maps.ps1 to put the street map on the drive.' });
    checks = out;
  }

  onMount(async () => {
    try {
      familyDone = (await familyStatus()).exists;
      inventoryDone = (await inventoryList()).length > 0;
      const c = await whatFailsNext();
      waterSet = c.water_days != null;
      foodSet = c.food_days != null;
      drillDone = (await drillsList()).some((d) => d.times_done > 0);
    } catch {
      /* leave as not done */
    }
    await setupCheck();
  });

  // A walkthrough, in the order most people should do it. `done` is worked out from what you have really filled in;
  // null means a step with nothing to tick off (it is reading and thinking).
  let steps: Step[] = $derived([
    {
      title: 'Decide what you are planning for',
      why: 'You do not need a separate plan for every disaster. The same basics (water, food, warmth, light, medicine, a way to leave and a way to reach people) cover most of them. Start with what is most likely where you live.',
      todo: ['Think of the last time something went wrong near you: a long power cut, a storm, a flood, a wildfire, a road closed for days.', 'Pick the one or two likeliest. The drills later in this list are written for them.', 'Write the answer on the Family page, in the plan box, so everyone at home sees the same thing.'],
      done: null, tab: 'Drills', button: 'See the drills',
    },
    {
      title: 'Water',
      why: 'Water is the first thing to run short, and a person cannot go long without it.',
      todo: ['Store at least 1 gallon per person per day, for drinking and basic washing.', 'Count 3 days if you may have to leave, and 2 weeks if you would stay at home.', 'Add each jug or case to the Inventory with its use-by date and how much you use per day, so the program can count the days for you.', 'Learn how to make water safe: a rolling boil for 1 minute (3 minutes above 6,500 feet).'],
      goal: 'Goal: 14 days of water at home.', done: waterSet, tab: 'Inventory', button: 'Open Inventory',
    },
    {
      title: 'Food',
      why: 'Food that needs no power and little water keeps people steady and keeps the stress down.',
      todo: ['Choose food your family will really eat, and check for allergies and special diets.', 'Include food that needs no cooking, and a way to heat some if you can do it safely.', 'Put it in the Inventory with use-by dates, and use the oldest first.'],
      goal: 'Goal: 14 days of food at home, 3 days in a bag you can grab.', done: foodSet, tab: 'Inventory', button: 'Open Inventory',
    },
    {
      title: 'Health and the Family page',
      why: 'In an emergency you may not remember every name, medicine or phone number. Write them down now.',
      todo: ['Fill in each person: conditions, allergies, doctor, and medicines exactly as the doctor or pharmacy wrote them.', 'Ask your doctor or pharmacist how much extra to keep, and never change a dose on your own.', 'Add phone numbers for family, neighbours and one contact who lives far away.', 'A passphrase can lock the page. If you set one, write it down and keep it somewhere safe.'],
      done: familyDone, tab: 'Family', button: 'Open Family',
    },
    {
      title: 'Power, light and a way to reach people',
      why: 'When the power is out, small things (a light, a charged phone, a radio) make a big difference.',
      todo: ['Keep a flashlight or headlamp for each person, with spare batteries.', 'Keep a battery or hand-crank radio, and a charged power bank for phones.', 'Keep your phone numbers on paper, because phones go flat.', 'Never run a generator, grill or camp stove indoors or in a garage: the fumes (carbon monoxide) can kill.'],
      done: null, tab: 'Inventory', button: 'Add these to Inventory',
    },
    {
      title: 'Where to go',
      why: 'If you must leave, everyone should already know where to meet, even if the phones do not work.',
      todo: ['Open Maps, find your home, and add it as a pin.', 'Add two places to meet: one close to home and one outside your neighbourhood.', 'Look at two ways out of town, in case one is blocked.', 'Save the map so it opens next time. It works with no internet.'],
      done: mapsDone, tab: 'Maps', button: 'Open Maps',
    },
    {
      title: 'Print your binder',
      why: 'Screens go dark. A paper copy of the important pages still works.',
      todo: ['Open the Binder and print it.', 'Keep it where the whole household can find it, not locked away.', 'Reprint after you change your plan or your family page.'],
      done: null, tab: 'Binder', button: 'Open Binder',
    },
    {
      title: 'Practice with a drill',
      why: 'A plan that has never been tried always has a gap. A drill finds it while everything still works.',
      todo: ['Start with the 72-hour power outage drill.', 'Follow the steps with a timer running, and write down what went wrong.', 'Fix the one thing that hurt most, then run it again another day.'],
      done: drillDone, tab: 'Drills', button: 'Open Drills',
    },
    {
      title: 'Keep it fresh',
      why: 'Supplies expire and families change. A few minutes a month keeps the plan true.',
      todo: ['Once a month, open the Dashboard and deal with the first two things it suggests.', 'Use the oldest food and water first, and replace what you used.', 'After any real emergency, write down what worked and what did not.'],
      done: null, tab: 'Dashboard', button: 'Open Dashboard',
    },
  ]);
  let sel = $state(0);
  const short = (t: string) => t.replace(/^Decide what you are planning for$/, 'Decide').replace(/^Health and the Family page$/, 'Health').replace(/^Power, light and a way to reach people$/, 'Power').replace(/^Print your binder$/, 'Binder').replace(/^Practice with a drill$/, 'Drill').replace(/^Keep it fresh$/, 'Keep fresh');
  let ticks: Record<number, boolean[]> = $state((() => { try { return JSON.parse(localStorage.getItem('cindrix.start.ticks') || '{}'); } catch { return {}; } })());
  function tickKey(n: number, i: number, on: boolean) {
    const row = [...(ticks[n] ?? [])]; row[i] = on; ticks[n] = row;
    try { localStorage.setItem('cindrix.start.ticks', JSON.stringify(ticks)); } catch { /* kept for this visit only */ }
  }
  let tickable = $derived(steps.filter((s) => s.done !== null));
  let inProgress = $derived(tickable.filter((s) => s.done === false && (ticks[steps.indexOf(s)] ?? []).some(Boolean)).length);
  let doneCount = $derived(tickable.filter((s) => s.done).length);
  let notStarted = $derived(tickable.length - doneCount - inProgress);
</script>

<PageHeader title="Get ready" tagline="Build Readiness • Close Gaps • Be Prepared" />

<div class="top3">
  <section class="dbl" style="--c: var(--glow-blue)">
    <div class="pnl-h nosl">Readiness status</div>
    <Gauge value={tickable.length ? (doneCount / tickable.length) * 100 : 0} label="Overall ready" />
  </section>
  <section class="dbl" style="--c: var(--glow-blue)">
    <div class="pnl-h nosl">Progress overview</div>
    {#each [['check', 'Steps completed', doneCount, 'var(--glow-green)'], ['hourglass', 'In progress', inProgress, 'var(--glow-blue)'], ['calx', 'Not started', notStarted, 'var(--glow-blue)']] as [ic, lb, n, col]}
      <div class="po"><span style="color:{col}"><Icon name={ic as string} size={26} /></span><span class="pl">{lb}</span><span class="pb"><span style="width:{tickable.length ? (Number(n) / tickable.length) * 100 : 0}%; background:{col}"></span></span><span class="pn">{ic === 'check' ? `${n} of ${tickable.length}` : n}</span></div>
    {/each}
  </section>
  <section class="dbl yourplan" style="--c: var(--glow-blue); background-image: linear-gradient(90deg, #0a1018 40%, rgba(10, 16, 24, .2)), url(/ui/tent.jpg); background-size: auto, auto 100%; background-position: 0 0, right center; background-repeat: no-repeat">
    <div class="pnl-h nosl"><Icon name="clipboard" size={28} /> Your plan</div>
    <p>Your step-by-step readiness walkthrough. Check each item as you complete it and build a solid, practical plan for your family and situation.</p>
    <button class="btn ico" onclick={() => go('What next')}>What next <Icon name="chevron" size={16} /></button>
  </section>
</div>

<nav class="chev" aria-label="Steps">
  {#each steps as s, n}
    <button class="cv" class:on={sel === n} class:done={s.done === true} onclick={() => (sel = n)} title={s.title}>
      <span class="cn">{s.done === true ? '✓' : n + 1}</span><span class="ct">{short(s.title)}</span>
    </button>
  {/each}
</nav>

{#if steps[sel]}
  {@const s = steps[sel]}
  <div class="main2">
    <section class="dbl stepmain" style="--c: var(--glow-blue)">
      <div class="steptop">
        <span class="mark big" class:done={s.done === true}>{sel + 1}</span>
        <h2 class="stitle">{s.title}</h2>
        {#if s.tab}<button class="btn primary ico" onclick={() => go(s.tab!)}><Icon name="list" size={16} /> {s.button}</button>{/if}
      </div>
      <p class="why">{s.why}</p>
      <div class="keyact">
        <div class="ka-h"><Icon name="target" size={22} /> Key actions</div>
        {#each s.todo as t, i}
          <label class="ka"><input type="checkbox" checked={!!ticks[sel]?.[i]} onchange={(e) => tickKey(sel, i, e.currentTarget.checked)} /><span>{t}</span></label>
        {/each}
      </div>
      {#if s.goal}<div class="goalbox"><Icon name="flag" size={22} /><b>GOAL</b><i></i><span>{s.goal.replace(/^Goal:\s*/, '')}</span></div>{/if}
    </section>
    <section class="dbl stepcheck" style="--c: var(--glow-blue)">
      <div class="p2h"><div class="pnl-h nosl"><Icon name="clipboard" size={26} /> Step checklist</div><span class="cnt">{doneCount} of {tickable.length}</span></div>
      {#each steps as t, n}
        <div class="sc2" class:on={sel === n}>
          <span class="box" class:done={t.done === true}>{t.done === true ? '✓' : ''}</span>
          <span class="sn">{n + 1}. {t.title}</span>
          {#if t.done === true}<span class="pill on">Done</span>{:else}<button class="pill btnpill" onclick={() => (sel = n)}>Open</button>{/if}
        </div>
      {/each}
    </section>
  </div>
{/if}

<nav class="tiles5" aria-label="Quick links">
  <button class="mt" style="--c: var(--glow-blue)" onclick={() => go('Inventory')}><span class="bg" style="background-image:url(/ui/tile-water.jpg)"></span><span class="mico"><Icon name="drop" size={30} /></span><span><div class="mt-t">Water</div><div class="mt-s">Store • Treat • Purify</div></span></button>
  <button class="mt" style="--c: var(--glow-blue)" onclick={() => go('Inventory')}><span class="bg" style="background-image:url(/ui/tile-food.jpg)"></span><span class="mico" style="color: var(--glow-amber)"><Icon name="fork" size={30} /></span><span><div class="mt-t">Food</div><div class="mt-s">Stock • Preserve • Rotate</div></span></button>
  <button class="mt" style="--c: var(--glow-blue)" onclick={() => go('Family')}><span class="bg" style="background-image:url(/ui/tile-health.jpg)"></span><span class="mico"><Icon name="medical" size={30} /></span><span><div class="mt-t">Health</div><div class="mt-s">First Aid • Medicine • Care</div></span></button>
  <button class="mt" style="--c: var(--glow-blue)" onclick={() => go('Inventory')}><span class="bg" style="background-image:url(/ui/tile-power.jpg)"></span><span class="mico" style="color: var(--glow-amber)"><Icon name="bolt" size={30} /></span><span><div class="mt-t">Power</div><div class="mt-s">Light • Charge • Run</div></span></button>
  <button class="mt" style="--c: var(--glow-blue)" onclick={() => go('Drills')}><span class="bg" style="background-image:url(/ui/tile-tools.jpg)"></span><span class="mico"><Icon name="gear" size={30} /></span><span><div class="mt-t">Prep tools</div><div class="mt-s">Drills • Plans • Checklists</div></span></button>
</nav>

{#if checks.length}
  <section class="panel" aria-label="Setup check">
    <div class="row between"><span class="label">IS THE DRIVE READY? (SETUP CHECK)</span><span class="count">{checks.filter((c) => c.ok).length} of {checks.length} ready</span></div>
    {#each checks as c}
      <div class="step">
        <span class="mark" class:done={c.ok}>{c.ok ? '✓' : '!'}</span>
        <div class="grow">
          <div class="title">{c.label}</div>
          <div class="muted small">{c.text}</div>
        </div>
      </div>
    {/each}
  </section>
{/if}

<div class="panel small muted">Cindrix Ops is a reference tool, not a replacement for emergency services. In an emergency, call 911.</div>

<style>
  .top3 { display: grid; grid-template-columns: 1fr 1.25fr 1.7fr; gap: 14px; }
  .top3 > section { padding: 14px 18px; }
  @media (max-width: 1100px) { .top3 { grid-template-columns: 1fr; } }
  .po { display: grid; grid-template-columns: 30px 130px 1fr auto; gap: 10px; align-items: center; padding: 5px 0; }
  .pl { color: #e6edf5; }
  .pb { height: 8px; border-radius: 5px; background: #1b2735; overflow: hidden; display: block; }
  .pb > span { display: block; height: 100%; box-shadow: 0 0 8px currentColor; }
  .pn { font-family: var(--mono); min-width: 52px; text-align: right; }
  .yourplan p { margin: 0 0 10px; max-width: 58%; color: #dfe7f0; }
  .chev { display: flex; gap: 0; overflow-x: auto; scrollbar-width: none; }
  .cv { flex: 1 0 128px; display: flex; align-items: center; justify-content: center; gap: 10px; height: 54px; padding: 0 22px; border: 0; color: #fff; font-family: 'Barlow Condensed', 'Arial Narrow', var(--display); font-stretch: condensed; font-size: 19px; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; background: #0e1824; clip-path: polygon(0 0, calc(100% - 18px) 0, 100% 50%, calc(100% - 18px) 100%, 0 100%, 18px 50%); margin-left: -8px; box-shadow: inset 0 0 0 2px #24456b; }
  .cv:first-child { margin-left: 0; clip-path: polygon(0 0, calc(100% - 18px) 0, 100% 50%, calc(100% - 18px) 100%, 0 100%); }
  .cv.on { background: linear-gradient(180deg, #38a4ff, #1c7fe0); }
  .cn { display: inline-flex; align-items: center; justify-content: center; width: 30px; height: 30px; border-radius: 50%; border: 2px solid #4db0ff; font-size: 16px; }
  .cv.done .cn { background: #1f6f3a; border-color: #43e06b; }
  .main2 { display: grid; grid-template-columns: minmax(0, 1.7fr) minmax(300px, 1fr); gap: 14px; align-items: start; }
  @media (max-width: 1000px) { .main2 { grid-template-columns: 1fr; } }
  .stepmain, .stepcheck { padding: 16px 20px; }
  .steptop { display: flex; flex-wrap: wrap; align-items: center; gap: 12px 14px; }
  .mark { flex: 0 0 28px; height: 28px; border-radius: 50%; display: inline-flex; align-items: center; justify-content: center; border: 2px solid #4db0ff; color: #fff; font-size: 13px; }
  .mark.big { flex-basis: 40px; height: 40px; font-size: 22px; }
  .mark.done { background: var(--ok-bg); border-color: var(--ok-line); color: var(--ok); }
  .stitle { flex: 1 1 200px; margin: 0; font-size: 26px; letter-spacing: .05em; }
  .why { margin: 12px 0; color: #e3eaf2; }
  .keyact { border: 1px solid #24456b; border-radius: 10px; padding: 10px 14px; background: #0a121b; }
  .ka-h { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; font-family: 'Barlow Condensed', 'Arial Narrow', var(--display); font-size: 20px; font-weight: 700; letter-spacing: .06em; text-transform: uppercase; color: var(--glow-blue); }
  .ka { display: flex !important; flex-direction: row !important; align-items: flex-start; gap: 10px !important; padding: 5px 0; font-size: 15px !important; color: #f0f4f8 !important; }
  .ka input { width: 22px; height: 22px; margin-top: 1px; accent-color: #43e06b; flex: none; }
  .goalbox { display: flex; align-items: center; gap: 12px; margin-top: 12px; padding: 10px 14px; border: 1.5px solid var(--glow-amber); border-radius: 10px; background: rgba(255, 176, 46, .06); color: var(--glow-amber); }
  .goalbox b { font-size: 20px; letter-spacing: .06em; }
  .goalbox i { width: 1px; align-self: stretch; background: #6b5320; }
  .goalbox span { color: #f0f4f8; }
  .p2h { display: flex; justify-content: space-between; align-items: center; }
  .cnt { padding: 3px 14px; border: 1.5px solid #4db0ff; border-radius: 8px; font-family: var(--mono); color: #fff; }
  .sc2 { display: flex; align-items: center; gap: 12px; padding: 11px 4px; border-top: 1px solid #1d3550; }
  .sc2.on { background: rgba(61, 155, 255, .08); }
  .box { flex: none; width: 22px; height: 22px; border: 2px solid #8da2b8; border-radius: 4px; display: inline-flex; align-items: center; justify-content: center; font-size: 14px; color: #fff; }
  .box.done { background: #43e06b; border-color: #43e06b; color: #08210f; }
  .sn { flex: 1; font-size: 16px; }
  .btnpill { background: transparent; border-color: #cfd8e3; color: #fff; cursor: pointer; }
  .tiles5 { display: grid; grid-template-columns: repeat(auto-fit, minmax(210px, 1fr)); gap: 12px; }
  .tiles5 .mt { min-height: 68px; }
  .tiles5 .mt .bg { width: 38%; opacity: .8; }
  .step { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px; padding: 14px 0; border-top: 1px solid var(--line); margin-top: 6px; }
  .grow { flex: 1 1 260px; }
  .title { font-weight: 600; }
  .small { font-size: 13px; }
  .between { justify-content: space-between; }
  .count { font-family: var(--mono); font-size: 13px; color: var(--blue); }
</style>