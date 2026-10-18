<script lang="ts">
  import { whatFailsNext, type Cascade } from '../lib/api';
  import { onMount } from 'svelte';

  let { go }: { go: (tab: string) => void } = $props();

  let data: Cascade | null = $state(null);
  let error = $state('');

  onMount(async () => {
    try {
      data = await whatFailsNext();
    } catch (e) {
      error = String(e);
    }
  });

  function fmt(d: number | null): string {
    return d == null ? 'Not set' : d < 10 ? d.toFixed(1) + ' days' : Math.round(d) + ' days';
  }
  function tone(d: number): string {
    return d < 7 ? 'act' : d < 14 ? 'check' : 'ok';
  }
</script>

<section>
  <div class="row">
    <h1>What fails next</h1>
    <span class="badge">Estimate · not a prediction</span>
  </div>
  <p class="sub">What runs out first, using the numbers in your Inventory. Change the amounts or the "used per day" and this updates.</p>
</section>

{#if error}<div class="warn" role="alert">{error}</div>{/if}

{#if data}
  <div class="tiles">
    <div class="tile"><div class="label">WATER</div><div class="big {data.water_days == null ? '' : tone(data.water_days)}">{fmt(data.water_days)}</div></div>
    <div class="tile"><div class="label">FOOD</div><div class="big {data.food_days == null ? '' : tone(data.food_days)}">{fmt(data.food_days)}</div></div>
    <div class="tile"><div class="label">EXPIRING IN 3 MONTHS</div><div class="big {data.expiring.length ? 'check' : 'ok'}">{data.expiring.length} item{data.expiring.length === 1 ? '' : 's'}</div></div>
  </div>

  <section class="panel">
    <div class="label">RUNS OUT FIRST</div>
    {#if data.runs_out.length === 0}
      <p class="muted">No items have a "used per day" amount yet. Add one in the Inventory.</p>
    {/if}
    {#each data.runs_out as r}
      <div class="line">
        <div class="grow"><strong>{r.name}</strong><div class="muted small">{r.location}</div></div>
        <div class="{tone(r.days)}">{fmt(r.days)}</div>
      </div>
    {/each}
  </section>

  {#if data.expiring.length}
    <section class="panel">
      <div class="label">EXPIRING OR PAST DATE</div>
      {#each data.expiring as e}
        <div class="line">
          <div class="grow"><strong>{e.name}</strong></div>
          <div class={e.months < 0 ? 'act' : 'check'}>{e.months < 0 ? 'Past its date' : e.months === 0 ? 'This month' : 'In ' + e.months + ' month' + (e.months === 1 ? '' : 's')}</div>
        </div>
      {/each}
    </section>
  {/if}

  <div class="row"><button class="btn" onclick={() => go('Inventory')}>Open Inventory</button></div>
{/if}

<style>
  .tiles { display: flex; flex-wrap: wrap; gap: 12px; }
  .tile { flex: 1 1 180px; padding: 14px; background: var(--panel); border: 1px solid var(--line); border-radius: 12px; }
  .big { font-size: 24px; font-weight: 600; margin-top: 2px; }
  .line { display: flex; gap: 12px; padding: 10px 0; border-top: 1px solid var(--line); align-items: center; }
  .grow { flex: 1; }
  .small { font-size: 13px; }
</style>
