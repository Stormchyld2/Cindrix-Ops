<script lang="ts">
  import { inventoryList, inventorySave, inventoryDelete, stockNeeds, profileLoad, type Item, type Need } from '../lib/api';
  import { onMount } from 'svelte';
  import { categories, categoryHint, categoryUnits, categoryAsk, forWhomChoices, conditionChoices } from '../lib/categories';
  import { learner, loadPeople } from '../lib/learner.svelte';

  let items: Item[] = $state([]);
  let error = $state('');
  let category = $state('All');
  let location = $state('All');
  let search = $state('');
  let show = $state('all');
  let selected: string[] = $state([]);
  let confirming = $state(false);
  let dlg: HTMLDialogElement;
  let plandlg: HTMLDialogElement;
  // the plan-what-to-have dialog
  let hh = $state(loadHousehold());
  let needs: Need[] = $state([]);
  let picks: string[] = $state([]);
  let applied = $state('');
  function loadHousehold() {
    try {
      const t = localStorage.getItem('cindrix.household');
      if (t) return { adults: 2, children: 0, babies: 0, small: 0, large: 0, days: 14, ...JSON.parse(t) };
    } catch { /* use the defaults */ }
    return { adults: 2, children: 0, babies: 0, small: 0, large: 0, days: 14 };
  }
  const needCategories = $derived([...new Set(needs.map((n) => n.category))]);
  function have(name: string): number | null {
    const m = items.filter((i) => i.name.toLowerCase() === name.toLowerCase());
    return m.length ? m.reduce((a, i) => a + i.quantity, 0) : null;
  }
  let fromDetails = $state('');
  // the numbers saved in Settings > My details
  async function useMyDetails() {
    fromDetails = '';
    try {
      const d = await profileLoad();
      if (d.adults + d.children + d.babies + d.small_pets + d.large_pets === 0) { fromDetails = 'Nobody is entered in My details yet (Settings > My details).'; return; }
      hh.adults = d.adults; hh.children = d.children; hh.babies = d.babies; hh.small = d.small_pets; hh.large = d.large_pets;
      fromDetails = 'Filled in from My details.';
      await showNeeds();
    } catch (e) { error = String(e); }
  }
  async function showNeeds() {
    applied = '';
    try { localStorage.setItem('cindrix.household', JSON.stringify(hh)); } catch { /* not remembered */ }
    try {
      const n = (x: unknown) => Math.max(0, Math.floor(Number(x) || 0));
      needs = await stockNeeds(n(hh.adults), n(hh.children), n(hh.babies), n(hh.small), n(hh.large), n(hh.days) || 14);
      picks = needs.map((x) => x.name);
      if (!needs.length) error = 'Add at least one person to see what to have.';
    } catch (e) {
      error = String(e);
    }
  }
  async function applyNeeds() {
    let added = 0, updated = 0;
    try {
      for (const n of needs.filter((x) => picks.includes(x.name))) {
        const ex = items.find((i) => i.name.toLowerCase() === n.name.toLowerCase());
        if (ex) {
          items = await inventorySave({ ...ex, target: n.amount, size: ex.size || '' });
          updated++;
        } else {
          items = await inventorySave({ ...blank(), category: n.category, name: n.name, quantity: 0, unit: n.unit, target: n.amount, notes: n.why });
          added++;
        }
      }
      applied = `Done: ${updated} updated, ${added} added. Open “Low or expiring” to see what is short.`;
      show = 'low';
    } catch (e) {
      error = String(e);
    }
  }
  function openPlan() {
    applied = '';
    plandlg.showModal();
  }
  let form: Item = $state(blank());

  function blank(): Item {
    return { id: '', category: 'Water', name: '', location: '', quantity: 1, unit: '', use_per_day: 0, use_by: '', notes: '', brand: '', size: '', for_whom: '', bought: '', target: 0, condition: '', checked: '', cost: 0, source: '', serial: '' };
  }

  onMount(async () => {
    if (!learner.loaded) void loadPeople();
    await load();
  });

  async function load() {
    try {
      items = await inventoryList();
      error = '';
    } catch (e) {
      error = String(e);
    }
  }

  let locations = $derived(['All', ...new Set(items.map((i) => i.location).filter((l) => l))]);
  let shown = $derived(
    items.filter((i) => {
      if (category !== 'All' && i.category !== category) return false;
      if (location !== 'All' && i.location !== location) return false;
      if (search && !(i.name + ' ' + i.notes).toLowerCase().includes(search.toLowerCase())) return false;
      if (show === 'low' && i.status === 'ok') return false;
      return true;
    }),
  );
  let allSelected = $derived(shown.length > 0 && shown.every((i) => selected.includes(i.id)));

  function count(cat: string): number {
    return items.filter((i) => i.category === cat).length;
  }

  function toggleAll() {
    if (allSelected) selected = selected.filter((id) => !shown.some((i) => i.id === id));
    else selected = [...new Set([...selected, ...shown.map((i) => i.id)])];
  }

  function openNew() {
    form = { ...blank(), category: category === 'All' ? 'Water' : category };
    dlg.showModal();
  }
  function openEdit(i: Item) {
    form = { ...blank(), ...i };
    dlg.showModal();
  }
  async function saveForm() {
    if (!form.name.trim()) return;
    try {
      items = await inventorySave({ ...form, name: form.name.trim(), quantity: Number(form.quantity) || 0, use_per_day: Number(form.use_per_day) || 0, target: Number(form.target) || 0, cost: Number(form.cost) || 0 });
      dlg.close();
    } catch (e) {
      error = String(e);
    }
  }
  async function step(i: Item, by: number) {
    const q = Math.max(0, i.quantity + by);
    try {
      items = await inventorySave({ ...i, quantity: q });
    } catch (e) {
      error = String(e);
    }
  }
  async function deleteSelected() {
    try {
      items = await inventoryDelete(selected);
      selected = [];
      confirming = false;
    } catch (e) {
      error = String(e);
    }
  }

  function lasts(i: Item): string {
    if (i.days_left == null) return '—';
    return i.days_left < 10 ? i.days_left.toFixed(1) + ' days' : Math.round(i.days_left) + ' days';
  }
  const statusText: Record<string, string> = { ok: 'OK', check: 'Check', act: 'Act now' };
</script>

<section class="head row">
  <div class="grow">
    <h1>Inventory</h1>
    <p class="sub">Everything you have, where it is, how long it lasts and when it expires.</p>
  </div>
  <button class="btn" onclick={openPlan} title="Work out what to have on hand and how much, for your household">How much should I have?</button>
  <button class="plus" aria-label="Create new item" title="Create new item" onclick={openNew}>+</button>
</section>

{#if error}<div class="warn" role="alert">{error}</div>{/if}

<div class="layout">
  <aside class="panel">
    <div class="label">CATEGORIES</div>
    <button class="cat" class:on={category === 'All'} onclick={() => (category = 'All')}><span>All</span><span class="muted">{items.length}</span></button>
    {#each categories as c}
      <button class="cat" class:on={category === c} onclick={() => (category = c)}><span>{c}</span><span class="muted">{count(c)}</span></button>
    {/each}
  </aside>

  <section class="panel main">
    <select class="input catselect" aria-label="Category" bind:value={category}>
      <option value="All">All categories ({items.length})</option>
      {#each categories as c}<option value={c}>{c} ({count(c)})</option>{/each}
    </select>
    <div class="row">
      <input class="input grow" type="search" placeholder="Search your inventory" aria-label="Search" bind:value={search} />
      <select class="input" aria-label="Show" bind:value={show}>
        <option value="all">Show all</option>
        <option value="low">Low or expiring</option>
      </select>
    </div>
    <div class="row chips">
      {#each locations as l}
        <button class="chip" class:on={location === l} onclick={() => (location = l)}>{l === 'All' ? 'All locations' : l}</button>
      {/each}
    </div>

    <div class="bar row">
      <label class="row"><input type="checkbox" checked={allSelected} onchange={toggleAll} aria-label="Select all items" /> Select all</label>
      <span class="muted">{selected.length} selected</span>
      {#if confirming}
        <span class="act">Delete {selected.length} item{selected.length === 1 ? '' : 's'}?</span>
        <button class="btn danger" onclick={deleteSelected}>Yes, delete</button>
        <button class="btn" onclick={() => (confirming = false)}>Cancel</button>
      {:else}
        <button class="btn danger right" disabled={selected.length === 0} onclick={() => (confirming = true)}>Delete selected</button>
      {/if}
    </div>

    {#if shown.length === 0}
      <p class="muted">Nothing here yet. Tap + to add an item.</p>
    {/if}
    {#each shown as i (i.id)}
      <div class="item">
        <input type="checkbox" bind:group={selected} value={i.id} aria-label={'Select ' + i.name} />
        <button class="name" onclick={() => openEdit(i)}>
          <strong>{i.name}</strong>
          <span class="muted small">{i.location || 'No place set'}{i.brand ? ' · ' + i.brand : ''}{i.size ? ' · ' + i.size : ''}{i.for_whom ? ' · for ' + i.for_whom : ''}{i.target ? ' · want ' + i.target : ''}{i.notes ? ' · ' + i.notes : ''}</span>
        </button>
        <div class="qty">
          <button class="mini" aria-label="Remove one" onclick={() => step(i, -1)}>−</button>
          <span class="mono">{i.quantity} {i.unit}</span>
          <button class="mini" aria-label="Add one" onclick={() => step(i, 1)}>+</button>
        </div>
        <div class="small cell lasts">{lasts(i)}</div>
        <div class="small cell useby">{i.use_by || '—'}</div>
        <div class="small cell st {i.status}">● {statusText[i.status ?? 'ok']}</div>
      </div>
    {/each}

    <div class="warn">Medicine is counted only. Doses and changes are never suggested here. Follow your doctor's written plan.</div>
  </section>
</div>

<dialog bind:this={dlg} aria-labelledby="dt">
  <form class="dform" onsubmit={(e) => { e.preventDefault(); saveForm(); }}>
    <h2 id="dt">{form.id ? 'Edit item' : 'New item'}</h2>
    <p class="muted hint">Only the item name is needed. Each extra answer makes “days left”, the shopping list and the checks more true.</p>

    <fieldset>
      <legend>1. What is it?</legend>
      <label>Item<input class="input" bind:value={form.name} required placeholder="Bottled water" /></label>
      <label>Category
        <select class="input" bind:value={form.category}>{#each categories as c}<option>{c}</option>{/each}</select>
        {#if categoryHint[form.category]}<span class="muted hint">For example: {categoryHint[form.category]}</span>{/if}
      </label>
      <div class="row">
        <label class="half">Brand or type<input class="input" bind:value={form.brand} placeholder="Mountain House, AA lithium…" /></label>
        <label class="half">Size of one<input class="input" bind:value={form.size} placeholder="1 gal, 15 oz can, 10 ft" /></label>
      </div>
      <label>Model or serial number<input class="input" bind:value={form.serial} placeholder="For gear you might need to repair or replace" /></label>
    </fieldset>

    <fieldset>
      <legend>2. How much?</legend>
      <div class="row">
        <label class="half">How many do you have<input class="input" type="number" min="0" step="any" bind:value={form.quantity} /></label>
        <label class="half">Unit
          <input class="input" list="units" bind:value={form.unit} placeholder="gal, meals, pcs" />
          <datalist id="units">{#each categoryUnits[form.category] ?? [] as u}<option value={u}></option>{/each}</datalist>
        </label>
      </div>
      <div class="row">
        <label class="half">How many do you want to keep<input class="input" type="number" min="0" step="any" bind:value={form.target} />
          <span class="muted hint">0 means no goal. Below it, the item shows as “Check”. “How much should I have?” fills this in for you.</span></label>
        <label class="half">Used per day<input class="input" type="number" min="0" step="any" bind:value={form.use_per_day} />
          <span class="muted hint">All together, per day. It makes “days left” true.</span></label>
      </div>
    </fieldset>

    <fieldset>
      <legend>3. Who and where?</legend>
      <label>Who is it for
        <input class="input" list="whom" bind:value={form.for_whom} placeholder="Everyone, a name, Pets…" />
        <datalist id="whom">{#each [...forWhomChoices, ...learner.people] as w}<option value={w}></option>{/each}</datalist>
      </label>
      <label>Where it is
        <input class="input" list="places" bind:value={form.location} placeholder="Pantry, Garage, Go-bag…" />
        <datalist id="places">{#each locations.filter((l) => l !== 'All') as l}<option value={l}></option>{/each}</datalist>
      </label>
    </fieldset>

    <fieldset>
      <legend>4. Dates and condition</legend>
      <div class="row">
        <label class="half">Bought or packed<input class="input" type="month" bind:value={form.bought} /></label>
        <label class="half">Use by or replace by<input class="input" type="month" bind:value={form.use_by} /></label>
      </div>
      <div class="row">
        <label class="half">Last looked at<input class="input" type="date" bind:value={form.checked} /></label>
        <label class="half">Condition
          <select class="input" bind:value={form.condition}>{#each conditionChoices as c}<option value={c}>{c || 'Not set'}</option>{/each}</select>
        </label>
      </div>
    </fieldset>

    <fieldset>
      <legend>5. Buying</legend>
      <div class="row">
        <label class="half">Cost (all together, $)<input class="input" type="number" min="0" step="any" bind:value={form.cost} /></label>
        <label class="half">Where to get more<input class="input" bind:value={form.source} placeholder="Store, website, neighbour" /></label>
      </div>
    </fieldset>

    {#if categoryAsk[form.category]?.length}
      <div class="ask">
        <strong>For {form.category.toLowerCase()}, also think about:</strong>
        <ul>{#each categoryAsk[form.category] as q}<li>{q}</li>{/each}</ul>
      </div>
    {/if}

    <label>Notes<input class="input" bind:value={form.notes} /></label>
    <div class="row">
      <button class="btn primary" type="submit">Save item</button>
      <button class="btn" type="button" onclick={() => dlg.close()}>Cancel</button>
    </div>
  </form>
</dialog>

<dialog bind:this={plandlg} aria-labelledby="pt" class="wide">
  <div class="dform">
    <h2 id="pt">How much should I have?</h2>
    <p class="muted hint">Tell us who you are looking after and for how many days. The list below shows what to have on hand, from the Red Cross and Ready.gov where they give a number, and a plain rule of thumb where they do not (each line says which). Then set them as your goals, and the Inventory will show what is short.</p>
    <div class="row">
      <button class="btn" type="button" onclick={useMyDetails} title="Fill in the numbers from Settings, My details">Use my details</button>
      {#if fromDetails}<span class="muted small">{fromDetails}</span>{/if}
    </div>
    <div class="row">
      <label class="third">Adults<input class="input" type="number" min="0" bind:value={hh.adults} /></label>
      <label class="third">Children<input class="input" type="number" min="0" bind:value={hh.children} /></label>
      <label class="third">Babies<input class="input" type="number" min="0" bind:value={hh.babies} /></label>
      <label class="third">Small pets<input class="input" type="number" min="0" bind:value={hh.small} /></label>
      <label class="third">Large dogs<input class="input" type="number" min="0" bind:value={hh.large} /></label>
      <label class="third">For how long
        <select class="input" bind:value={hh.days}>
          <option value={3}>3 days (to leave with)</option>
          <option value={7}>1 week</option>
          <option value={14}>2 weeks (for home)</option>
          <option value={30}>1 month</option>
          <option value={90}>3 months</option>
          <option value={180}>6 months</option>
          <option value={365}>1 year</option>
        </select>
      </label>
    </div>
    <div class="row"><button class="btn primary" onclick={showNeeds}>Show what to have</button></div>
    {#if needs.length}
      {#each needCategories as cat}
        <div class="label">{cat.toUpperCase()}</div>
        {#each needs.filter((n) => n.category === cat) as n (n.name)}
          <label class="need">
            <input type="checkbox" checked={picks.includes(n.name)} onchange={() => (picks = picks.includes(n.name) ? picks.filter((x) => x !== n.name) : [...picks, n.name])} aria-label={'Use ' + n.name} />
            <span class="grow"><strong>{n.name}</strong> <span class="mono">{Number.isInteger(n.amount) ? n.amount : n.amount.toFixed(1)} {n.unit}</span>
              {#if have(n.name) != null}<span class="muted small"> · you have {have(n.name)}</span>{/if}
              <span class="muted small block">{n.why}{n.why ? ' ' : ''}<em>({n.basis})</em></span></span>
          </label>
        {/each}
      {/each}
      <p class="muted hint">Items with the same name are updated. The others are added with “0 on hand” so they show as things to get. Rename or delete extras afterwards.</p>
      <div class="row">
        <button class="btn primary" onclick={applyNeeds} disabled={!picks.length}>Set {picks.length} as my goals</button>
        <button class="btn" onclick={() => plandlg.close()}>Close</button>
      </div>
      {#if applied}<div class="done">{applied}</div>{/if}
    {:else}
      <div class="row"><button class="btn" onclick={() => plandlg.close()}>Close</button></div>
    {/if}
  </div>
</dialog>

<style>
  .head { justify-content: space-between; flex-wrap: nowrap; }
  .grow { flex: 1 1 200px; }
  .plus { width: 48px; height: 48px; border-radius: 50%; border: 0; background: var(--blue); color: #07131d; font-size: 30px; font-weight: 700; line-height: 1; }
  .layout { display: flex; flex-wrap: wrap; gap: 16px; align-items: flex-start; }
  aside { flex: 0 1 230px; min-width: 200px; display: flex; flex-direction: column; gap: 4px; }
  .cat { display: flex; justify-content: space-between; padding: 8px 10px; border-radius: 8px; border: 1px solid transparent; background: transparent; text-align: left; }
  .cat.on { background: var(--blue-soft); border-color: var(--blue-line); color: var(--blue); font-weight: 600; }
  .main { flex: 1 1 560px; min-width: 0; display: flex; flex-direction: column; gap: 10px; }
  .chips { gap: 6px; }
  .chip { padding: 6px 12px; border-radius: 999px; border: 1px solid var(--line2); background: var(--panel); font-size: 13px; color: #b5ada0; }
  .chip.on { background: var(--blue-soft); border-color: var(--blue-line); color: var(--blue); font-weight: 600; }
  .bar { padding: 8px 10px; border-radius: 10px; background: #241512; border: 1px solid #6b2a20; }
  .right { margin-left: auto; }
  .item { display: grid; grid-template-columns: 24px minmax(0, 1fr) 150px 80px 70px 80px; gap: 6px 14px; align-items: center; padding: 10px 0; border-top: 1px solid var(--line); }
  .item input[type='checkbox'], .bar input[type='checkbox'] { width: 20px; height: 20px; accent-color: var(--blue); }
  .name { display: flex; flex-direction: column; text-align: left; background: none; border: 0; padding: 0; }
  .qty { display: flex; align-items: center; gap: 6px; }
  .mini { width: 26px; height: 26px; border-radius: 6px; border: 1px solid var(--line2); background: var(--panel2); }
  .cell { white-space: nowrap; }
  .catselect { display: none; }
  @media (max-width: 760px) {
    aside { display: none; }
    .catselect { display: block; width: 100%; }
    .item { grid-template-columns: 24px auto 1fr; }
    .item .name { grid-column: 2 / 4; }
    .item .qty { grid-column: 2; }
    .item .lasts { grid-column: 3; }
    .item .useby { grid-column: 2; }
    .item .st { grid-column: 3; }
  }
  .small { font-size: 13px; }
  .mono { font-family: var(--mono); font-size: 13px; min-width: 62px; text-align: center; white-space: nowrap; }
  .dform { display: flex; flex-direction: column; gap: 10px; padding: 18px; }
  .dform label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--muted); }
  .half { flex: 1 1 140px; }
  .hint { font-size: 12px; display: block; margin-top: 2px; }
  .dform fieldset { border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px 12px; display: flex; flex-direction: column; gap: 10px; margin: 0; }
  .dform legend { font-size: 13px; color: var(--blue); padding: 0 6px; letter-spacing: .04em; }
  .ask { padding: 10px 12px; border-left: 3px solid var(--blue); background: var(--panel2); border-radius: 4px; font-size: 13px; }
  .ask ul { margin: 6px 0 0; padding-left: 18px; }
  dialog { max-height: 92vh; overflow: auto; max-width: min(640px, 96vw); }
  dialog.wide { max-width: min(760px, 96vw); }
  .third { flex: 1 1 110px; }
  .need { display: flex !important; flex-direction: row !important; gap: 10px !important; align-items: flex-start; padding: 8px 0; border-top: 1px solid var(--line); font-size: 14px !important; color: inherit !important; }
  .need input { width: 20px; height: 20px; accent-color: var(--blue); margin-top: 2px; }
  .need .grow { flex: 1; }
  .block { display: block; }
  .done { padding: 10px 12px; border: 1px solid var(--blue-line); background: var(--blue-soft); border-radius: 8px; }
</style>
