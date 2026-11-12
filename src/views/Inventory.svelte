<script lang="ts">
  import PageHeader from '../lib/PageHeader.svelte';
  import Icon from '../lib/Icon.svelte';
  import { inventoryCsv, inventoryImport, exportText, inventoryList, inventorySave, inventoryDelete, scanInboxList, scanInboxPreview, scanInboxAccept, scanInboxDismiss, syncInboxList, syncMerge, stockNeeds, profileLoad, inventoryPlaces, inventoryWhere, shelfLifeSuggest, placeParts, insidePlace, type Item, type Need, type Place, type Found, type ShelfGuess, type ScanFile, type ScanLine } from '../lib/api';
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

  // Places inside places (docs/42): "Garage > Shelf 2 > Black pail".
  let places: Place[] = $state([]);
  let found: Found[] = $state([]);
  async function refreshPlaces() {
    try { places = await inventoryPlaces(); } catch { places = []; }
  }
  $effect(() => {
    items; // run again whenever the list changes
    void refreshPlaces();
  });
  $effect(() => {
    const q = search.trim();
    if (q.length < 2) { found = []; return; }
    const t = setTimeout(async () => {
      try { found = await inventoryWhere(q); } catch { found = []; }
    }, 200);
    return () => clearTimeout(t);
  });
  const trail = $derived(placeParts(location === 'All' ? '' : location));
  const steps = $derived(trail.map((p, n) => ({ name: p, path: trail.slice(0, n + 1).join(' > ') })));
  const inside = $derived(places.filter((p) => p.depth === trail.length && (trail.length === 0 || insidePlace(p.path, location))));
  function openFound(f: Found) {
    const it = items.find((i) => i.id === f.id);
    if (it) openEdit(it);
  }

  // How food is packed, and a use-by month worked out from it (docs/42).
  const packagingChoices: [string, string][] = [
    ['mylar', 'Foil (mylar) pouch'], ['o2', 'Oxygen absorber inside'], ['vacuum', 'Vacuum sealed'],
    ['sealed-can', 'Sealed metal can'], ['jar', 'Glass jar with a gasket lid'], ['bucket', 'Food-grade bucket with a gasket lid'],
    ['desiccant', 'Desiccant packet'],
  ];
  const hasPack = (code: string) => (form.packaging ?? '').split(',').includes(code);
  function togglePack(code: string) {
    const have = (form.packaging ?? '').split(',').filter(Boolean);
    form.packaging = (have.includes(code) ? have.filter((c) => c !== code) : [...have, code]).join(',');
    guess = null; guessNote = '';
  }
  let guess: ShelfGuess | null = $state(null);
  let guessNote = $state('');
  async function suggestUseBy() {
    guess = null; guessNote = '';
    if (!form.name.trim()) { guessNote = 'Type the item name first.'; return; }
    try {
      const g = await shelfLifeSuggest(form.name, form.packaging ?? '', form.bought ?? '');
      if (!g) { guessNote = 'There is no shelf-life guess for this item. Use the date on the package.'; return; }
      guess = g;
    } catch (e) { guessNote = String(e); }
  }
  function useGuess() {
    if (guess?.use_by) form.use_by = guess.use_by;
    if (guess?.assumed_today && !form.bought) form.bought = guess.use_by ? new Date().toISOString().slice(0, 7) : '';
    guess = null;
  }

  function blank(): Item {
    return { id: '', category: 'Water', name: '', location: '', quantity: 1, unit: '', use_per_day: 0, use_by: '', notes: '', brand: '', size: '', for_whom: '', bought: '', target: 0, condition: '', checked: '', cost: 0, source: '', serial: '', packaging: '' };
  }

  // Files left by Cindrix Scan, waiting until the person says to add them.
  let waiting: ScanFile[] = $state([]);
  let peek = $state('');
  let peekLines: ScanLine[] = $state([]);
  let scanNote = $state('');
  async function loadWaiting() {
    try { waiting = await scanInboxList(); } catch { waiting = []; }
  }
  async function look(name: string) {
    if (peek === name) { peek = ''; return; }
    try { peekLines = await scanInboxPreview(name); peek = name; } catch (e) { error = String(e); }
  }
  async function takeIn(name: string) {
    try {
      const r = await scanInboxAccept(name);
      const bits = [];
      if (r.added) bits.push(`${r.added} added`);
      if (r.removed) bits.push(`${r.removed} taken out`);
      if (r.repeated) bits.push(`${r.repeated} already done before`);
      scanNote = bits.join(', ') + '.' + (r.missing.length ? ` There was less than expected of: ${r.missing.join(', ')}.` : '');
      peek = '';
      await loadWaiting();
      await load();
    } catch (e) { error = String(e); }
  }
  async function setAside(name: string) {
    try { await scanInboxDismiss(name); peek = ''; await loadWaiting(); } catch (e) { error = String(e); }
  }

  // A copy of the inventory from the other editing device (the Windows computer or the phone that hosts the server).
  let syncWaiting: string[] = $state([]);
  async function loadSync() {
    try { syncWaiting = await syncInboxList(); } catch { syncWaiting = []; }
  }
  async function mergeCopy(name: string) {
    try {
      const r = await syncMerge(name);
      const bits = [];
      if (r.added) bits.push(`${r.added} new`);
      if (r.removed) bits.push(`${r.removed} removed`);
      if (r.combined) bits.push(`${r.combined} counts combined`);
      scanNote = 'Joined with the other copy' + (bits.length ? ': ' + bits.join(', ') : ': nothing differed') + '.' + (r.notes.length ? ' ' + r.notes.join(' ') : '');
      await loadSync();
      await load();
    } catch (e) { error = String(e); }
  }

  onMount(async () => {
    if (!learner.loaded) void loadPeople();
    await load();
    await loadWaiting();
    await loadSync();
  });

  async function load() {
    try {
      items = await inventoryList();
      error = '';
    } catch (e) {
      error = String(e);
    }
  }

  let locations = $derived(['All', ...places.map((p) => p.path)]);
  let shown = $derived(
    items.filter((i) => {
      if (category !== 'All' && i.category !== category) return false;
      if (location !== 'All' && !insidePlace(i.location, location)) return false;
      if (search && !(i.name + ' ' + i.notes + ' ' + (i.brand ?? '') + ' ' + i.location).toLowerCase().includes(search.toLowerCase())) return false;
      if (show === 'low' && i.status === 'ok') return false;
      return true;
    }),
  );
  let allSelected = $derived(shown.length > 0 && shown.every((i) => selected.includes(i.id)));

  // the list of categories: the built-in ones plus the person's own, minus any they hide (this only hides the name from the list; items stay)
  type CatCfg = { extra: string[]; hidden: string[] };
  function loadCats(): CatCfg {
    try { const t = localStorage.getItem('cindrix.inventory.cats'); if (t) { const j = JSON.parse(t); return { extra: j.extra ?? [], hidden: j.hidden ?? [] }; } } catch { /* use the defaults */ }
    return { extra: [], hidden: [] };
  }
  let catCfg: CatCfg = $state(loadCats());
  function saveCats() { try { localStorage.setItem('cindrix.inventory.cats', JSON.stringify(catCfg)); } catch { /* not kept */ } }
  const builtIn: string[] = [...categories];
  const allCats = $derived([...builtIn, ...catCfg.extra.filter((c) => !builtIn.includes(c))]);
  const cats = $derived(allCats.filter((c) => !catCfg.hidden.includes(c)));
  const isOwn = (c: string) => catCfg.extra.includes(c) && !builtIn.includes(c);
  const usedCategories = $derived(cats.length);
  let catDlg: HTMLDialogElement;
  let catNew = $state('');
  let catNote = $state('');
  let renaming = $state('');
  let renameTo = $state('');
  function addCat() {
    const n = catNew.trim().replace(/[,\n\r]/g, ' ').replace(/\s+/g, ' ');
    catNote = '';
    if (!n) return;
    if (allCats.some((c) => c.toLowerCase() === n.toLowerCase())) { catNote = 'You already have a category with that name.'; return; }
    catCfg.extra = [...catCfg.extra, n];
    saveCats();
    catNew = '';
    catNote = `Added "${n}".`;
  }
  function toggleHide(c: string) {
    catCfg.hidden = catCfg.hidden.includes(c) ? catCfg.hidden.filter((x) => x !== c) : [...catCfg.hidden, c];
    if (category === c) category = 'All';
    saveCats();
  }
  async function doRename(c: string) {
    const n = renameTo.trim().replace(/[,\n\r]/g, ' ').replace(/\s+/g, ' ');
    catNote = '';
    if (!n || n === c) { renaming = ''; return; }
    if (allCats.some((x) => x.toLowerCase() === n.toLowerCase())) { catNote = 'You already have a category with that name.'; return; }
    try {
      for (const i of items.filter((x) => x.category === c)) await inventorySave({ ...i, category: n });
      items = await inventoryList();
    } catch (e) { catNote = String(e); return; }
    catCfg.extra = catCfg.extra.map((x) => (x === c ? n : x));
    catCfg.hidden = catCfg.hidden.map((x) => (x === c ? n : x));
    if (category === c) category = n;
    saveCats();
    renaming = '';
    catNote = `Renamed to "${n}". Its items moved with it.`;
  }
  function removeCat(c: string) {
    if (count(c) > 0) { catNote = 'Move or delete the items in it first.'; return; }
    catCfg.extra = catCfg.extra.filter((x) => x !== c);
    catCfg.hidden = catCfg.hidden.filter((x) => x !== c);
    if (category === c) category = 'All';
    saveCats();
    catNote = `Removed "${c}".`;
  }
  function showOnly(c: string) { category = c; catDlg.close(); }
  const inStock = $derived(items.filter((i) => i.quantity > 0).length);
  const nowMonth = new Date().toISOString().slice(0, 7);
  const isExpired = (i: Item) => !!i.use_by && i.use_by < nowMonth;
  const expiredCount = $derived(items.filter(isExpired).length);
  const lowCount = $derived(items.filter((i) => !isExpired(i) && (i.status ?? 'ok') !== 'ok').length);
  const healthPct = $derived(items.length ? Math.round((items.filter((i) => (i.status ?? 'ok') === 'ok' && !isExpired(i)).length / items.length) * 100) : 0);

  // pictures for the category list
  const CAT_ICON: Record<string, string> = {
    'Water': 'drop', 'Food': 'fork', 'Medical and diabetes': 'medical', 'Power and light': 'bolt', 'Tools and repair': 'tools',
    'Shelter and clothing': 'shield', 'Hygiene and sanitation': 'drop', 'Documents and cash': 'folder', 'Cooking and fuel': 'bolt',
    'Communication and navigation': 'comms', 'Safety and protection': 'shield', 'Babies and children': 'users', 'Pets and animals': 'user',
    'Vehicle and go-bag': 'run', 'Comfort and morale': 'heart', 'Fuel and heat': 'bolt', 'Cleaning and laundry': 'drop',
    'Garden and long-term food': 'fork', 'Medical equipment and special needs': 'pulse', 'Fishing and outdoors': 'tag',
  };
  const catIcon = (c: string) => CAT_ICON[c] ?? 'tag';
  // import and export
  let ioDlg: HTMLDialogElement;
  let ioNote = $state('');
  async function exportAll() {
    ioNote = '';
    try { ioNote = 'Saved: ' + (await exportText('inventory.csv', await inventoryCsv())) + '. It opens in a spreadsheet.'; } catch (e) { ioNote = String(e); }
  }
  async function importFile(e: Event) {
    ioNote = '';
    const f = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!f) return;
    try {
      const r = await inventoryImport(await f.text());
      items = await inventoryList();
      ioNote = `Added ${r.added} item${r.added === 1 ? '' : 's'}.${r.skipped ? ` ${r.skipped} already there, so left alone.` : ''} Nothing you already had was changed.`;
    } catch (err) { ioNote = String(err); }
    (e.currentTarget as HTMLInputElement).value = '';
  }

  function count(cat: string): number {
    return items.filter((i) => i.category === cat).length;
  }

  function toggleAll() {
    if (allSelected) selected = selected.filter((id) => !shown.some((i) => i.id === id));
    else selected = [...new Set([...selected, ...shown.map((i) => i.id)])];
  }

  function openNew() {
    form = { ...blank(), category: category === 'All' ? 'Water' : category };
    guess = null; guessNote = '';
    dlg.showModal();
  }
  function openEdit(i: Item) {
    form = { ...blank(), ...i };
    guess = null; guessNote = '';
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

<PageHeader icon="cube" title="Inventory" tagline="Everything you have, where it is, how long it lasts and when it expires.">
  <button class="btn ico" aria-label="Create new item" title="Create new item" onclick={openNew}><Icon name="plus" size={18} /> Add item</button>
  <button class="btn ico" onclick={() => { catNote = ''; renaming = ''; catDlg.showModal(); }} title="Show only one kind of item, or add, rename and hide categories"><Icon name="layers" size={18} /> Categories</button>
  <button class="btn ico" onclick={() => { ioNote = ''; ioDlg.showModal(); }} title="Save the Inventory as a spreadsheet file, or add items from one"><Icon name="swap" size={18} /> Import / Export</button>
  <button class="btn ico" onclick={openPlan} title="Work out what to have on hand and how much, for your household"><Icon name="help" size={18} /> How much should I have?</button>
</PageHeader>
<div class="invtiles">
  <div class="mt" style="--c: var(--glow-blue)"><span class="mico"><Icon name="cube" size={40} /></span><span><div class="mt-t">Total items</div><div class="mt-n">{items.length}</div></span></div>
  <div class="mt" style="--c: var(--glow-blue)"><span class="mico"><Icon name="layers" size={40} /></span><span><div class="mt-t">Categories</div><div class="mt-n">{usedCategories}</div></span></div>
  <div class="mt" style="--c: var(--glow-green)"><span class="mico"><Icon name="check" size={40} /></span><span><div class="mt-t">In stock</div><div class="mt-n">{inStock}</div></span></div>
  <button class="mt" style="--c: var(--glow-amber)" onclick={() => (show = 'low')} title="Show what is low or to check"><span class="mico"><Icon name="warn" size={40} /></span><span><div class="mt-t">Low stock</div><div class="mt-n">{lowCount}</div></span></button>
  <button class="mt" style="--c: var(--glow-red)" onclick={() => (show = 'low')} title="Show what is past its date"><span class="mico"><Icon name="calx" size={40} /></span><span><div class="mt-t">Expired</div><div class="mt-n">{expiredCount}</div></span></button>
  <div class="mt health" style="--c: var(--glow-blue)"><span class="grow"><div class="mt-t">Inventory health</div><div class="hbar"><span style="width:{healthPct}%"></span></div></span><span class="mt-n">{healthPct}%</span></div>
</div>

{#if error}<div class="warn" role="alert">{error}</div>{/if}

{#if scanNote}<div class="okbar" role="status">{scanNote} <button class="mini" onclick={() => (scanNote = '')}>OK</button></div>{/if}

{#if syncWaiting.length}
  <section class="panel waiting" aria-label="Copy from the other device">
    <h2>A copy of the inventory from your other device</h2>
    <p class="muted">Joining keeps both sets of changes. Counts are added together, not replaced.</p>
    {#each syncWaiting as n (n)}
      <div class="wrow">
        <div class="grow"><strong>{n}</strong></div>
        <button class="btn primary" onclick={() => mergeCopy(n)} title="Join this copy with yours">Join with mine</button>
      </div>
    {/each}
  </section>
{/if}

{#if waiting.length}
  <section class="panel waiting" aria-label="Waiting from Scan">
    <h2>Waiting from your phone ({waiting.length})</h2>
    <p class="muted">Lists sent by Cindrix Scan. Nothing changes until you add them.</p>
    {#each waiting as w (w.name)}
      <div class="wrow">
        <div class="grow">
          <strong>{w.from || 'A phone'}</strong>
          <span class="muted">: {w.adds} to add{w.removes ? `, ${w.removes} to take out` : ''}</span>
        </div>
        <button class="btn" onclick={() => look(w.name)}>{peek === w.name ? 'Hide' : 'Look'}</button>
        <button class="btn primary" onclick={() => takeIn(w.name)} title="Put these into your inventory">Add to inventory</button>
        <button class="btn" onclick={() => setAside(w.name)} title="Set this list aside without using it">Not now</button>
      </div>
      {#if peek === w.name}
        <ul class="plines">
          {#each peekLines as l}
            <li>{l.remove ? 'Take out' : 'Add'} {l.qty} {l.unit} {l.name}{l.use_by ? `, use by ${l.use_by}` : ''}{l.location ? `, ${l.location}` : ''}</li>
          {/each}
        </ul>
      {/if}
    {/each}
  </section>
{/if}

<div class="layout">
  <aside class="dbl" style="--c: var(--glow-blue)">
    <div class="pnl-h nosl"><Icon name="users" size={22} /> Categories</div>
    <button class="cat" class:on={category === 'All'} onclick={() => (category = 'All')}><span class="cn"><Icon name="grid" size={16} /> All items</span><span>{items.length}</span></button>
    {#each cats as c}
      <button class="cat" class:on={category === c} onclick={() => (category = c)}><span class="cn"><Icon name={catIcon(c)} size={16} /> {c}</span><span>{count(c)}</span></button>
    {/each}
  </aside>

  <section class="dbl main" style="--c: var(--glow-blue)">
    <select class="input catselect" aria-label="Category" bind:value={category}>
      <option value="All">All categories ({items.length})</option>
      {#each cats as c}<option value={c}>{c} ({count(c)})</option>{/each}
    </select>
    <div class="row">
      <label class="srch grow"><Icon name="search" size={20} /><input type="search" placeholder="Search your inventory, or ask where something is…" aria-label="Search" bind:value={search} /></label>
      <select class="input" aria-label="Show" bind:value={show}>
        <option value="all">Show all</option>
        <option value="low">Low or expiring</option>
      </select>
    </div>
    <div class="row chips" aria-label="Where">
      <button class="chip" class:on={location === 'All'} onclick={() => (location = 'All')} title="Show everything, wherever it is">All places</button>
      {#each steps as st, n (st.path)}
        <span class="muted">›</span>
        <button class="chip" class:on={n === steps.length - 1} onclick={() => (location = st.path)} title={'Show what is in ' + st.name}>{st.name}</button>
      {/each}
      {#if inside.length}<span class="muted">{steps.length ? 'Inside it:' : 'Open:'}</span>{/if}
      {#each inside as p (p.path)}
        <button class="chip sub" onclick={() => (location = p.path)} title={'Look inside ' + p.name}>{p.name} <span class="muted">{p.inside}</span></button>
      {/each}
    </div>
    {#if found.length}
      <div class="where" role="status">
        <strong>Where is it?</strong>
        {#each found.slice(0, 8) as f (f.id)}
          <button class="wline" onclick={() => openFound(f)} title="Open this item"><span>{f.name}</span><span class="muted"> · {f.quantity} {f.unit} · {f.place || 'no place set'}</span></button>
        {/each}
        {#if found.length > 8}<span class="muted small">and {found.length - 8} more below</span>{/if}
      </div>
    {/if}

    <div class="bar row selbar">
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
          <strong><span class="ci"><Icon name={catIcon(i.category)} size={20} /></span>{i.name}</strong>
          <span class="muted small">{i.location || 'No place set'}{i.brand ? ' · ' + i.brand : ''}{i.size ? ' · ' + i.size : ''}{i.for_whom ? ' · for ' + i.for_whom : ''}{i.target ? ' · want ' + i.target : ''}{i.notes ? ' · ' + i.notes : ''}</span>
        </button>
        <div class="qty">
          <button class="mini" aria-label="Remove one" onclick={() => step(i, -1)}>−</button>
          <span class="mono">{i.quantity} {i.unit}</span>
          <button class="mini" aria-label="Add one" onclick={() => step(i, 1)}>+</button>
        </div>
        <div class="small cell lasts">{lasts(i)}</div>
        <div class="small cell useby">{i.use_by || '—'}</div>
        <div class="small cell st {i.status}">● {statusText[i.status ?? 'ok']} <button class="kebab" aria-label={'Edit ' + i.name} title="Edit" onclick={() => openEdit(i)}>⋮</button></div>
      </div>
    {/each}

    <div class="medicine"><Icon name="info" size={22} /> Medicine is counted only. Doses and changes are never suggested here. Follow your doctor's written plan.</div>
  </section>
</div>

<dialog bind:this={catDlg} aria-label="Categories">
  <div class="dform">
    <h2>Categories</h2>
    <p class="muted hint">Pick one to show only those items. You can also add your own category, rename one you added, or hide one you do not use. Hiding only takes the name off the list. Your items are never deleted.</p>
    <div class="row"><button class="btn" onclick={() => showOnly('All')}>Show all items ({items.length})</button></div>
    <ul class="catman">
      {#each allCats as c (c)}
        <li class:hid={catCfg.hidden.includes(c)}>
          {#if renaming === c}
            <input class="input grow" aria-label="New name" bind:value={renameTo} onkeydown={(e) => { if (e.key === 'Enter') doRename(c); }} />
            <button class="btn primary" onclick={() => doRename(c)}>Save name</button>
            <button class="btn" onclick={() => (renaming = '')}>Cancel</button>
          {:else}
            <span class="cn grow"><Icon name={catIcon(c)} size={16} /> {c} <span class="muted">({count(c)})</span>{#if catCfg.hidden.includes(c)} <em class="muted">hidden</em>{/if}</span>
            {#if !catCfg.hidden.includes(c)}<button class="btn" onclick={() => showOnly(c)} title="Show only {c}">Show only</button>{/if}
            {#if isOwn(c)}
              <button class="btn" onclick={() => { renaming = c; renameTo = c; }}>Rename</button>
              <button class="btn" onclick={() => removeCat(c)} title="Only possible when it has no items">Remove</button>
            {/if}
            {#if c !== 'Water' && c !== 'Food'}<button class="btn" onclick={() => toggleHide(c)}>{catCfg.hidden.includes(c) ? 'Show in list' : 'Hide'}</button>{/if}
          {/if}
        </li>
      {/each}
    </ul>
    <div class="row"><input class="input grow" placeholder="Name of a new category" aria-label="New category" bind:value={catNew} onkeydown={(e) => { if (e.key === 'Enter') addCat(); }} /><button class="btn primary" onclick={addCat}>Add category</button></div>
    <p class="muted hint">Water and Food stay on the list because the Dashboard counts your days from them. The built-in names cannot be renamed for the same reason.</p>
    {#if catNote}<p role="status">{catNote}</p>{/if}
    <div class="row end"><button class="btn" onclick={() => catDlg.close()}>Close</button></div>
  </div>
</dialog>

<dialog bind:this={ioDlg} aria-label="Import or export">
  <div class="dform">
    <h2>Import / Export</h2>
    <p class="muted hint">Export saves everything in your Inventory as one file that opens in a spreadsheet. Import adds the lines from a file like that. A line that is already in your Inventory is skipped, and nothing you already have is changed.</p>
    <div class="row"><button class="btn primary" onclick={exportAll}>Save the Inventory as a file</button></div>
    <label>Add items from a file<input class="input" type="file" accept=".csv,text/csv,text/plain" onchange={importFile} /></label>
    {#if ioNote}<p role="status">{ioNote}</p>{/if}
    <div class="row end"><button class="btn" onclick={() => ioDlg.close()}>Close</button></div>
  </div>
</dialog>

<dialog bind:this={dlg} aria-labelledby="dt">
  <form class="dform" onsubmit={(e) => { e.preventDefault(); saveForm(); }}>
    <h2 id="dt">{form.id ? 'Edit item' : 'New item'}</h2>
    <p class="muted hint">Only the item name is needed. Each extra answer makes “days left”, the shopping list and the checks more true.</p>

    <fieldset>
      <legend>1. What is it?</legend>
      <label>Item<input class="input" bind:value={form.name} required placeholder="Bottled water" /></label>
      <label>Category
        <select class="input" bind:value={form.category}>{#each (allCats.includes(form.category) ? allCats : [...allCats, form.category]) as c}<option>{c}</option>{/each}</select>
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
        <input class="input" list="places" bind:value={form.location} placeholder="Pantry, or Garage > Shelf 2 > Black pail" />
        <datalist id="places">{#each locations.filter((l) => l !== 'All') as l}<option value={l}></option>{/each}</datalist>
        <span class="muted hint">To say where inside, put a &gt; between the parts: room, then shelf, then bin. Then you can open a shelf and see what is in it.</span>
      </label>
    </fieldset>

    <fieldset>
      <legend>4. Dates and condition</legend>
      {#if form.category === 'Food'}
        <div class="pack">
          <span class="muted">How is it packed? (changes how long it keeps)</span>
          <div class="row packrow">
            {#each packagingChoices as [code, text]}
              <label class="check"><input type="checkbox" checked={hasPack(code)} onchange={() => togglePack(code)} /> {text}</label>
            {/each}
          </div>
        </div>
      {/if}
      <div class="row">
        <label class="half">Bought or packed<input class="input" type="month" bind:value={form.bought} oninput={() => { guess = null; }} /></label>
        <label class="half">Use by or replace by<input class="input" type="month" bind:value={form.use_by} /></label>
      </div>
      {#if form.category === 'Food'}
        <div class="row">
          <button class="btn" type="button" onclick={suggestUseBy} title="Work out a use-by month from the item, how it is packed and when it was bought">Suggest a use-by date</button>
          {#if guessNote}<span class="muted small">{guessNote}</span>{/if}
        </div>
        {#if guess}
          <div class="ask">
            <strong>{guess.food}{guess.use_by ? ': use by ' + guess.use_by : ''}</strong>
            <div class="small">{guess.basis}{guess.assumed_today ? ' (No bought month was given, so this month was used.)' : ''}</div>
            {#if guess.hint}<div class="small muted">{guess.hint}</div>{/if}
            <div class="small muted">A guess to plan with. If the package has a date, use whichever comes first.</div>
            {#if guess.use_by}<button class="btn primary" type="button" onclick={useGuess}>Use {guess.use_by}</button>{/if}
          </div>
        {/if}
      {/if}
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
  .invtiles { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: 12px; }
  @media (max-width: 1200px) { .invtiles { grid-template-columns: repeat(3, 1fr); } }
  @media (max-width: 600px) { .invtiles { grid-template-columns: repeat(2, 1fr); } }
  .invtiles .mt-n { font-size: 32px; }
  .mt.health { flex-direction: row; }
  .hbar { height: 12px; border-radius: 7px; background: #1b2735; margin-top: 8px; overflow: hidden; }
  .hbar span { display: block; height: 100%; background: linear-gradient(90deg, #2bbf55, #43e06b); box-shadow: 0 0 10px #43e06b; }
  aside.dbl { padding: 14px; }
  .catman { list-style: none; margin: 8px 0; padding: 0; display: grid; gap: 6px; max-height: 46vh; overflow: auto; }
  .catman li { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 6px 8px; border: 1px solid var(--line); border-radius: 8px; }
  .catman li.hid { opacity: .6; }
  .catman .cn { display: inline-flex; align-items: center; gap: 8px; }
  .cat .cn { display: inline-flex; align-items: center; gap: 10px; }
  .cat { padding: 5px 10px; border-radius: 6px; color: #e6edf5; }
  .cat.on { background: linear-gradient(90deg, rgba(61, 155, 255, .25), transparent); border-color: #4db0ff; color: #fff; }
  .main.dbl { padding: 14px 16px; }
  .srch { display: flex; align-items: center; gap: 10px; padding: 0 14px; height: 44px; border: 1px solid #2a4a70; border-radius: 10px; background: #0a121b; color: #9fb4c9; }
  .srch input { flex: 1; background: transparent; border: 0; outline: 0; color: var(--text); font: inherit; }
  .selbar { background: linear-gradient(90deg, #2a0f0c, #1a0d0b); border: 1.5px solid #8a2a20; }
  .ci { display: inline-flex; vertical-align: middle; margin-right: 10px; color: var(--glow-blue); }
  .kebab { background: none; border: 0; color: #e6edf5; font-size: 20px; margin-left: 14px; padding: 0 6px; cursor: pointer; }
  .medicine { display: flex; align-items: center; gap: 10px; padding: 12px 14px; border: 1.5px solid #8a7420; border-radius: 10px; background: linear-gradient(90deg, #2a2408, #1a1706); color: #f3e9b8; }
  .medicine :global(svg) { color: var(--glow-amber); flex: none; }
  .end { justify-content: flex-end; }
  .head { justify-content: space-between; flex-wrap: nowrap; }
  .grow { flex: 1 1 200px; }
  .plus { width: 48px; height: 48px; border-radius: 50%; border: 0; background: var(--blue); color: #07131d; font-size: 30px; font-weight: 700; line-height: 1; }
  .layout { display: flex; flex-wrap: wrap; gap: 16px; align-items: flex-start; }
  aside { flex: 0 1 250px; min-width: 210px; display: flex; flex-direction: column; gap: 4px; }
  .cat { display: flex; justify-content: space-between; padding: 8px 10px; border-radius: 8px; border: 1px solid transparent; background: transparent; text-align: left; }
  .cat.on { background: var(--blue-soft); border-color: var(--blue-line); color: var(--blue); font-weight: 600; }
  .main { flex: 1 1 560px; min-width: 0; display: flex; flex-direction: column; gap: 10px; }
  .chips { gap: 6px; }
  .chip { padding: 6px 12px; border-radius: 999px; border: 1px solid var(--line2); background: var(--panel); font-size: 13px; color: var(--muted); }
  .chip.sub { border-style: dashed; }
  .where { padding: 8px 12px; border-radius: 10px; background: var(--panel2); border: 1px solid var(--line); display: flex; flex-direction: column; gap: 4px; }
  .wline { text-align: left; background: none; border: 0; padding: 2px 0; color: inherit; }
  .pack { display: flex; flex-direction: column; gap: 6px; font-size: 13px; }
  .packrow { gap: 6px 16px; }
  .check { display: flex !important; flex-direction: row !important; align-items: center; gap: 6px !important; font-size: 13px !important; color: inherit !important; }
  .check input { width: 18px; height: 18px; accent-color: var(--blue); }
  .chip.on { background: var(--blue-soft); border-color: var(--blue-line); color: var(--blue); font-weight: 600; }
  .bar { padding: 8px 10px; border-radius: 10px; background: var(--danger-bg); border: 1px solid var(--danger-line); }
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
  .waiting { margin: 0 0 1rem; padding: 0.8rem 1rem; }
  .wrow { display: flex; gap: 0.5rem; align-items: center; margin: 0.4rem 0; flex-wrap: wrap; }
  .plines { margin: 0.2rem 0 0.6rem 1rem; font-size: 0.9rem; }
  .okbar { padding: 0.6rem 1rem; margin: 0 0 1rem; border-radius: 10px; background: var(--panel, #2a2e33); }
</style>
