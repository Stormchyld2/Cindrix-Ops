<script lang="ts">
  // Security Operations > Security: what protects each site (zones, barriers, alarms), when each was last inspected,
  // and the patrol routes (core/src/security_site.rs). Inspections are lines added to a log, never edits.
  import { onMount } from 'svelte';
  import {
    securityOverview, securityAssetsList, securityAssetSave, securityAssetInspect, securityRoutesGet, securityRoutesSave, securityScheduleGet, securityScheduleSave,
    type AssetList, type AssetRow, type RouteRow, type RuleRow,
  } from '../lib/api';

  let { site = $bindable(''), me = '' }: { site: string; me: string } = $props();
  let sites: { id: string; name: string }[] = $state([]);
  let note = $state('');
  let list = $state<AssetList | null>(null);
  let routes: RouteRow[] = $state([]);
  let rules: RuleRow[] = $state([]);
  const who = $derived(me.trim());

  async function load() {
    try {
      const ov = await securityOverview();
      sites = ov.sites.map((s) => ({ id: s.id, name: s.name }));
      if (!sites.some((s) => s.id === site)) site = sites[0]?.id ?? '';
      if (site) { list = await securityAssetsList(site); routes = (await securityRoutesGet(site)).routes; rules = (await securityScheduleGet(site)).rules; }
    } catch (e) { note = String(e); }
  }
  onMount(load);
  $effect(() => { site; load(); });
  async function act(fn: () => Promise<unknown>, ok = '') { try { await fn(); note = ok; await load(); } catch (e) { note = String(e); } }

  const when = (iso: string) => { const d = new Date(iso); return isNaN(+d) ? iso : d.toLocaleString([], { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }); };
  const DUE: Record<string, string> = { ok: 'Up to date', overdue: 'Overdue', never: 'Never inspected', 'needs-work': 'Needs work', none: 'No regular check' };
  const kindLabel = (id: string) => list?.kinds.find((k) => k.id === id)?.label ?? id;

  // add or change a thing
  let editing = $state<string | null>(null); // asset id, or '' for a new one
  let fKind = $state('barrier'); let fName = $state(''); let fDays = $state(7); let fNotes = $state('');
  function edit(a: AssetRow | null) { editing = a?.id ?? ''; fKind = a?.kind ?? 'barrier'; fName = a?.name ?? ''; fDays = a?.every_days ?? 7; fNotes = a?.notes ?? ''; }
  const saveAsset = () => act(async () => { await securityAssetSave(site, editing ?? '', fKind, fName, Number(fDays) || 0, fNotes); editing = null; }, 'Saved.');

  // inspect
  let looking = $state<string | null>(null); let lResult: 'ok' | 'needs-work' = $state('ok'); let lNote = $state('');
  const inspect = (a: AssetRow) => act(async () => { await securityAssetInspect(a.id, lResult, who, lNote); looking = null; lNote = ''; }, 'Written in the inspection log.');

  // routes: one line per checkpoint, in the order they are walked
  function addRoute() { routes = [...routes, { name: '', checkpoints: [] }]; }
  function dropRoute(i: number) { routes = routes.filter((_, k) => k !== i); }
  const cpText = (r: RouteRow) => r.checkpoints.join('\n');
  function setCp(i: number, v: string) { routes[i].checkpoints = v.split('\n').map((s) => s.trim()).filter(Boolean); }
  const saveRoutes = () => act(() => securityRoutesSave(site, routes), 'The routes are saved. A patrol can use them as its checkpoints.');
  function addRule() { rules = [...rules, { route: '', every_hours: 4, from: '20:00', to: '06:00' }]; }
  function dropRule(i: number) { rules = rules.filter((_, k) => k !== i); }
  const saveRules = () => act(() => securityScheduleSave(site, rules.map((r) => ({ ...r, every_hours: Number(r.every_hours) || 0 }))), 'Saved. When a patrol is overdue inside its hours, Command shows it under "Needs a look".');
</script>

{#if note}<p class="note" role="status">{note}</p>{/if}
<div class="row pick">
  <label class="label" for="st">Site</label>
  <select id="st" class="input" bind:value={site}>{#each sites as s}<option value={s.id}>{s.name}</option>{/each}</select>
</div>

{#if !site}
  <div class="panel"><p class="muted">Add a site on the Command tab first.</p></div>
{:else}
  <div class="panel">
    <h2>Zones, barriers and alarms</h2>
    <p class="muted">List what protects this site and how often each should be looked at. Quiet alarms that need no power, like trip lines and noise makers, come before cameras. Anything overdue or needing work shows under "Needs a look" on Command.</p>
    {#if list && list.unreadable_lines > 0}<p class="note">Some lines could not be read. They were left exactly as they are, and nothing new is added to a damaged log until they are looked at.</p>{/if}
    {#if list && list.assets.length}
      <div class="scroll"><table class="grid">
        <thead><tr><th>Name</th><th>Kind</th><th>Every</th><th>Last looked at</th><th>State</th><th></th></tr></thead>
        <tbody>
          {#each list.assets as a (a.id)}
            <tr class:weak={a.due === 'overdue' || a.due === 'needs-work'}>
              <td><b>{a.name}</b>{#if a.notes}<div class="small muted">{a.notes}</div>{/if}</td>
              <td>{kindLabel(a.kind)}</td>
              <td>{a.every_days ? `${a.every_days} day${a.every_days === 1 ? '' : 's'}` : '–'}</td>
              <td>{a.last_at ? `${when(a.last_at)} by ${a.last_by}` : 'Never'}{#if a.last_result === 'needs-work'}<div class="small">{a.last_note}</div>{/if}</td>
              <td>{DUE[a.due]}</td>
              <td class="acts"><button class="btn" onclick={() => { looking = a.id; lResult = 'ok'; lNote = ''; }}>Inspect</button> <button class="btn" onclick={() => edit(a)}>Change</button></td>
            </tr>
            {#if looking === a.id}
              <tr><td colspan="6">
                <div class="row ch">
                  <select class="input" bind:value={lResult} aria-label="Result"><option value="ok">All fine</option><option value="needs-work">Needs work</option></select>
                  <input class="input grow" placeholder={lResult === 'ok' ? 'Anything to add (optional)' : 'What needs work'} bind:value={lNote} aria-label="Note" />
                  <button class="btn primary" disabled={!who || (lResult === 'needs-work' && !lNote.trim())} onclick={() => inspect(a)} title={who ? '' : 'Write your name at the top first'}>Write it down</button>
                  <button class="btn" onclick={() => (looking = null)}>Cancel</button>
                </div>
              </td></tr>
            {/if}
          {/each}
        </tbody>
      </table></div>
    {:else}<p class="muted">Nothing listed for this site yet.</p>{/if}

    {#if editing === null}
      <div class="acts"><button class="btn primary" onclick={() => edit(null)}>Add something</button></div>
    {:else}
      <div class="row ch">
        <select class="input" bind:value={fKind} aria-label="Kind">{#each list?.kinds ?? [] as k}<option value={k.id}>{k.label}</option>{/each}</select>
        <input class="input grow" placeholder="Name (North gate, Trip line by the creek)" bind:value={fName} aria-label="Name" />
        <label class="small">Look at it every <input class="input short" type="number" min="0" max="3650" bind:value={fDays} /> days (0 = no regular check)</label>
        <input class="input grow" placeholder="Notes" bind:value={fNotes} aria-label="Notes" />
        <button class="btn primary" disabled={!fName.trim()} onclick={saveAsset}>Save</button>
        <button class="btn" onclick={() => (editing = null)}>Cancel</button>
      </div>
    {/if}
  </div>

  <div class="panel">
    <h2>Patrol routes</h2>
    <p class="muted">Name each route and list its checkpoints in the order they are walked, one per line. The Operations tab uses them when a patrol starts.</p>
    {#each routes as r, i}
      <div class="row ch">
        <input class="input" placeholder="Route name" bind:value={r.name} aria-label="Route name" />
        <textarea class="input grow" rows="3" placeholder="One checkpoint per line" value={cpText(r)} oninput={(e) => setCp(i, (e.currentTarget as HTMLTextAreaElement).value)} aria-label="Checkpoints"></textarea>
        <button class="btn" onclick={() => dropRoute(i)}>Remove</button>
      </div>
    {/each}
    <div class="acts"><button class="btn" onclick={addRoute}>Add a route</button> <button class="btn primary" onclick={saveRoutes}>Save the routes</button></div>
  </div>

  <div class="panel">
    <h2>Patrol schedule</h2>
    <p class="muted">Say how often this site should be patrolled and during which hours. This is only a reminder: if no patrol has gone when one is due, Command shows it under "Needs a look". Nothing is opened or changed for you. Leave the route empty to count any patrol.</p>
    {#each rules as r, i}
      <div class="row ch">
        <input class="input" list="rts" placeholder="Route (any)" bind:value={r.route} aria-label="Route" />
        <label class="small">Every <input class="input short" type="number" min="1" max="72" bind:value={r.every_hours} /> hours</label>
        <label class="small">from <input class="input" type="time" bind:value={r.from} /></label>
        <label class="small">to <input class="input" type="time" bind:value={r.to} /></label>
        <button class="btn" onclick={() => dropRule(i)}>Remove</button>
      </div>
    {/each}
    <datalist id="rts">{#each routes as r}<option value={r.name}></option>{/each}</datalist>
    <div class="acts"><button class="btn" onclick={addRule}>Add a patrol time</button> <button class="btn primary" onclick={saveRules}>Save the schedule</button></div>
  </div>
{/if}

<style>
  .pick { margin-bottom: 10px; align-items: center; gap: 8px; }
  .ch { gap: 8px; flex-wrap: wrap; align-items: center; margin: 8px 0; }
  .short { width: 80px; }
  .grow { flex: 1; min-width: 160px; }
  .acts { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 8px; }
  td.acts { margin-top: 0; }
  .grid { width: 100%; border-collapse: collapse; font-size: 13px; margin-top: 8px; }
  .grid th, .grid td { text-align: left; padding: 5px 8px; border-top: 1px solid var(--line); vertical-align: top; }
  .grid tr.weak td { background: var(--danger-bg, transparent); }
  .scroll { overflow-x: auto; }
  .small { font-size: 12px; }
  .note { padding: 8px 10px; border-radius: 8px; background: var(--panel); }
</style>
