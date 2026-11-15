<script lang="ts">
  import PageHeader from '../lib/PageHeader.svelte';
  import { whatFailsNext, replacementsList, replacementSet, type Cascade, type Replacement } from '../lib/api';
  import { onMount } from 'svelte';
  import { settings } from '../lib/settings.svelte';

  let { go }: { go: (tab: string) => void } = $props();

  let data: Cascade | null = $state(null);
  let error = $state('');
  let repl: Replacement[] = $state([]);

  onMount(async () => {
    try {
      data = await whatFailsNext();
      repl = await replacementsList();
    } catch (e) {
      error = String(e);
    }
  });

  const keep: [string, string][] = [
    ['keep-water-from-failing', 'Water'], ['keep-food-from-failing', 'Food'], ['keep-power-from-failing', 'Power and light'],
    ['keep-warm-and-cool-from-failing', 'Warmth and cooling'], ['keep-cooking-fuel-from-failing', 'Cooking and fuel'],
    ['keep-communication-from-failing', 'Communication'], ['keep-medicine-from-failing', 'Medicine and health'],
    ['keep-sanitation-from-failing', 'Toilets and hygiene'], ['keep-the-car-from-failing', 'The car and go-bags'],
    ['keep-papers-and-cash-from-failing', 'Papers and cash'], ['keep-home-safe-from-failing', 'Home safety'],
    ['keep-pets-from-failing', 'Pets'], ['keep-babies-and-children-from-failing', 'Babies and children'], ['keep-knowledge-from-failing', 'Knowledge and maps'],
  ];
  function openPlan(id: string) {
    try { sessionStorage.setItem('cindrix.openPlan', id); } catch { /* the Plans page just opens on its list */ }
    go('What next');
  }
  const stateWord: Record<string, string> = { notstarted: 'Not started', learning: 'Learning it', inplace: 'In place' };
  async function setState(need: string, st: Replacement['state']) {
    try { await replacementSet(need, st); repl = await replacementsList(); } catch (e) { error = String(e); }
  }
  function fmt(d: number | null): string {
    return d == null ? 'Not set' : d < 10 ? d.toFixed(1) + ' days' : Math.round(d) + ' days';
  }
  function tone(d: number): string {
    return d < settings.values.planActDays ? 'act' : d < settings.values.planCheckDays ? 'check' : 'ok';
  }
</script>

<section>
  <PageHeader eyebrow="ESTIMATE · NOT A PREDICTION" title="What fails next" tagline="What runs out first, using the numbers in your Inventory. Change the amounts or the &quot;used per day&quot; and this updates." />
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

<section class="panel">
  <div class="label">WHAT REPLACES IT</div>
  <p class="muted">Stored food and supplies run out. Each line below says what takes over when they do, the skill you need for it, and how long it takes to get going. Mark where you are.</p>
  {#each repl as r (r.need)}
    <div class="line wrap">
      <div class="grow">
        <strong>{r.need}</strong> <span class="muted">is replaced by</span> {r.replaced_by}
        <div class="muted small">Skill: {r.skill} · Takes about {r.lead_time}</div>
        {#if r.warning && r.state !== 'inplace'}<div class="check small">{r.warning}</div>{/if}
      </div>
      <div class="row seg" role="group" aria-label={`Where you are with ${r.need}`}>
        {#each ['notstarted', 'learning', 'inplace'] as st}
          <button class="btn sm" class:primary={r.state === st} onclick={() => setState(r.need, st as Replacement['state'])} title={`Mark ${r.need} as: ${stateWord[st]}`}>{stateWord[st]}</button>
        {/each}
      </div>
    </div>
  {/each}
  <div class="row"><button class="btn" onclick={() => go('training')} title="Open the skills grid">Open the skills grid</button></div>
</section>

<section class="panel">
  <div class="label">HOW TO KEEP IT FROM FAILING</div>
  <p class="muted">Counting what runs out is the first half. Each walkthrough below shows what to do, and how, so that thing does not fail in the first place. Pick the one that is short or running low.</p>
  <div class="row keep">
    {#each keep as [id, label]}
      <button class="btn" onclick={() => openPlan(id)} aria-label={`Open the walkthrough: keep ${label} from failing`}>{label}</button>
    {/each}
  </div>
</section>

<style>
  .tiles { display: flex; flex-wrap: wrap; gap: 12px; }
  .tile { flex: 1 1 180px; padding: 14px; background: var(--panel); border: 1px solid var(--line); border-radius: 12px; }
  .big { font-size: 24px; font-weight: 600; margin-top: 2px; }
  .line { display: flex; gap: 12px; padding: 10px 0; border-top: 1px solid var(--line); align-items: center; }
  .grow { flex: 1; }
  .line.wrap { flex-wrap: wrap; }
  .small { font-size: 13px; }
  .keep { gap: 8px; margin-top: 8px; }
</style>
