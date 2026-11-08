<script lang="ts">
  // Security Operations > Operations: the watchbill (docs/44 section 9). A draft covers nothing until a person
  // schedules it; the person on watch acknowledges it; a watch with no handover stays open until someone looks.
  import { onMount } from 'svelte';
  import {
    securityOverview, securityWatchList, securityWatchDraft, securityWatchAdd, securityWatchMove,
    securityWatchAck, securityWatchDiscard, securityWatchScheduleAll,
    type SecurityOverview, type WatchList, type WatchRow,
  } from '../lib/api';

  let { site = $bindable(''), me = '' }: { site: string; me: string } = $props();
  let ov: SecurityOverview | null = $state(null);
  let list: WatchList | null = $state(null);
  let note = $state('');

  async function load() {
    try {
      ov = await securityOverview();
      if (!ov.sites.some((s) => s.id === site)) site = ov.sites[0]?.id ?? '';
      list = site ? await securityWatchList(site) : null;
    } catch (e) { note = String(e); }
  }
  onMount(load);
  async function pickSite(id: string) { site = id; list = id ? await securityWatchList(id) : null; }
  async function act(fn: () => Promise<unknown>) { try { await fn(); note = ''; await load(); } catch (e) { note = String(e); } }

  const watches = $derived(list?.watches ?? []);
  const drafts = $derived(watches.filter((w) => w.state === 'proposed'));
  const siteName = $derived(ov?.sites.find((s) => s.id === site)?.name ?? '');
  const when = (iso: string) => { const d = new Date(iso); return isNaN(+d) ? iso : d.toLocaleString([], { weekday: 'short', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }); };
  const hrs = (w: WatchRow) => Math.round((Date.parse(w.end) - Date.parse(w.start)) / 360000) / 10;
  const STATE: Record<string, string> = { proposed: 'Draft', scheduled: 'Scheduled', acknowledged: 'Acknowledged', 'on-duty': 'On duty', completed: 'Done', missed: 'Missed' };

  // a handover that really happened earlier (radio, paper)
  let ackFor: string | null = $state(null);
  let ackAt = $state('');
  async function ack(w: WatchRow) { await act(() => securityWatchAck(w.id, ackAt)); ackFor = null; ackAt = ''; }

  // drafting
  let dPeople = $state('');
  let dStart = $state('');
  let dHours = $state(6);
  let dCount = $state(4);
  const draft = () => act(async () => {
    const n = await securityWatchDraft(site, dPeople.split(/[,\n]/), dStart, dHours, dCount);
    note = `${n} draft watches made. They cover nothing until you schedule them.`;
  });

  // adding one by hand
  let aPerson = $state('');
  let aRole = $state('watch');
  let aStart = $state('');
  let aEnd = $state('');
  const add = () => act(async () => { await securityWatchAdd(site, aPerson, aRole, aStart, aEnd); aPerson = ''; });
</script>

{#if note}<div class="warn no-print" role="status">{note} <button class="btn" onclick={() => (note = '')}>OK</button></div>{/if}

{#if ov && ov.sites.length === 0}
  <div class="panel"><h2>No site yet</h2><p class="muted">Add a site on the Command tab first. Each site has its own watchbill.</p></div>
{:else if ov}
  <div class="row top no-print">
    <label class="label" for="osite">Site</label>
    <select id="osite" class="input" value={site} onchange={(e) => pickSite(e.currentTarget.value)}>
      {#each ov.sites as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
    </select>
    <span class="grow"></span>
    <button class="btn" onclick={() => window.print()} title="Print the watchbill, so it works without the program or any power">Print watchbill</button>
  </div>

  {#if !me.trim()}<p class="warn no-print">Type your name at the top first. Every change is written down with it.</p>{/if}

  {#if list && list.warnings.length}
    <div class="panel warns no-print"><h2>Check these</h2><ul class="plain">{#each list.warnings as w}<li>{w}</li>{/each}</ul></div>
  {/if}

  <div class="panel">
    <div class="row"><h2>Watchbill: {siteName}</h2><span class="grow"></span>
      {#if drafts.length}<button class="btn primary no-print" onclick={() => act(() => securityWatchScheduleAll(site))}>Schedule all {drafts.length} drafts</button>{/if}
    </div>
    {#if watches.length === 0}
      <p class="muted">No watches written down for this site. Draft a watchbill below, or add one by hand.</p>
    {:else}
      <table class="wb">
        <thead><tr><th>When</th><th>Hours</th><th>Who</th><th>Role</th><th>Status</th><th class="no-print"></th></tr></thead>
        <tbody>
          {#each watches as w (w.id)}
            <tr class={w.state}>
              <td>{when(w.start)} to {when(w.end)}</td>
              <td>{hrs(w)}</td>
              <td><b>{w.person}</b></td>
              <td>{w.role}</td>
              <td><span class="badge s {w.state}">{STATE[w.state]}</span>{#if w.ack_at} <span class="muted small">{when(w.ack_at)}{w.entered_at && w.entered_at !== w.ack_at ? ' (written ' + when(w.entered_at) + ')' : ''}</span>{/if}</td>
              <td class="acts no-print">
                {#if w.state === 'proposed'}
                  <button class="btn" onclick={() => act(() => securityWatchMove(w.id, 'scheduled'))}>Schedule</button>
                  <button class="btn" onclick={() => act(() => securityWatchDiscard(w.id))}>Throw away</button>
                {:else if w.state === 'scheduled'}
                  <button class="btn" onclick={() => (ackFor = ackFor === w.id ? null : w.id)}>Acknowledge</button>
                  <button class="btn" onclick={() => act(() => securityWatchMove(w.id, 'missed'))}>Missed</button>
                {:else if w.state === 'acknowledged'}
                  <button class="btn" onclick={() => act(() => securityWatchMove(w.id, 'on-duty'))}>On duty</button>
                  <button class="btn" onclick={() => act(() => securityWatchMove(w.id, 'missed'))}>Missed</button>
                {:else if w.state === 'on-duty'}
                  <button class="btn" onclick={() => act(() => securityWatchMove(w.id, 'completed'))} title="The watch is over and handed to the next person">Handed over</button>
                  <button class="btn" onclick={() => act(() => securityWatchMove(w.id, 'missed'))}>Missed</button>
                {:else if w.state === 'missed'}
                  <button class="btn" onclick={() => act(() => securityWatchMove(w.id, 'scheduled'))}>Schedule again</button>
                {/if}
              </td>
            </tr>
            {#if ackFor === w.id}
              <tr class="no-print"><td colspan="6">
                <div class="row">
                  <button class="btn primary" onclick={() => ack(w)}>{w.person} acknowledges now</button>
                  <span class="muted">or it happened earlier, by radio or on paper:</span>
                  <input class="input" type="datetime-local" bind:value={ackAt} aria-label="When it really happened" />
                  <button class="btn" disabled={!ackAt} onclick={() => ack(w)}>Write it in at that time</button>
                </div>
              </td></tr>
            {/if}
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <div class="panel no-print">
    <h2>Draft a watchbill</h2>
    <p class="muted">Watches run one after another and are handed round the people in order. A draft is a suggestion: it covers nothing until you schedule it.</p>
    <div class="row">
      <textarea class="input grow" placeholder="Names, one per line or separated by commas" bind:value={dPeople} rows="2" aria-label="Names"></textarea>
    </div>
    <div class="row">
      <label class="label" for="ds">First watch starts</label><input id="ds" class="input" type="datetime-local" bind:value={dStart} />
      <label class="label" for="dh">Hours each</label><input id="dh" class="input num" type="number" min="1" max="24" bind:value={dHours} />
      <label class="label" for="dc">How many watches</label><input id="dc" class="input num" type="number" min="1" max="200" bind:value={dCount} />
      <button class="btn" disabled={!me.trim() || !dStart || !dPeople.trim()} onclick={draft}>Make draft</button>
    </div>
  </div>

  <div class="panel no-print">
    <h2>Add one watch</h2>
    <div class="row">
      <input class="input" placeholder="Who" bind:value={aPerson} aria-label="Who" />
      <input class="input" placeholder="Role" bind:value={aRole} aria-label="Role" />
      <input class="input" type="datetime-local" bind:value={aStart} aria-label="Starts" />
      <input class="input" type="datetime-local" bind:value={aEnd} aria-label="Ends" />
      <button class="btn" disabled={!me.trim() || !aPerson.trim() || !aStart || !aEnd} onclick={add}>Add</button>
    </div>
    <p class="muted small">Adding a watch yourself counts as scheduling it.</p>
  </div>
{/if}

<style>
  .panel { margin-bottom: 14px; }
  .panel h2 { font-size: 15px; margin-bottom: 8px; }
  .top { margin-bottom: 14px; }
  .grow { flex: 1; }
  .small { font-size: 12px; }
  .num { width: 80px; }
  ul.plain { margin: 6px 0 0; padding-left: 18px; }
  .warns li { color: var(--warn); }
  table.wb { width: 100%; border-collapse: collapse; }
  .wb th { text-align: left; font-family: var(--mono); font-size: 11px; letter-spacing: .12em; text-transform: uppercase; color: var(--muted); padding: 6px 8px; border-bottom: 1px solid var(--line2); }
  .wb td { padding: 8px; border-bottom: 1px solid var(--line); vertical-align: top; }
  .wb tr.proposed td { opacity: .75; }
  .wb tr.missed td { color: var(--danger-text); }
  .acts { white-space: nowrap; text-align: right; }
  .acts .btn { padding: 5px 10px; }
  .badge.s.acknowledged, .badge.s.on-duty, .badge.s.completed { color: var(--ok-text); border-color: var(--ok-line); background: var(--ok-bg); }
  .badge.s.scheduled { color: var(--warn); }
  .badge.s.proposed { color: var(--muted); border-color: var(--line2); background: transparent; }
  .badge.s.missed { color: var(--danger-text); border-color: var(--danger-line); background: var(--danger-bg); }
  @media print {
    .wb td, .wb th { border-color: #999; color: #111 !important; }
    .badge.s { background: none !important; border-color: #666 !important; color: #111 !important; }
  }
</style>
