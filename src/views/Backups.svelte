<script lang="ts">
  // Settings > Backups (core/src/userbackup.rs). A copy of your own files (Family page, inventory, security records,
  // plans, tasks) in another place. Every file is read back and checked. Nothing is overwritten or deleted.
  import { onMount } from 'svelte';
  import { backupStatus, backupSetDestination, backupNow, backupCheck, backupBringBack, pickFolder, type BackupStatus } from '../lib/api';

  let st = $state<BackupStatus | null>(null);
  let note = $state('');
  let error = $state('');
  let busy = $state(false);
  let asking = $state('');

  async function load() { try { st = await backupStatus(); } catch (e) { error = String(e); } }
  onMount(load);
  async function run(f: () => Promise<string | void>) {
    error = ''; note = ''; busy = true;
    try { const r = await f(); if (r) note = r; } catch (e) { error = String(e); }
    busy = false; asking = '';
    await load();
  }
  const choose = () => run(async () => { const p = await pickFolder(); if (p) { await backupSetDestination(p); return 'Saved. Copies will go there.'; } });
  const when = (stamp: string) => `${stamp.slice(0, 10)} ${stamp.slice(11, 13)}:${stamp.slice(13, 15)}`;
</script>

<div class="panel">
  <h2>Back up your own files</h2>
  <p class="muted">Your Family page, inventory, plans, tasks and security records live in one folder on this drive. If the drive breaks or is lost, they go with it. A backup is a plain copy of that folder somewhere else, ideally on another drive, and it is checked file by file. Keep it somewhere as safe as the drive: locked files stay locked in the copy, and anything not locked can be read by whoever holds it.</p>
  {#if error}<p class="warn" role="alert">{error}</p>{/if}
  {#if note}<p class="note" role="status">{note}</p>{/if}

  <div class="row">
    <div class="grow"><b>Where copies go:</b> {st?.destination || 'not chosen yet'}{#if st && st.destination && !st.reachable} <span class="warn">(cannot be reached right now: is the drive plugged in?)</span>{/if}</div>
    <button class="btn" onclick={choose} disabled={busy}>Choose a folder</button>
    <button class="btn primary" onclick={() => run(backupNow)} disabled={busy || !st?.destination || !st?.reachable} title="Copy everything now and check it">{busy ? 'Working…' : 'Back up now'}</button>
  </div>
  {#if st && st.destination}
    <p class="muted">{st.days_since === null ? 'No copy has been made yet.' : st.days_since === 0 ? 'The newest copy is from today.' : `The newest copy is ${st.days_since} day${st.days_since === 1 ? '' : 's'} old.`}</p>
  {/if}
</div>

{#if st && st.backups.length}
  <div class="panel">
    <h3>Copies</h3>
    {#each st.backups as b (b.name)}
      <div class="row item">
        <div class="grow"><b>{when(b.stamp)}</b> <span class="muted small">{b.files} files</span></div>
        <button class="btn" onclick={() => run(() => backupCheck(b.name))} disabled={busy} title="Read every file back and compare">Check it</button>
        {#if asking === b.name}
          <span class="small">Put back only the files that are missing here? Nothing that is here now is overwritten.</span>
          <button class="btn primary" onclick={() => run(() => backupBringBack(b.name))} disabled={busy}>Yes, put them back</button>
          <button class="btn" onclick={() => (asking = '')}>No</button>
        {:else}
          <button class="btn" onclick={() => (asking = b.name)} disabled={busy} title="Add back files that are missing from this drive">Bring back missing files</button>
        {/if}
      </div>
    {/each}
    <p class="muted small">To replace everything with a copy, close the program, then copy the files from the copy's folder over the <code>user</code> folder yourself. Bringing back missing files is the safe version of that.</p>
  </div>
{/if}

<style>
  .row { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .item { padding: 8px 0; border-top: 1px solid var(--line); }
  .grow { flex: 1; min-width: 220px; }
  .small { font-size: 12px; }
  .warn { color: var(--danger-text, var(--danger)); }
  .note { padding: 8px 10px; border-radius: 8px; background: var(--panel); }
</style>
