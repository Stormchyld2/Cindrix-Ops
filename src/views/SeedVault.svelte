<script lang="ts">
  import { onMount } from 'svelte';
  import { seedsList, seedSave, seedDelete, seedGuides, type Seed, type Card } from '../lib/api';
  import { parts, blocks } from '../lib/cardtext';

  let { go }: { go: (tab: string) => void } = $props();

  const CROPS = ['Bean (bush)', 'Bean (pole)', 'Dry bean', 'Pea', 'Corn (field or flour)', 'Sweet corn', 'Squash (winter)', 'Squash (summer)', 'Pumpkin', 'Tomato', 'Pepper', 'Cucumber', 'Lettuce', 'Spinach', 'Kale', 'Collards', 'Cabbage', 'Broccoli', 'Carrot', 'Beet', 'Radish', 'Turnip', 'Onion', 'Leek', 'Parsnip', 'Swiss chard', 'Okra', 'Melon', 'Watermelon', 'Sunflower', 'Basil', 'Parsley', 'Cilantro', 'Dill', 'Oats', 'Wheat', 'Buckwheat', 'Flax'];
  const KINDS = ['Heirloom', 'Open-pollinated', 'Hybrid', 'Not sure'];

  let seeds: Seed[] = $state([]);
  let guides: Card[] = $state([]);
  let tab: 'guides' | 'mine' = $state('guides');
  let openGuide: Card | null = $state(null);
  let error = $state('');
  let search = $state('');
  let show = $state('all');
  let selected: string[] = $state([]);
  let confirming = $state(false);
  let dlg: HTMLDialogElement;
  let form: Seed = $state(blank());

  function blank(): Seed {
    return { id: '', crop: '', variety: '', kind: 'Heirloom', packets: 1, seeds_per_packet: 0, year_packed: new Date().getFullYear(), germination: 0, tested: '', source: '', location: '', plant_when: '', days_to_harvest: 0, target: 0, notes: '' };
  }

  onMount(async () => {
    try {
      guides = await seedGuides();
      seeds = await seedsList();
      if (seeds.length) tab = 'mine';
    } catch (e) {
      error = String(e);
    }
  });

  const GROUPS: [string, string][] = [['buy', 'WHAT TO BUY'], ['store', 'KEEPING SEEDS'], ['grow', 'PLANNING WHAT TO GROW'], ['save', 'SAVING YOUR OWN SEED']];
  const groups = $derived(GROUPS.map(([k, t]) => ({ title: t, list: guides.filter((g) => g.kind === k) })).filter((g) => g.list.length));
  const other = $derived(guides.filter((g) => !GROUPS.some(([k]) => k === g.kind)));

  const shown = $derived(seeds.filter((s) => {
    if (show === 'low' && s.status === 'ok') return false;
    if (search && !(s.crop + ' ' + s.variety + ' ' + s.notes + ' ' + s.location).toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  }));
  const allSelected = $derived(shown.length > 0 && shown.every((s) => selected.includes(s.id)));
  const packets = $derived(seeds.reduce((a, s) => a + s.packets, 0));
  const needAttention = $derived(seeds.filter((s) => s.status !== 'ok').length);
  const statusText: Record<string, string> = { ok: 'Good', check: 'Check', replace: 'Replace' };

  function toggleAll() {
    if (allSelected) selected = selected.filter((id) => !shown.some((s) => s.id === id));
    else selected = [...new Set([...selected, ...shown.map((s) => s.id)])];
  }
  function openNew() { form = blank(); dlg.showModal(); }
  function openEdit(s: Seed) { form = { ...blank(), ...s }; dlg.showModal(); }
  async function saveForm() {
    if (!form.crop.trim()) return;
    const n = (x: unknown) => Number(x) || 0;
    try {
      seeds = await seedSave({ ...form, crop: form.crop.trim(), packets: n(form.packets), seeds_per_packet: n(form.seeds_per_packet), year_packed: Math.round(n(form.year_packed)), germination: Math.min(100, Math.max(0, n(form.germination))), days_to_harvest: n(form.days_to_harvest), target: n(form.target) });
      dlg.close();
    } catch (e) { error = String(e); }
  }
  async function step(s: Seed, by: number) {
    try { seeds = await seedSave({ ...s, packets: Math.max(0, s.packets + by) }); } catch (e) { error = String(e); }
  }
  async function deleteSelected() {
    try { seeds = await seedDelete(selected); selected = []; confirming = false; } catch (e) { error = String(e); }
  }
  function ageText(s: Seed): string {
    if (s.age == null) return 'Year not set';
    const left = (s.years_good ?? 3) - s.age;
    return `${s.age} year${s.age === 1 ? '' : 's'} old` + (left > 0 ? `, about ${left} left` : ', past its usual life');
  }
</script>

<section class="head row">
  <div class="grow">
    <h1>Seed vault</h1>
    <p class="sub">Seeds are food you can grow again. Learn what to buy and how to keep it, and keep count of what you have.</p>
  </div>
  <button class="btn" onclick={() => go('Academy')} title="Open the Academy, where the seed vault course is">Seed vault course →</button>
</section>

{#if error}<div class="warn" role="alert">{error}</div>{/if}

<div class="tabs" role="tablist">
  <button role="tab" aria-selected={tab === 'guides'} class:on={tab === 'guides'} onclick={() => { tab = 'guides'; openGuide = null; }}>Guides: what to buy and how</button>
  <button role="tab" aria-selected={tab === 'mine'} class:on={tab === 'mine'} onclick={() => (tab = 'mine')}>My seeds ({seeds.length})</button>
</div>

{#if tab === 'guides'}
  {#if openGuide}
    <div class="row"><button class="btn" onclick={() => (openGuide = null)}>← All guides</button></div>
    <article class="panel">
      <h2>{openGuide.title}</h2>
      {#each blocks(openGuide.body) as b}
        {#if b.kind === 'p'}<p>{#each parts(b.lines[0]) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</p>
        {:else if b.kind === 'ol'}<ol>{#each b.lines as l}<li>{#each parts(l) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</li>{/each}</ol>
        {:else}<ul>{#each b.lines as l}<li>{#each parts(l) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</li>{/each}</ul>{/if}
      {/each}
    </article>
  {:else}
    <div class="panel intro">
      <strong>Why a seed vault?</strong> Stored food runs out. Seed does not, if you keep it dry and cool, test it and plant it. A small, well-chosen vault lets a family grow food year after year. Start with the guides, then write down what you buy under <em>My seeds</em>.
    </div>
    {#each groups as g (g.title)}
      <div class="label gh">{g.title}</div>
      <div class="list">
        {#each g.list as c (c.id)}
          <button class="card" onclick={() => { openGuide = c; window.scrollTo?.(0, 0); }}><strong>{c.title}</strong>{#if c.status !== 'reviewed'}<span class="badge">Draft · review pending</span>{/if}</button>
        {/each}
      </div>
    {/each}
    {#if other.length}
      <div class="label gh">MORE</div>
      <div class="list">{#each other as c (c.id)}<button class="card" onclick={() => (openGuide = c)}><strong>{c.title}</strong></button>{/each}</div>
    {/if}
    {#if !guides.length}<p class="muted">No guides found. They are text files in the <code>seed-guides</code> folder.</p>{/if}
  {/if}
{:else}
  <div class="row top">
    <span class="muted">{seeds.length} kinds · {packets} packets{needAttention ? ` · ${needAttention} to check` : ''}</span>
    <button class="plus" aria-label="Add seeds" title="Add seeds to the vault" onclick={openNew}>+</button>
  </div>
  <section class="panel main">
    <div class="row">
      <input class="input grow" type="search" placeholder="Search your seeds" aria-label="Search seeds" bind:value={search} />
      <select class="input" aria-label="Show" bind:value={show}>
        <option value="all">Show all</option>
        <option value="low">Old or running low</option>
      </select>
    </div>
    <div class="bar row">
      <label class="row"><input type="checkbox" checked={allSelected} onchange={toggleAll} aria-label="Select all seeds" /> Select all</label>
      <span class="muted">{selected.length} selected</span>
      {#if confirming}
        <span class="act">Delete {selected.length}?</span>
        <button class="btn danger" onclick={deleteSelected}>Yes, delete</button>
        <button class="btn" onclick={() => (confirming = false)}>Cancel</button>
      {:else}
        <button class="btn danger right" disabled={selected.length === 0} onclick={() => (confirming = true)}>Delete selected</button>
      {/if}
    </div>
    {#if shown.length === 0}<p class="muted">{seeds.length ? 'No seeds match.' : 'Nothing in the vault yet. Press + to add your first seeds, or read the guides first.'}</p>{/if}
    {#each shown as s (s.id)}
      <div class="item">
        <input type="checkbox" bind:group={selected} value={s.id} aria-label={'Select ' + s.crop + ' ' + s.variety} />
        <button class="name" onclick={() => openEdit(s)}>
          <strong>{s.crop}{s.variety ? ' · ' + s.variety : ''}</strong>
          <span class="muted small">{s.kind || 'Type not set'}{s.location ? ' · ' + s.location : ''}{s.plant_when ? ' · plant: ' + s.plant_when : ''}{s.target ? ' · want ' + s.target : ''}</span>
        </button>
        <div class="qty">
          <button class="mini" aria-label="Use one packet" onclick={() => step(s, -1)}>−</button>
          <span class="mono">{s.packets} pk</span>
          <button class="mini" aria-label="Add one packet" onclick={() => step(s, 1)}>+</button>
        </div>
        <div class="small cell">{ageText(s)}{s.germination ? ` · tested ${s.germination}%` : ''}</div>
        <div class="small cell st {s.status}">● {statusText[s.status ?? 'ok']}</div>
      </div>
    {/each}
    <div class="warn">How long seed lasts is a rule of thumb from extension services. A germination test is better than any chart: put 10 seeds on a damp paper towel in a bag, keep it warm, and count how many sprout. 8 of 10 is good.</div>
  </section>
{/if}

<dialog bind:this={dlg} aria-labelledby="sdt">
  <form class="dform" onsubmit={(e) => { e.preventDefault(); void saveForm(); }}>
    <h2 id="sdt">{form.id ? 'Edit seeds' : 'Add seeds'}</h2>
    <p class="muted hint">Only the crop is needed. Each extra answer makes the checks and the plan more useful.</p>
    <fieldset>
      <legend>1. What is it?</legend>
      <label>Crop
        <input class="input" list="crops" bind:value={form.crop} required placeholder="Tomato, pole bean, carrot…" />
        <datalist id="crops">{#each CROPS as c}<option value={c}></option>{/each}</datalist>
      </label>
      <div class="row">
        <label class="half">Variety<input class="input" bind:value={form.variety} placeholder="Brandywine, Kentucky Wonder…" /></label>
        <label class="half">Type
          <select class="input" bind:value={form.kind}>{#each KINDS as k}<option>{k}</option>{/each}</select>
          <span class="muted hint">Heirloom and open-pollinated seed grows true from saved seed. Hybrid does not.</span>
        </label>
      </div>
    </fieldset>
    <fieldset>
      <legend>2. How much?</legend>
      <div class="row">
        <label class="half">Packets you have<input class="input" type="number" min="0" step="any" bind:value={form.packets} /></label>
        <label class="half">Seeds in a packet (about)<input class="input" type="number" min="0" bind:value={form.seeds_per_packet} /></label>
      </div>
      <label>Packets you want to keep<input class="input" type="number" min="0" step="any" bind:value={form.target} /><span class="muted hint">0 means no goal. Below it, the seed shows as “Check”.</span></label>
    </fieldset>
    <fieldset>
      <legend>3. Age and testing</legend>
      <div class="row">
        <label class="half">Year harvested or packed<input class="input" type="number" min="1990" max="2100" bind:value={form.year_packed} /></label>
        <label class="half">Last germination test<input class="input" type="date" bind:value={form.tested} /></label>
      </div>
      <label>Percent that sprouted (0 to 100)<input class="input" type="number" min="0" max="100" bind:value={form.germination} /><span class="muted hint">8 of 10 seeds sprouting is 80. A good test says old seed is still fine; under 50 means replace it.</span></label>
    </fieldset>
    <fieldset>
      <legend>4. Growing</legend>
      <label>When to plant<input class="input" bind:value={form.plant_when} placeholder="Spring after the last frost, late summer…" /></label>
      <label>Days from planting to harvest<input class="input" type="number" min="0" bind:value={form.days_to_harvest} /></label>
    </fieldset>
    <fieldset>
      <legend>5. Where</legend>
      <div class="row">
        <label class="half">Where it is kept<input class="input" bind:value={form.location} placeholder="Fridge door, seed box…" /></label>
        <label class="half">Where you got it<input class="input" bind:value={form.source} placeholder="Company, neighbour, saved yourself" /></label>
      </div>
    </fieldset>
    <label>Notes<input class="input" bind:value={form.notes} placeholder="How it grew, how deep to plant, how far apart" /></label>
    <div class="row">
      <button class="btn primary" type="submit">Save seeds</button>
      <button class="btn" type="button" onclick={() => dlg.close()}>Cancel</button>
    </div>
  </form>
</dialog>

<style>
  .head { justify-content: space-between; flex-wrap: nowrap; }
  .grow { flex: 1 1 200px; }
  .tabs { display: flex; gap: 6px; flex-wrap: wrap; margin: 8px 0 12px; }
  .tabs button { padding: 8px 14px; border-radius: 999px; border: 1px solid var(--line2); background: var(--panel); color: #b5ada0; }
  .tabs button.on { background: var(--blue-soft); border-color: var(--blue-line); color: var(--blue); font-weight: 600; }
  .intro { max-width: 760px; }
  .gh { margin-top: 16px; }
  .list { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; margin: 8px 0; }
  .card { text-align: left; padding: 14px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); display: flex; flex-direction: column; gap: 6px; }
  .card:hover { border-color: var(--blue-line); }
  .badge { align-self: flex-start; }
  article { max-width: 760px; margin-top: 10px; }
  article p, article li { line-height: 1.6; }
  article ol, article ul { padding-left: 22px; }
  .top { justify-content: space-between; }
  .plus { width: 48px; height: 48px; border-radius: 50%; border: 0; background: var(--blue); color: #07131d; font-size: 30px; font-weight: 700; line-height: 1; }
  .main { display: flex; flex-direction: column; gap: 10px; }
  .bar { padding: 8px 10px; border-radius: 10px; background: #241512; border: 1px solid #6b2a20; }
  .right { margin-left: auto; }
  .item { display: grid; grid-template-columns: 24px minmax(0, 1fr) 120px 230px 80px; gap: 6px 14px; align-items: center; padding: 10px 0; border-top: 1px solid var(--line); }
  .item input[type='checkbox'], .bar input[type='checkbox'] { width: 20px; height: 20px; accent-color: var(--blue); }
  .name { display: flex; flex-direction: column; text-align: left; background: none; border: 0; padding: 0; }
  .qty { display: flex; align-items: center; gap: 6px; }
  .mini { width: 26px; height: 26px; border-radius: 6px; border: 1px solid var(--line2); background: var(--panel2); }
  .mono { font-family: var(--mono); font-size: 13px; min-width: 52px; text-align: center; white-space: nowrap; }
  .small { font-size: 13px; }
  .st.ok { color: #6fcf8d; } .st.check { color: #e0b44c; } .st.replace { color: #e5675a; }
  @media (max-width: 760px) {
    .item { grid-template-columns: 24px 1fr; }
    .item .name { grid-column: 2; }
    .item .qty, .item .cell { grid-column: 2; }
  }
  dialog { max-height: 92vh; overflow: auto; max-width: min(640px, 96vw); }
  .dform { display: flex; flex-direction: column; gap: 10px; padding: 18px; }
  .dform label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--muted); }
  .dform fieldset { border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px 12px; display: flex; flex-direction: column; gap: 10px; margin: 0; }
  .dform legend { font-size: 13px; color: var(--blue); padding: 0 6px; }
  .half { flex: 1 1 140px; }
  .hint { font-size: 12px; display: block; margin-top: 2px; }
</style>
