<script lang="ts">
  import { onMount } from 'svelte';
  import {
    safeBackupsWaiting, safeBackupsKept, safeBackupKeep, safeBackupSetAside, safeBackupRemove, safeBackupToOutbox,
    type SafeBackup,
  } from '../lib/api';

  let waiting: SafeBackup[] = $state([]);
  let kept: SafeBackup[] = $state([]);
  let note = $state('');
  let error = $state('');
  let confirmRemove = $state('');

  async function load() {
    try {
      waiting = await safeBackupsWaiting();
      kept = await safeBackupsKept();
    } catch (e) {
      error = String(e);
    }
  }
  onMount(load);

  function size(n: number): string {
    if (n < 1024 * 1024) return `${Math.max(1, Math.round(n / 1024))} KB`;
    if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
    return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }
  function when(t: number): string {
    return t ? new Date(t * 1000).toLocaleString() : '';
  }
  async function run(f: () => Promise<unknown>, done: string) {
    error = ''; note = '';
    try { await f(); if (done) note = done; } catch (e) { error = String(e); }
    confirmRemove = '';
    await load();
  }
</script>

<div class="panel">
  <div class="label">BACKUPS FROM CINDRIX SAFE</div>
  <p>Cindrix Safe on your phone can leave a backup of your documents here. The backup is locked with your Safe passphrase. This program keeps it safe but cannot open it, and nobody else can without your passphrase or recovery code.</p>
  <p class="muted fine">In Safe, open Backup and restore and choose the Cindrix Ops inbox, or send the file with AirLink into <code>user/inbox/safe</code>.</p>
  {#if note}<p class="ok">{note}</p>{/if}
  {#if error}<div class="warn" role="alert">{error}</div>{/if}
</div>

<div class="panel">
  <div class="label">WAITING ({waiting.length})</div>
  {#if waiting.length === 0}
    <p class="muted">Nothing is waiting.</p>
  {:else}
    {#each waiting as w (w.name)}
      <div class="wrow">
        <div class="grow">
          <strong>{w.name}</strong>
          <span class="muted">: {size(w.size)}, {when(w.modified)}{w.problem ? '' : `, ${w.items} saved ${w.items === 1 ? 'item' : 'items'}`}</span>
          {#if w.problem}<div class="warn">{w.problem}</div>{/if}
        </div>
        {#if !w.problem}
          <button class="btn primary" onclick={() => run(() => safeBackupKeep(w.name), 'Backup kept.')} title="Move this backup into your kept backups">Keep</button>
        {/if}
        <button class="btn" onclick={() => run(() => safeBackupSetAside(w.name), 'Set aside. The file is in the done folder.')} title="Set this file aside without keeping it">Not now</button>
      </div>
    {/each}
  {/if}
</div>

<div class="panel">
  <div class="label">KEPT ({kept.length})</div>
  {#if kept.length === 0}
    <p class="muted">No backups kept yet.</p>
  {:else}
    {#each kept as k (k.name)}
      <div class="wrow">
        <div class="grow">
          <strong>{k.name}</strong>
          <span class="muted">: {size(k.size)}, {when(k.modified)}, {k.items} saved {k.items === 1 ? 'item' : 'items'}</span>
        </div>
        <button class="btn" onclick={() => run(async () => { const p = await safeBackupToOutbox(k.name); note = `A copy is ready to send back to the phone: ${p}`; }, '')} title="Make a copy in the outbox so you can send it to the phone with AirLink">Copy to outbox</button>
        {#if confirmRemove === k.name}
          <button class="btn danger" onclick={() => run(() => safeBackupRemove(k.name), 'Backup deleted.')}>Yes, delete it</button>
          <button class="btn" onclick={() => (confirmRemove = '')}>No</button>
        {:else}
          <button class="btn" onclick={() => (confirmRemove = k.name)} title="Delete this kept backup for good">Delete</button>
        {/if}
      </div>
    {/each}
    <p class="muted fine">To restore on the phone, send the copy back and use Restore from a backup in Safe. You will need the passphrase the vault had when the backup was made.</p>
  {/if}
</div>

<style>
  .wrow { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; padding: 8px 0; border-bottom: 1px solid var(--line2); }
  .grow { flex: 1; min-width: 220px; }
  .ok { color: var(--blue); }
  .btn.danger { border-color: var(--red, #c0392b); color: var(--red, #c0392b); }
</style>
