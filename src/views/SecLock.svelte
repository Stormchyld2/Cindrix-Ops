<script lang="ts">
  // The passcode for the security records and the device sign-in files (docs/47). `gate` shows only the unlock box,
  // for the Security Operations screen while the records are locked; otherwise this is the Settings panel.
  import { onMount } from 'svelte';
  import { secLockStatus, secLockSet, secLockUnlock, secLockRelock, secLockChange, secLockRemove, type SecLockStatus } from '../lib/api';

  let { gate = false, onchange = () => {} }: { gate?: boolean; onchange?: () => void } = $props();

  let st = $state<SecLockStatus | null>(null);
  let pass = $state('');
  let pass2 = $state('');
  let oldPass = $state('');
  let note = $state('');
  let error = $state('');
  let busy = $state(false);
  let mode = $state<'' | 'change' | 'remove'>('');

  async function load() { try { st = await secLockStatus(); } catch (e) { error = String(e); } }
  onMount(load);
  async function run(f: () => Promise<string | void>) {
    error = ''; note = ''; busy = true;
    try { const r = await f(); if (r) note = r; pass = ''; pass2 = ''; oldPass = ''; mode = ''; } catch (e) { error = String(e).replace(/^Error:\s*/, ''); }
    busy = false;
    await load();
    onchange();
  }
  const turnOn = () => {
    if (pass !== pass2) { error = 'The two passcodes are not the same.'; return; }
    return run(async () => { const n = await secLockSet(pass); return `The lock is on. ${n} file${n === 1 ? ' is' : 's are'} now locked.`; });
  };
  const unlock = () => run(() => secLockUnlock(pass));
</script>

{#if gate}
  <div class="panel gate">
    <h2>The security records are locked</h2>
    <p class="muted">Type the passcode to open them. Until then nothing in Security Operations can be read, and phones and other devices cannot use the Hub.</p>
    {#if error}<p class="warn" role="alert">{error}</p>{/if}
    <form class="row" onsubmit={(e) => { e.preventDefault(); unlock(); }}>
      <input class="input" type="password" autocomplete="off" placeholder="Passcode" aria-label="Passcode" bind:value={pass} />
      <button class="btn primary" type="submit" disabled={busy || !pass}>Unlock</button>
    </form>
  </div>
{:else}
  <div class="panel">
    <h2>Lock the security records</h2>
    <p class="muted">Locks everything under Security Operations (watchbill, incidents, radio plan, check-ins) and the list of trusted phones and the share PIN, so someone who finds this drive cannot read them. It does not cover your pictures or journal (those have their own lock on the Family page), or reports from phones that are waiting to be accepted. If you forget the passcode, the locked files cannot be opened by anyone, including us. Keep a backup copy of the passcode somewhere safe.</p>
    {#if error}<p class="warn" role="alert">{error}</p>{/if}
    {#if note}<p class="note" role="status">{note}</p>{/if}
    {#if st && !st.on}
      <div class="row">
        <input class="input" type="password" autocomplete="new-password" placeholder="New passcode (6 or more characters)" aria-label="New passcode" bind:value={pass} />
        <input class="input" type="password" autocomplete="new-password" placeholder="Type it again" aria-label="Type the passcode again" bind:value={pass2} />
        <button class="btn primary" onclick={turnOn} disabled={busy || !pass} title="Lock the records with this passcode">Turn the lock on</button>
      </div>
    {:else if st && !st.unlocked}
      <form class="row" onsubmit={(e) => { e.preventDefault(); unlock(); }}>
        <input class="input" type="password" autocomplete="off" placeholder="Passcode" aria-label="Passcode" bind:value={pass} />
        <button class="btn primary" type="submit" disabled={busy || !pass}>Unlock</button>
      </form>
    {:else if st}
      <p>The lock is on and the records are open. They lock again when you press Lock, or when the program closes.</p>
      <div class="row">
        <button class="btn" onclick={() => run(secLockRelock)} disabled={busy} title="Close the records now">Lock now</button>
        <button class="btn" onclick={() => (mode = mode === 'change' ? '' : 'change')} disabled={busy}>Change the passcode</button>
        <button class="btn" onclick={() => (mode = mode === 'remove' ? '' : 'remove')} disabled={busy} title="Write the records back as plain files">Take the lock off</button>
      </div>
      {#if mode === 'change'}
        <div class="row">
          <input class="input" type="password" autocomplete="off" placeholder="Current passcode" aria-label="Current passcode" bind:value={oldPass} />
          <input class="input" type="password" autocomplete="new-password" placeholder="New passcode" aria-label="New passcode" bind:value={pass} />
          <button class="btn primary" onclick={() => run(async () => { await secLockChange(oldPass, pass); return 'The passcode is changed.'; })} disabled={busy || !oldPass || !pass}>Save</button>
        </div>
      {:else if mode === 'remove'}
        <div class="row">
          <input class="input" type="password" autocomplete="off" placeholder="Passcode" aria-label="Passcode" bind:value={oldPass} />
          <button class="btn primary" onclick={() => run(async () => { const n = await secLockRemove(oldPass); return `The lock is off. ${n} file${n === 1 ? ' is' : 's are'} plain again.`; })} disabled={busy || !oldPass}>Take it off</button>
        </div>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .gate { max-width: 560px; }
  .row { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-top: 8px; }
  .input { min-width: 220px; }
</style>
