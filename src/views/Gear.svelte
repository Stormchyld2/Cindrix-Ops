<script lang="ts">
  // Gear: tents, radios, walking poles and anything else that is not food. Its own list, apart from the food Inventory.
  // Core: core/src/gear.rs (user/gear.txt). It starts empty: everything here is typed in by the person.
  import PageHeader from '../lib/PageHeader.svelte';
  import { onMount } from 'svelte';
  import { gearList, gearSave, gearDelete, gearSyncExport, gearSyncWaiting, gearSyncPreview, gearSyncAccept, type GearListing, type GearPiece, type GearSyncPlan } from '../lib/api';

  let data = $state<GearListing | null>(null);
  let note = $state('');
  let search = $state('');
  let kind = $state('All');
  let form = $state<GearPiece | null>(null);
  let removing = $state('');

  const blank = (): GearPiece => ({ id: '', name: '', kind: 'other', make: '', model: '', serial: '', location: '', qty: 1, condition: 'good', bought: '', notes: '' });
  async function load() { try { data = await gearList(); } catch (e) { note = String(e); } }
  onMount(() => { load(); loadWaiting(); });

  // Copy with the other device (laptop and phone): each sends a sheet, the other accepts it, and a piece changed on both is decided by a person.
  let waiting = $state<string[]>([]);
  let open = $state('');
  let plan = $state<GearSyncPlan | null>(null);
  let picks = $state<Record<string, boolean>>({});
  async function loadWaiting() { try { waiting = await gearSyncWaiting(); } catch { waiting = []; } }
  async function makeSheet() {
    try { note = `The sheet is saved as ${await gearSyncExport()}. Put it in the other device's inbox/sync folder (user/inbox/sync) and accept it there.`; } catch (e) { note = String(e); }
  }
  async function look(name: string) {
    try { plan = await gearSyncPreview(name); open = name; picks = {}; } catch (e) { note = String(e); }
  }
  const pieceText = (p: GearPiece | null) => (p ? `${p.name}${p.make || p.model ? ' (' + [p.make, p.model].filter(Boolean).join(' ') + ')' : ''}, ${p.qty} at ${p.location || 'no place'}, ${condName(p.condition)}${p.notes ? ', ' + p.notes : ''}` : 'Removed');
  async function accept() {
    try {
      const r = await gearSyncAccept(open, Object.entries(picks));
      note = `Took ${r.taken} new or changed, removed ${r.removed}. ${r.waiting ? r.waiting + ' still waiting for you to choose.' : 'Nothing is waiting.'}`;
      if (!r.waiting) { open = ''; plan = null; }
      else await look(open);
      await load(); await loadWaiting();
    } catch (e) { note = String(e); }
  }

  const kindName = (id: string) => data?.kinds.find((k) => k[0] === id)?.[1] ?? id;
  const condName = (id: string) => data?.conditions.find((k) => k[0] === id)?.[1] ?? id;
  const shown = $derived.by(() => {
    const words = search.toLowerCase().split(/\s+/).filter(Boolean);
    return (data?.items ?? []).filter((p) => (kind === 'All' || p.kind === kind) && words.every((w) => `${p.name} ${kindName(p.kind)} ${p.make} ${p.model} ${p.serial} ${p.location} ${p.notes}`.toLowerCase().includes(w)));
  });

  async function save() {
    if (!form) return;
    try { data = await gearSave({ ...form, qty: Number(form.qty) || 0 }); form = null; note = 'Saved.'; } catch (e) { note = String(e); }
  }
  async function remove(id: string) {
    try { data = await gearDelete([id]); removing = ''; note = 'Removed.'; } catch (e) { note = String(e); }
  }
</script>

<PageHeader eyebrow="CINDRIX OPS" title="Gear" tagline="Tents, radios, walking poles and everything else that is not food, with make, model and serial number. Food stays in the Inventory." />

<div class="panel">
  <h2>Copy with the other device</h2>
  <p class="muted small">Make a sheet here and put it on the other device, and the other way round. Nothing is joined without you: a piece changed on both devices is shown to you to choose.</p>
  <button class="btn" onclick={makeSheet} title="Save this device's gear list as a sheet for the other device">Make a sheet for the other device</button>
  {#each waiting as w (w)}
    <div class="row"><span class="grow">{w}</span><button class="btn" onclick={() => look(w)} title="See what accepting this sheet would change">Look at it</button></div>
  {/each}
  {#if plan && open}
    <p class="small">From device {plan.from}: {plan.take.length} new or changed, {plan.remove.length} removed there, {plan.same} the same, {plan.conflicts.length} to decide.</p>
    {#each plan.conflicts as c (c.id)}
      <div class="small">
        <b>{c.id}</b><br />
        <label><input type="radio" name={'c' + c.id} onchange={() => (picks[c.id] = false)} /> Keep mine: {pieceText(c.mine)}</label><br />
        <label><input type="radio" name={'c' + c.id} onchange={() => (picks[c.id] = true)} /> Use theirs: {pieceText(c.theirs)}</label>
      </div>
    {/each}
    <button class="btn primary" onclick={accept} title="Accept the sheet. Pieces you have not chosen for stay as they are">Accept this sheet</button>
  {/if}
</div>

<div class="panel">
  {#if note}<p class="note" role="status">{note}</p>{/if}
  {#if data && data.unreadable_lines > 0}<p class="note">Some lines in the gear file could not be read. They were left exactly as they are, and nothing can be saved until they are looked at.</p>{/if}
  <div class="row ch">
    <input class="input grow" placeholder="Search by name, make, model, serial number or place" bind:value={search} aria-label="Search gear" />
    <select class="input" bind:value={kind} aria-label="Kind"><option>All</option>{#each data?.kinds ?? [] as k}<option value={k[0]}>{k[1]}</option>{/each}</select>
    <button class="btn primary" onclick={() => (form = blank())} title="Add a piece of gear">Add gear</button>
  </div>

  {#if form}
    <div class="panel inner">
      <h3>{form.id ? 'Change this piece' : 'Add a piece of gear'}</h3>
      <div class="row ch">
        <label class="small grow">Name<input class="input" placeholder="Tent, radio, walking pole…" bind:value={form.name} /></label>
        <label class="small">Kind<select class="input" bind:value={form.kind}>{#each data?.kinds ?? [] as k}<option value={k[0]}>{k[1]}</option>{/each}</select></label>
        <label class="small">How many<input class="input short" type="number" min="1" max="9999" bind:value={form.qty} /></label>
      </div>
      <div class="row ch">
        <label class="small grow">Make<input class="input" bind:value={form.make} /></label>
        <label class="small grow">Model<input class="input" bind:value={form.model} /></label>
        <label class="small grow">Serial number<input class="input" bind:value={form.serial} /></label>
      </div>
      <div class="row ch">
        <label class="small grow">Where it is kept<input class="input" placeholder="Garage > Shelf 2" bind:value={form.location} /></label>
        <label class="small">Condition<select class="input" bind:value={form.condition}>{#each data?.conditions ?? [] as c}<option value={c[0]}>{c[1]}</option>{/each}</select></label>
        <label class="small">Date bought<input class="input" type="date" bind:value={form.bought} /></label>
      </div>
      <label class="small">Notes<textarea class="input" rows="2" bind:value={form.notes}></textarea></label>
      <div class="row ch"><button class="btn primary" onclick={save}>Save</button><button class="btn" onclick={() => (form = null)}>Cancel</button></div>
    </div>
  {/if}

  {#if shown.length}
    <table class="grid"><thead><tr><th>Name</th><th>Make and model</th><th>Serial number</th><th>Kept at</th><th>Count</th><th>Condition</th><th></th></tr></thead><tbody>
      {#each shown as p (p.id)}
        <tr class:weak={p.condition === 'repair' || p.condition === 'out'}>
          <td><b>{p.name}</b><div class="small muted">{kindName(p.kind)}{p.bought ? `, bought ${p.bought}` : ''}</div>{#if p.notes}<div class="small">{p.notes}</div>{/if}</td>
          <td>{[p.make, p.model].filter(Boolean).join(' ') || '—'}</td>
          <td>{p.serial || '—'}</td>
          <td>{p.location || '—'}</td>
          <td>{p.qty}</td>
          <td>{condName(p.condition)}</td>
          <td class="acts">
            <button class="btn" onclick={() => (form = { ...p })}>Change</button>
            {#if removing === p.id}<button class="btn" onclick={() => remove(p.id)}>Yes, remove it</button><button class="btn" onclick={() => (removing = '')}>Keep it</button>
            {:else}<button class="btn" onclick={() => (removing = p.id)}>Remove</button>{/if}
          </td>
        </tr>
      {/each}
    </tbody></table>
  {:else if data && data.items.length}<p class="muted">Nothing matches that.</p>
  {:else}<p class="muted">Nothing here yet. Press "Add gear" to enter your first piece. Nothing is filled in for you.</p>{/if}
</div>
