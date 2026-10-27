<script lang="ts">
  // The passcode card for one person's private notes (journal, own photos, comms plan).
  // `needsUnlock` is set by the page when a call came back "locked"; `onchange` runs after a lock/unlock so the page reloads.
  import { lockStatus, lockSet, lockUnlock, lockRelock, lockChange, lockRemove, type LockStatus } from './api';

  let { person, needsUnlock = false, onchange, what = 'journal, own pictures and comms plan' }: { person: string; needsUnlock?: boolean; onchange: () => void; what?: string } = $props();

  let status: LockStatus = $state({ protected: false, unlocked: true });
  let mode: '' | 'set' | 'change' | 'remove' = $state('');
  let pass = $state('');
  let again = $state('');
  let old = $state('');
  let note = $state('');
  let busy = $state(false);
  const show = $derived(status.protected && (!status.unlocked || needsUnlock));

  async function refresh() {
    try { status = await lockStatus(person); } catch { /* keep what we know */ }
  }
  $effect(() => { person; needsUnlock; mode = ''; pass = ''; again = ''; old = ''; note = ''; void refresh(); });

  async function run(f: () => Promise<unknown>, done: string) {
    if (busy) return;
    busy = true; note = '';
    try { await f(); mode = ''; pass = ''; again = ''; old = ''; await refresh(); note = done; onchange(); }
    catch (e) { note = String(e).replace(/^Error:\s*/, ''); }
    busy = false;
  }
  const unlock = () => run(() => lockUnlock(person, pass), '');
  function setIt() {
    if (pass !== again) { note = 'The two passcodes are not the same.'; return; }
    void run(() => lockSet(person, pass), 'Your private notes now have a passcode.');
  }
  function changeIt() {
    if (pass !== again) { note = 'The two new passcodes are not the same.'; return; }
    void run(() => lockChange(person, old, pass), 'The passcode was changed.');
  }
  const removeIt = () => run(() => lockRemove(person, pass), 'The passcode was taken off.');
  const lockNow = () => run(() => lockRelock(person), 'Locked.');
</script>

{#if show}
  <form class="panel card" onsubmit={(e) => { e.preventDefault(); void unlock(); }}>
    <strong>{person ? person + '’s' : 'These'} private notes are locked</strong>
    <p class="muted">Type the passcode to open the {what}.</p>
    <div class="row">
      <input class="input" type="password" autocomplete="off" bind:value={pass} aria-label="Passcode" placeholder="Passcode" />
      <button class="btn primary" type="submit" disabled={!pass || busy}>Unlock</button>
    </div>
    {#if note}<p class="notice" role="status">{note}</p>{/if}
  </form>
{:else}
  <div class="row bar">
    {#if status.protected}
      <span class="muted small">🔒 Protected by a passcode. Only {person || 'this person'} can read it.</span>
      <button class="btn" onclick={lockNow} disabled={busy}>Lock now</button>
      <button class="btn" onclick={() => (mode = mode === 'change' ? '' : 'change')}>Change passcode</button>
      <button class="btn" onclick={() => (mode = mode === 'remove' ? '' : 'remove')}>Take the passcode off</button>
    {:else}
      <button class="btn" onclick={() => (mode = mode === 'set' ? '' : 'set')} title="Keep this person’s journal, own pictures and comms plan so that nobody else can read them">🔒 Protect with a passcode</button>
    {/if}
  </div>
  {#if mode === 'set'}
    <form class="panel card" onsubmit={(e) => { e.preventDefault(); setIt(); }}>
      <strong>Choose a passcode</strong>
      <p class="muted small">At least 6 characters. It protects the {what}. Anyone using this computer, and anyone who copies the drive, will see only scrambled files. <b>If you forget the passcode, nobody can get these back.</b> Write it down and keep it somewhere safe.</p>
      <input class="input" type="password" autocomplete="new-password" bind:value={pass} aria-label="New passcode" placeholder="Passcode" />
      <input class="input" type="password" autocomplete="new-password" bind:value={again} aria-label="Type it again" placeholder="Type it again" />
      <div class="row"><button class="btn primary" type="submit" disabled={!pass || busy}>{busy ? 'Protecting…' : 'Protect'}</button></div>
    </form>
  {:else if mode === 'change'}
    <form class="panel card" onsubmit={(e) => { e.preventDefault(); changeIt(); }}>
      <strong>Change the passcode</strong>
      <input class="input" type="password" autocomplete="off" bind:value={old} aria-label="Current passcode" placeholder="Current passcode" />
      <input class="input" type="password" autocomplete="new-password" bind:value={pass} aria-label="New passcode" placeholder="New passcode (at least 6 characters)" />
      <input class="input" type="password" autocomplete="new-password" bind:value={again} aria-label="Type the new one again" placeholder="Type the new one again" />
      <div class="row"><button class="btn primary" type="submit" disabled={!old || !pass || busy}>Change</button></div>
    </form>
  {:else if mode === 'remove'}
    <form class="panel card" onsubmit={(e) => { e.preventDefault(); void removeIt(); }}>
      <strong>Take the passcode off</strong>
      <p class="muted small">The files go back to being ordinary files that anyone using this computer can read.</p>
      <input class="input" type="password" autocomplete="off" bind:value={pass} aria-label="Passcode" placeholder="Passcode" />
      <div class="row"><button class="btn primary" type="submit" disabled={!pass || busy}>Take it off</button></div>
    </form>
  {/if}
  {#if note}<p class="notice" role="status">{note}</p>{/if}
{/if}

<style>
  .card { display: flex; flex-direction: column; gap: 8px; max-width: 520px; margin-top: 10px; border-left: 3px solid var(--cat-morale); }
  .bar { margin-top: 8px; gap: 8px; align-items: center; flex-wrap: wrap; }
  .small { font-size: 13px; }
  .notice { margin: 8px 0 0; }
</style>
