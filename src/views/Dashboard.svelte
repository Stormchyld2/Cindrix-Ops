<script lang="ts">
  import { onMount } from 'svelte';
  import { whatFailsNext, inventoryList, drillsList, mapPacks, modelStatus, listModels, libraryStatus, dashboardSuggestions, type Cascade, type Suggestion } from '../lib/api';
  import { settings } from '../lib/settings.svelte';

  let { go }: { go: (tab: string) => void } = $props();

  let data: Cascade | null = $state(null);
  let items = $state(0);
  let tips: Suggestion[] = $state([]);
  let drillsDone = $state(0);
  let drillsTotal = $state(0);
  let error = $state('');
  let loaded = $state(false);

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
    loaded = true;
  });

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

<style>
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
