<script lang="ts">
  import { familyStatus, inventoryList } from '../lib/api';
  import { onMount } from 'svelte';

  let { go }: { go: (tab: string) => void } = $props();

  type Step = { title: string; text: string; done: boolean; tab?: string; button?: string };
  let familyDone = $state(false);
  let inventoryDone = $state(false);

  onMount(async () => {
    try {
      familyDone = (await familyStatus()).exists;
      inventoryDone = (await inventoryList()).length > 0;
    } catch {
      /* leave as not done */
    }
  });

  let steps: Step[] = $derived([
    { title: 'Set your home spot', text: 'Tap your home on the map and save it. (Maps are not built yet.)', done: false },
    { title: 'Fill in the Family page', text: "Names, medicines as your doctor wrote them, allergies, contacts.", done: familyDone, tab: 'Family', button: 'Open Family' },
    { title: 'Add your county maps', text: 'Street and topo come on the drive. (Not built yet.)', done: false },
    { title: 'Count your supplies', text: 'Water, food, glucose supplies and batteries, with use-by dates.', done: inventoryDone, tab: 'Inventory', button: 'Open Inventory' },
    { title: 'Print your binder', text: 'A paper copy of your plan. (Not built yet.)', done: false },
    { title: 'Run your first drill', text: 'The 72-hour power outage drill. (Not built yet.)', done: false },
  ]);
  let doneCount = $derived(steps.filter((s) => s.done).length);
</script>

<section>
  <h1>Get ready in 6 steps</h1>
  <p class="sub">Do these now, while everything still works. Progress is worked out from what you have really filled in.</p>
</section>

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
  .count { font-family: var(--mono); font-size: 13px; color: var(--accent); }
  .track { margin-top: 8px; height: 8px; border-radius: 4px; background: var(--line); overflow: hidden; }
  .track span { display: block; height: 100%; background: var(--accent); }
  .step { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px; padding: 14px 0; border-top: 1px solid var(--line); margin-top: 6px; }
  .mark { flex: 0 0 28px; height: 28px; border-radius: 50%; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--line2); color: var(--muted); font-size: 13px; }
  .mark.done { background: #1f2a1d; border-color: #34502f; color: var(--ok); }
  .grow { flex: 1 1 260px; }
  .title { font-weight: 600; }
  .small { font-size: 13px; }
</style>
