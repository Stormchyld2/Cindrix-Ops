<script lang="ts">
  // Keep another computer or phone in step: gear, inventory and security records are copied both ways by themselves
  // while both are on the same network. Core: core/src/peer.rs and peersync.rs (docs/34, docs/61).
  import { onMount } from 'svelte';
  import { linkStatus, linkJoin, linkUnlink, linkSyncNow, linkDismiss, type LinkStatus } from '../lib/api';

  let st = $state<LinkStatus | null>(null);
  let note = $state('');
  let url = $state('');
  let name = $state('');
  let busy = $state(false);

  async function load() { try { st = await linkStatus(); } catch (e) { note = String(e); } }
  onMount(load);

  async function join() {
    busy = true;
    try { st = await linkJoin(url, name); note = 'Linked. The two devices will copy by themselves every few minutes.'; url = ''; } catch (e) { note = String(e); } finally { busy = false; }
  }
  async function unlink() {
    try { st = await linkUnlink(); note = 'The link was removed. Nothing was deleted on either device.'; } catch (e) { note = String(e); }
  }
  async function now() {
    busy = true;
    try {
      const r = await linkSyncNow();
      note = `Done. Gear changed: ${r.gear_taken}. Inventory changed: ${r.inventory_changes}. Security lines added: ${r.log_lines}.${r.notes.length ? ' ' + r.notes.join(' ') : ''}`;
      await load();
    } catch (e) { note = String(e); } finally { busy = false; }
  }
  async function dismiss(file: string, line: string) {
    try { st = await linkDismiss(file, line); } catch (e) { note = String(e); }
  }
  const when = (t: number) => (t ? new Date(t * 1000).toLocaleString([], { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }) : 'not yet');
</script>

<div class="panel">
  <h2>Keep another device in step</h2>
  <p class="muted small">Link your laptop and phone so gear, inventory and security records are copied between them by themselves while both are on the same Wi-Fi or hotspot. Nothing goes over the internet. Anything that clashes is kept for you to look at, never overwritten.</p>
  {#if note}<p class="small" role="status">{note}</p>{/if}
  {#if st?.linked}
    <p>Linked to <b>{st.address}</b> as <b>{st.name}</b>. Last copied: {when(st.last_at)}.</p>
    {#each st.last_notes as n}<p class="small">{n}</p>{/each}
    <button class="btn" onclick={now} disabled={busy} title="Copy with the other device right now">{busy ? 'Copying...' : 'Copy now'}</button>
    <button class="btn" onclick={unlink} title="Stop copying with the other device. Nothing is deleted">Remove the link</button>
    {#if st.gear_waiting}<p class="small">{st.gear_waiting} gear sheet(s) wait for you on the Gear page.</p>{/if}
    {#if st.held.length}
      <h3>Held for you to look at</h3>
      <p class="muted small">These lines from the other device use a number this device already uses for something different. Neither side was changed. If you want one of them, add it by hand, then clear it here.</p>
      {#each st.held as h (h.file + h.line)}
        <div class="line"><span class="grow small"><b>{h.file}</b>: {h.why}<br />{h.line.replace(/\t/g, '  ')}</span><button class="btn" onclick={() => dismiss(h.file, h.line)} title="Clear this from the list. The log is not changed">Clear</button></div>
      {/each}
    {/if}
  {:else}
    <p class="small">On the other device: turn sharing on, open Settings, Sharing, make an invite with no name, and copy the address shown under the QR code. In People and roles there, give the name you choose below a role that has the copy permission (add a role for it if needed). Then paste the address here.</p>
    <label class="small">Invite address <input bind:value={url} placeholder="http://192.168.43.1:8080/join?c=..." style="width: 24em" /></label>
    <label class="small">A name for this device <input bind:value={name} maxlength="40" placeholder="Laptop" /></label>
    <button class="btn primary" onclick={join} disabled={busy || !url.trim() || !name.trim()} title="Join the other device with this invite and start copying">Link</button>
  {/if}
</div>

<style>
  .line { display: flex; gap: 12px; align-items: center; padding: 6px 0; border-top: 1px solid var(--line); }
  .grow { flex: 1; }
  .small { font-size: 12px; }
</style>
