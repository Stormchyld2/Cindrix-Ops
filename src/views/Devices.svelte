<script lang="ts">
  // Settings > Devices (core/src/devices.rs, docs/45). Phones and browsers the Hub trusts, by the device's own code.
  // "Erase" is an order that waits until the device reaches the Hub. Host only.
  import { onMount } from 'svelte';
  import { devicesList, devicesAction, type DeviceRow } from '../lib/api';

  let list: DeviceRow[] = $state([]);
  let note = $state('');
  let asking = $state(''); // the device whose erase is waiting for a second click

  async function load() { try { list = await devicesList(); } catch (e) { note = String(e); } }
  onMount(load);

  async function act(id: string, action: 'erase' | 'cancel' | 'lock') {
    try {
      list = await devicesAction(id, action);
      asking = '';
      note = action === 'erase' ? 'Erase ordered. It happens the next time this device reaches the Hub. Until then it can do nothing else here.'
        : action === 'cancel' ? 'The erase order was taken back.' : 'This device is locked out.';
    } catch (e) { note = String(e); }
  }

  const day = (t: number) => (t ? new Date(t * 1000).toLocaleString([], { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }) : 'never');
  const KIND: Record<string, string> = { pocket: 'Cindrix Pocket', browser: 'Browser' };
  const STATE: Record<string, string> = { trusted: 'Trusted', 'erase-ordered': 'Erase waiting for the device', erased: 'Erased and locked out', revoked: 'Locked out' };
  const live = $derived(list.filter((d) => d.state === 'trusted' || d.state === 'erase-ordered'));
  const gone = $derived(list.filter((d) => d.state === 'erased' || d.state === 'revoked'));
</script>

<div class="panel">
  <h2>Connected devices</h2>
  <p class="muted">Every phone or browser that joined with an invite is listed here. A device is trusted by its own code, not by the person who owns it, so two phones for one person are two devices.</p>
  {#if note}<p class="note" role="status">{note}</p>{/if}

  {#if live.length === 0}
    <p class="muted">No devices have joined yet. Make an invite under Sharing.</p>
  {/if}
  {#each live as d (d.id)}
    <div class="dev">
      <div class="grow">
        <b>{d.label || d.person}</b> <span class="muted small">{d.person} · {KIND[d.kind]} · code {d.id}</span>
        <div class="small muted">Joined {day(d.since)} · last reached the Hub {day(d.last_seen)}</div>
        {#if d.state === 'erase-ordered'}<div class="warn small">{STATE[d.state]} since {day(d.state_at)}. It is told the next time it connects.</div>{/if}
      </div>
      {#if d.state === 'erase-ordered'}
        <button class="btn" onclick={() => act(d.id, 'cancel')} title="The phone was found: take the erase order back">Found it: cancel the erase</button>
        <button class="btn danger" onclick={() => act(d.id, 'lock')} title="Shut this device out now, without waiting for it to connect">Lock out now</button>
      {:else if asking === d.id}
        <span class="small">Erase this device's Cindrix data the next time it connects, and lock it out?</span>
        <button class="btn danger" onclick={() => act(d.id, 'erase')}>Yes, erase it</button>
        <button class="btn" onclick={() => (asking = '')}>No</button>
      {:else}
        <button class="btn" onclick={() => (asking = d.id)} title="For a lost or stolen phone">Erase this device</button>
        <button class="btn" onclick={() => act(d.id, 'lock')} title="Shut this device out now. It has to start over with an invite.">Lock out</button>
      {/if}
    </div>
  {/each}

  <details class="more">
    <summary>What erase does, and what it cannot do</summary>
    <ul>
      <li>It is an <b>order that waits</b>. The Hub cannot reach a phone that is off or far away. The next time the phone reaches the Hub, it is told to erase.</li>
      <li>On a <b>Pocket phone</b> it erases everything Cindrix keeps there: saved cards, reports not yet sent, saved maps, and the sign-in. It cannot wipe the rest of the phone.</li>
      <li>On a <b>browser</b> it can only lock the device out and ask the page to clear what it saved. It cannot reach the browser's other data.</li>
      <li>Once it is done, the device's sign-in is removed. It has to start over with a new invite.</li>
      <li>It does <b>not</b> keep the phone off your Wi-Fi or hotspot. Change the hotspot password for that, and make a new PIN so anyone who knew the old one is signed out.</li>
      <li>If the phone was never switched on again, the order never arrives. Locking it out stops it from signing in to the Hub either way.</li>
    </ul>
  </details>
</div>

{#if gone.length}
  <div class="panel">
    <h3>Locked out</h3>
    {#each gone as d (d.id)}
      <div class="dev quiet">
        <div class="grow"><b>{d.label || d.person}</b> <span class="muted small">{d.person} · {KIND[d.kind]} · code {d.id}</span>
          <div class="small muted">{STATE[d.state]} {day(d.state_at)}</div></div>
      </div>
    {/each}
  </div>
{/if}

<style>
  .dev { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; padding: 10px 0; border-top: 1px solid var(--line); }
  .dev.quiet { opacity: 0.8; }
  .grow { flex: 1; min-width: 220px; }
  .small { font-size: 12px; }
  .warn { color: var(--danger-text, var(--danger)); }
  .note { padding: 8px 10px; border-radius: 8px; background: var(--ok-bg, var(--panel)); }
  .more { margin-top: 14px; }
  .more summary { cursor: pointer; font-weight: 600; }
  .more ul { margin: 8px 0 0; padding-left: 20px; display: flex; flex-direction: column; gap: 6px; }
</style>
