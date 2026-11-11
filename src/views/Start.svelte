<script lang="ts">
  import PageHeader from '../lib/PageHeader.svelte';
  import Ring from '../lib/Ring.svelte';
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
  let tickable = $derived(steps.filter((s) => s.done !== null));
  let doneCount = $derived(tickable.filter((s) => s.done).length);
</script>

<PageHeader eyebrow="GET READY" title="Your plan, step by step" tagline="Work down this list once, in order. Each step says why it matters and what to do. Steps that can be checked tick themselves when you have really filled something in." />

<section class="panel glow overview" style="--c: var(--blue)">
  <Ring value={tickable.length ? (doneCount / tickable.length) * 100 : 0} size={132} label="ready" />
  <div class="ovtxt">
    <div class="label">YOUR PROGRESS</div>
    <p class="count">{doneCount} of {tickable.length} checked steps done</p>
    <div class="track"><span style="width:{tickable.length ? (doneCount / tickable.length) * 100 : 0}%"></span></div>
    <div class="chev" aria-label="Steps">
      {#each steps as s, n}
        <a class="cv" class:done={s.done === true} href={'#step' + n} title={s.title}>{s.done === true ? '✓' : n + 1}</a>
      {/each}
    </div>
  </div>
  <div class="ovnext">
    <div class="label">WHEN THIS LIST IS DONE</div>
    <p class="muted small">It does not end here. Repeating checks, learning plans and a plan for each kind of emergency pick up where this stops.</p>
    <button class="btn" onclick={() => go('What next')}>What next →</button>
  </div>
</section>

<div class="steps2">
{#each steps as s, n}
  <section class="panel stepcard glow" id={'step' + n} style="--c: {s.done === true ? 'var(--ok)' : 'var(--blue)'}">
    <div class="steptop">
      <span class="mark" class:done={s.done === true}>{s.done === true ? '✓' : n + 1}</span>
      <h2 class="stitle">{s.title}</h2>
    </div>
    <p class="why">{s.why}</p>
    <ul class="todo">{#each s.todo as t}<li>{t}</li>{/each}</ul>
    <div class="stfoot">
      {#if s.goal}<p class="goal">{s.goal}</p>{/if}
      {#if s.tab}<button class="btn" class:primary={s.done !== true} onclick={() => go(s.tab!)}>{s.button}</button>{/if}
    </div>
  </section>
{/each}
</div>

<nav class="statrow quick" aria-label="Quick links">
  <button class="stat" style="--c: var(--cat-ops)" onclick={() => go('Inventory')}><span class="ico">▣</span><span><div class="n">Inventory</div><div class="t">Water, food, gear</div></span></button>
  <button class="stat" style="--c: var(--cat-intel)" onclick={() => go('Family')}><span class="ico">☺</span><span><div class="n">Family</div><div class="t">People and contacts</div></span></button>
  <button class="stat" style="--c: var(--cat-academy)" onclick={() => go('Maps')}><span class="ico">⌖</span><span><div class="n">Maps</div><div class="t">Where to go</div></span></button>
  <button class="stat" style="--c: var(--cat-security)" onclick={() => go('Drills')}><span class="ico">▶</span><span><div class="n">Drills</div><div class="t">Practice runs</div></span></button>
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
  .between { justify-content: space-between; }
  .count { font-family: var(--mono); font-size: 13px; color: var(--blue); }
  .track { margin-top: 8px; height: 8px; border-radius: 4px; background: var(--line); overflow: hidden; }
  .track span { display: block; height: 100%; background: var(--blue); }
  .step { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px; padding: 14px 0; border-top: 1px solid var(--line); margin-top: 6px; }
  .mark { flex: 0 0 28px; height: 28px; border-radius: 50%; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--line2); color: var(--muted); font-size: 13px; }
  .mark.done { background: var(--ok-bg); border-color: var(--ok-line); color: var(--ok); }
  .grow { flex: 1 1 260px; }
  .overview { display: grid; grid-template-columns: auto minmax(0, 1fr) 280px; gap: 20px; align-items: center; }
  .ovtxt p { margin: 4px 0 8px; }
  .chev { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 12px; }
  .cv { min-width: 34px; height: 30px; padding: 0 10px; display: inline-flex; align-items: center; justify-content: center; clip-path: polygon(0 0, 85% 0, 100% 50%, 85% 100%, 0 100%, 15% 50%); background: var(--panel2); color: var(--muted); font-size: 13px; text-decoration: none; }
  .cv.done { background: var(--ok-bg); color: var(--ok); }
  .ovnext { border-left: 1px solid var(--line); padding-left: 20px; }
  @media (max-width: 900px) { .overview { grid-template-columns: 1fr; } .ovnext { border: 0; padding: 0; } }
  .steps2 { display: grid; grid-template-columns: repeat(auto-fill, minmax(420px, 1fr)); gap: 14px; }
  @media (max-width: 520px) { .steps2 { grid-template-columns: 1fr; } }
  .stepcard { display: flex; flex-direction: column; scroll-margin-top: 12px; }
  .stfoot { margin-top: auto; padding-top: 10px; display: flex; flex-wrap: wrap; gap: 8px 12px; align-items: center; justify-content: space-between; }
  .quick { margin-top: 2px; }
  .steptop { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px; }
  .stitle { flex: 1 1 200px; font-size: 18px; letter-spacing: .06em; }
  .why { margin: 10px 0 6px; color: var(--muted); }
  .todo { margin: 6px 0; padding-left: 22px; }
  .todo li { margin: 4px 0; }
  .goal { margin: 8px 0 0; font-family: var(--mono); font-size: 12px; letter-spacing: .06em; color: var(--blue); }
  .next { margin-top: 12px; }
  .title { font-weight: 600; }
  .small { font-size: 13px; }
</style>
