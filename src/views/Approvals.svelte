<script lang="ts">
  // Reports waiting on the host, for a person whose role may accept or set them aside (core/src/permissions.rs).
  import { onMount } from 'svelte';
  import { approvalsList, approvalsAccept, approvalsSetAside, type PocketReport } from '../lib/api';

  let waiting = $state<PocketReport[]>([]);
  let note = $state('');
  let error = $state('');
  let busy = $state(false);

  const kindWords: Record<string, string> = { report: 'Report', 'count-sheet': 'Count sheet', 'checkin-sheet': 'Check-ins', 'track-sheet': 'Walk or drive track' };

  async function load() {
    try { waiting = await approvalsList(); } catch (e) { error = String(e).replace(/^Error:\s*/, ''); }
  }
  onMount(load);
  async function act(f: () => Promise<string>) {
    error = ''; note = ''; busy = true;
    try { note = await f(); } catch (e) { error = String(e).replace(/^Error:\s*/, ''); }
    busy = false;
    await load();
  }
  const accept = (r: PocketReport) => act(async () => { const n = await approvalsAccept(r.name); return `Accepted. ${n} entr${n === 1 ? 'y was' : 'ies were'} added.`; });
  const setAside = (r: PocketReport) => act(async () => { await approvalsSetAside(r.name); return 'Set aside. The host can still look at it.'; });
</script>

<div class="panel">
  <h2>Reports waiting to be accepted</h2>
  <p class="muted">These came from phones and are waiting on the host. Accepting adds them to the host's records with your name noted. Setting one aside keeps it without using it.</p>
  {#if error}<p class="warn" role="alert">{error}</p>{/if}
  {#if note}<p class="note" role="status">{note}</p>{/if}
  {#each waiting as r (r.name)}
    <div class="line">
      <div class="grow">
        <b>{kindWords[r.kind] ?? 'Report'}</b> from {r.person || 'a phone'}
        <div class="small muted">{r.entries} entr{r.entries === 1 ? 'y' : 'ies'}, {r.created}</div>
        {#if r.problem}<div class="small warn">{r.problem}</div>{/if}
      </div>
      <button class="btn primary" onclick={() => accept(r)} disabled={busy || !!r.problem} title="Add this to the host's records">Accept</button>
      <button class="btn" onclick={() => setAside(r)} disabled={busy} title="Keep it, but do not use it">Set aside</button>
    </div>
  {:else}
    <p class="muted">Nothing is waiting.</p>
  {/each}
  <div class="row"><button class="btn" onclick={load} disabled={busy}>Look again</button></div>
</div>

<style>
  .line { display: flex; gap: 12px; align-items: center; padding: 8px 0; border-top: 1px solid var(--line); }
  .grow { flex: 1; }
  .small { font-size: 12px; }
  .row { margin-top: 10px; }
</style>
