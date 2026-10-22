<script lang="ts">
  import { inventoryList, inventorySave, inventoryDelete, type Item } from '../lib/api';
  import { onMount } from 'svelte';
  import { categories, categoryHint } from '../lib/categories';

  let items: Item[] = $state([]);
  let error = $state('');
  let category = $state('All');
  let location = $state('All');
  let search = $state('');
  let show = $state('all');
  let selected: string[] = $state([]);
  let confirming = $state(false);
  let dlg: HTMLDialogElement;
  let form: Item = $state(blank());

  function blank(): Item {
    return { id: '', category: 'Water', name: '', location: '', quantity: 1, unit: '', use_per_day: 0, use_by: '', notes: '' };
  }

  onMount(load);

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
    form = { ...i };
    dlg.showModal();
  }
  async function saveForm() {
    if (!form.name.trim()) return;
    try {
      items = await inventorySave({ ...form, name: form.name.trim(), quantity: Number(form.quantity) || 0, use_per_day: Number(form.use_per_day) || 0 });
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
          <span class="muted small">{i.location || 'No place set'}{i.notes ? ' · ' + i.notes : ''}</span>
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
    <label>Item<input class="input" bind:value={form.name} required /></label>
    <label>Category
      <select class="input" bind:value={form.category}>{#each categories as c}<option>{c}</option>{/each}</select>
      {#if categoryHint[form.category]}<span class="muted hint">For example: {categoryHint[form.category]}</span>{/if}
    </label>
    <label>Where it is<input class="input" bind:value={form.location} placeholder="Pantry, Garage, Go-bag..." /></label>
    <div class="row">
      <label class="half">How many<input class="input" type="number" min="0" step="any" bind:value={form.quantity} /></label>
      <label class="half">Unit<input class="input" bind:value={form.unit} placeholder="gal, meals, pcs" /></label>
    </div>
    <div class="row">
      <label class="half">Used per day<input class="input" type="number" min="0" step="any" bind:value={form.use_per_day} /></label>
      <label class="half">Use by (YYYY-MM)<input class="input" bind:value={form.use_by} placeholder="2027-06" pattern="\d{'{4}'}-\d{'{2}'}|" /></label>
    </div>
    <label>Notes<input class="input" bind:value={form.notes} /></label>
    <div class="row">
      <button class="btn primary" type="submit">Save item</button>
      <button class="btn" type="button" onclick={() => dlg.close()}>Cancel</button>
    </div>
  </form>
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
</style>
