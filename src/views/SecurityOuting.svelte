<script lang="ts">
  // Security Operations > Operations: who is out of camp and who is on patrol (docs/46, core/src/security_outing.rs).
  // Lateness only adds a line to "needs a look". It never opens an incident, and only a person closes an outing.
  import { onMount } from 'svelte';
  import { securityOutingList, securityOutingStart, securityOutingEvent, securityRoutesGet, type RouteRow, type OutingList, type OutingRow } from '../lib/api';

  let { site = '', me = '' }: { site: string; me: string } = $props();
  let data: OutingList | null = $state(null);
  let note = $state('');

  let routes: RouteRow[] = $state([]);
  // Suggest the checkpoints of the route a patrol was started on; if its name matches none, all of the site's.
  const cpChoices = (o: OutingRow) => routes.find((r) => r.name === o.task)?.checkpoints ?? routes.flatMap((r) => r.checkpoints);
  async function load() { try { data = site ? await securityOutingList(site) : null; routes = site ? (await securityRoutesGet(site)).routes : []; } catch (e) { note = String(e); } }
  onMount(load);
  $effect(() => { site; load(); });

  const pad = (n: number) => String(n).padStart(2, '0');
  const stamp = (plusMin: number) => { const d = new Date(Date.now() + plusMin * 60000); return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`; };
  const clock = (iso: string) => { const d = new Date(iso); return isNaN(+d) ? iso : d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' }); };
  const day = (iso: string) => { const d = new Date(iso); return isNaN(+d) ? iso : d.toLocaleString([], { weekday: 'short', hour: 'numeric', minute: '2-digit' }); };
  const names = (o: OutingRow) => o.people.join(' and ');

  const out = $derived((data?.outings ?? []).filter((o) => o.state === 'out'));
  const done = $derived((data?.outings ?? []).filter((o) => o.state !== 'out'));

  // ---- going out ----
  let kind: 'outing' | 'patrol' = $state('outing');
  let people = $state('');
  let task = $state('');
  let back = $state(stamp(120));
  let next = $state('');
  let method = $state('radio');
  async function go() {
    try {
      await securityOutingStart(site, kind, people, task, back, next, method, me || 'Base');
      people = ''; task = ''; next = ''; back = stamp(120); note = '';
      await load();
    } catch (e) { note = String(e); }
  }

  // ---- what happens to someone who is out ----
  let acting: { id: string; what: 'contact' | 'checkpoint' | 'extend' | 'cancel' } | null = $state(null);
  let status = $state('ok');
  let cpName = $state('');
  let text = $state('');
  let nextAt = $state('');
  let newBack = $state('');
  function open(o: OutingRow, what: 'contact' | 'checkpoint' | 'extend' | 'cancel') {
    acting = { id: o.id, what }; status = 'ok'; cpName = ''; text = ''; nextAt = ''; newBack = '';
  }
  async function send(o: OutingRow) {
    if (!acting) return;
    try {
      const w = acting.what;
      if (w === 'contact') await securityOutingEvent(o.id, 'contact', status, '', text, '', nextAt, me || 'Base');
      else if (w === 'checkpoint') await securityOutingEvent(o.id, 'checkpoint', cpName, status, text, '', nextAt, me || 'Base');
      else if (w === 'extend') await securityOutingEvent(o.id, 'extend', '', '', text, newBack, nextAt, me || 'Base');
      else await securityOutingEvent(o.id, 'cancel', '', '', text, '', '', me || 'Base');
      acting = null; note = '';
      await load();
    } catch (e) { note = String(e); }
  }
  async function backSafe(o: OutingRow) {
    try { await securityOutingEvent(o.id, 'return', '', '', '', '', '', me || 'Base'); note = ''; await load(); } catch (e) { note = String(e); }
  }
</script>

<div class="panel">
  <h2>Out of camp</h2>
  <p class="muted">Write down who goes out and when they expect to be back. A late check-in only adds a line to "Needs a look". It does not start an incident. Only a person marks them back safe.</p>
  {#if note}<p class="note" role="alert">{note}</p>{/if}
  {#if data && data.unreadable_lines > 0}<p class="note">Some lines in the outing log could not be read. They were left exactly as they are.</p>{/if}

  {#if !site}
    <p class="muted">Add a site on the Command tab first.</p>
  {:else}
    {#each out as o (o.id)}
      <div class="out" class:late={o.looks.length > 0}>
        <div class="grow">
          <b>{names(o)}</b> <span class="badge">{o.patrol ? 'Patrol' : 'Outing'}</span>
          <div>{o.task}</div>
          <div class="small muted">Left {clock(o.left)} · back by {day(o.back)}{o.next ? ` · next contact ${clock(o.next)}` : ''}{o.method ? ` · by ${o.method}` : ''}</div>
          {#if o.last}<div class="small">Last heard {clock(o.last.at)}: {o.last.status}{o.last.text ? `, ${o.last.text}` : ''}</div>{/if}
          {#if o.checkpoints.length}<div class="small muted">Checkpoints: {o.checkpoints.map((c) => `${c.name} ${clock(c.at)}`).join(' → ')}</div>{/if}
          {#each o.looks as l}<div class="look small">{l}</div>{/each}
        </div>
        <div class="acts">
          <button class="btn" onclick={() => open(o, 'contact')} title="Write down a radio call or a runner's report">Check-in</button>
          {#if o.patrol}<button class="btn" onclick={() => open(o, 'checkpoint')} title="They reached a checkpoint">Checkpoint</button>{/if}
          <button class="btn" onclick={() => open(o, 'extend')} title="Change the return time or next contact, with a reason">Change time</button>
          <button class="btn primary" onclick={() => backSafe(o)} title="They are back and everyone is accounted for">Back safe</button>
          <button class="btn" onclick={() => open(o, 'cancel')} title="They never went, or the outing was called off">Called off</button>
        </div>
        {#if acting && acting.id === o.id}
          <div class="form">
            {#if acting.what === 'contact' || acting.what === 'checkpoint'}
              {#if acting.what === 'checkpoint'}<label>Checkpoint <input bind:value={cpName} placeholder="North gate" list="cps" /></label>
              <datalist id="cps">{#each cpChoices(o) as c}<option value={c}></option>{/each}</datalist>{/if}
              <label>How are they <select bind:value={status}><option value="ok">All well</option><option value="delayed">Delayed</option><option value="help">Need help</option></select></label>
              <label>Note <input bind:value={text} placeholder="What they said" /></label>
              <label>Next contact (leave empty if none) <input type="datetime-local" bind:value={nextAt} /></label>
            {:else if acting.what === 'extend'}
              <label>Why <input bind:value={text} placeholder="Road washed out, going around" /></label>
              <label>New return time <input type="datetime-local" bind:value={newBack} /></label>
              <label>New next contact <input type="datetime-local" bind:value={nextAt} /></label>
            {:else}
              <label>Why <input bind:value={text} /></label>
            {/if}
            <div class="acts"><button class="btn primary" onclick={() => send(o)}>Save</button> <button class="btn" onclick={() => (acting = null)}>Cancel</button></div>
          </div>
        {/if}
      </div>
    {:else}
      <p class="muted">Nobody is out of camp.</p>
    {/each}

    <h3>Someone is going out</h3>
    <div class="form row">
      <label>What <select bind:value={kind}><option value="outing">Outing (a task)</option><option value="patrol">Patrol (a route with checkpoints)</option></select></label>
      <label>Who is going (names, with commas) <input bind:value={people} placeholder="Sam, Dana" /></label>
      <label>Task or route <input bind:value={task} placeholder="Water run to the creek" list="routes" /></label>
      <datalist id="routes">{#each routes as r}<option value={r.name}></option>{/each}</datalist>
      <label>Expected back <input type="datetime-local" bind:value={back} /></label>
      <label>First check-in (optional) <input type="datetime-local" bind:value={next} /></label>
      <label>Staying in touch by <select bind:value={method}><option value="radio">Radio</option><option value="runner">Runner</option><option value="none">Not in touch</option></select></label>
      <button class="btn primary" onclick={go} disabled={!people.trim() || !task.trim()}>They have left</button>
    </div>
    <p class="muted small">The group goes out in pairs. Going out alone shows up under "Needs a look".</p>

    {#if data?.last_patrol}<p class="small muted">Last patrol came back {day(data.last_patrol)}.</p>{/if}
    {#if done.length}
      <details><summary>Back or called off in the last day ({done.length})</summary>
        {#each done as o (o.id)}<div class="small">{names(o)} · {o.task} · {o.state === 'back' ? 'back safe' : 'called off'} {clock(o.ended)}</div>{/each}
      </details>
    {/if}
  {/if}
</div>

<style>
  .out { display: flex; gap: 10px; flex-wrap: wrap; align-items: flex-start; padding: 10px 0; border-top: 1px solid var(--line); }
  .out.late { border-left: 3px solid var(--danger, #c0392b); padding-left: 8px; }
  .grow { flex: 1; min-width: 240px; }
  .look { color: var(--danger-text, var(--danger)); }
  .acts { display: flex; gap: 6px; flex-wrap: wrap; }
  .form { display: flex; flex-direction: column; gap: 8px; width: 100%; margin-top: 6px; }
  .form.row { flex-direction: row; flex-wrap: wrap; align-items: flex-end; }
  .form label { display: flex; flex-direction: column; gap: 2px; font-size: 12px; }
  .small { font-size: 12px; }
  .note { padding: 8px 10px; border-radius: 8px; background: var(--danger-bg, var(--panel)); }
</style>
