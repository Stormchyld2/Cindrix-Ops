<script lang="ts">
  import { familyStatus, inventoryList, drillsList, libraryStatus, modelStatus, listModels, mapPacks, type LibraryStatus, type ModelStatus } from '../lib/api';
  import { onMount } from 'svelte';

  let { go }: { go: (tab: string) => void } = $props();

  type Step = { title: string; text: string; done: boolean; tab?: string; button?: string };
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
        : { label: 'Library', ok: false, text: 'No .zim archives found. Put them in the library folder (or run scripts\\build-drive.ps1), then press Look again in Settings.' });
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
      drillDone = (await drillsList()).some((d) => d.times_done > 0);
    } catch {
      /* leave as not done */
    }
    await setupCheck();
  });

  let steps: Step[] = $derived([
    { title: 'Set your home spot', text: 'Tap your home on the map and save it. (Maps are not built yet.)', done: false },
    { title: 'Fill in the Family page', text: "Names, medicines as your doctor wrote them, allergies, contacts.", done: familyDone, tab: 'Family', button: 'Open Family' },
    { title: 'Check your maps', text: 'The street map comes on the drive. Open Maps and look at your area.', done: mapsDone },
    { title: 'Count your supplies', text: 'Water, food, glucose supplies and batteries, with use-by dates.', done: inventoryDone, tab: 'Inventory', button: 'Open Inventory' },
    { title: 'Print your binder', text: 'A paper copy of your plan, for when the screens are dark.', done: false, tab: 'Binder', button: 'Open Binder' },
    { title: 'Run your first drill', text: 'The 72-hour power outage drill.', done: drillDone, tab: 'Drills', button: 'Open Drills' },
  ]);
  let doneCount = $derived(steps.filter((s) => s.done).length);
</script>

<section>
  <h1>Get ready in 6 steps</h1>
  <p class="sub">Do these now, while everything still works. Progress is worked out from what you have really filled in.</p>
</section>

{#if checks.length}
  <section class="panel" aria-label="Setup check">
    <div class="row between"><span class="label">SETUP CHECK</span><span class="count">{checks.filter((c) => c.ok).length} of {checks.length} ready</span></div>
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

<section class="panel">
  <div class="row between"><span class="label">READINESS</span><span class="count">{doneCount} of {steps.length} done</span></div>
  <div class="track"><span style="width:{(doneCount / steps.length) * 100}%"></span></div>
  {#each steps as s, n}
    <div class="step">
      <span class="mark" class:done={s.done}>{s.done ? '✓' : n + 1}</span>
      <div class="grow">
        <div class="title" class:muted={s.done}>{s.title}</div>
        <div class="muted small">{s.text}</div>
      </div>
      {#if s.tab}
        <button class="btn" class:primary={!s.done} onclick={() => go(s.tab!)}>{s.button}</button>
      {/if}
    </div>
  {/each}
</section>

<div class="panel small muted">Cindrix Ops is a reference tool, not a replacement for emergency services. In an emergency, call 911.</div>

<style>
  .between { justify-content: space-between; }
  .count { font-family: var(--mono); font-size: 13px; color: var(--blue); }
  .track { margin-top: 8px; height: 8px; border-radius: 4px; background: var(--line); overflow: hidden; }
  .track span { display: block; height: 100%; background: var(--blue); }
  .step { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px; padding: 14px 0; border-top: 1px solid var(--line); margin-top: 6px; }
  .mark { flex: 0 0 28px; height: 28px; border-radius: 50%; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--line2); color: var(--muted); font-size: 13px; }
  .mark.done { background: #1f2a1d; border-color: #34502f; color: var(--ok); }
  .grow { flex: 1 1 260px; }
  .title { font-weight: 600; }
  .small { font-size: 13px; }
</style>
