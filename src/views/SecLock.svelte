<script lang="ts">
  // The passcode for the security records and the device sign-in files (docs/47). `gate` shows only the unlock box,
  // for the Security Operations screen while the records are locked; otherwise this is the Settings panel.
  import { onMount } from 'svelte';
  import { secLockStatus, secLockSet, secLockUnlock, secLockRelock, secLockChange, secLockRemove, secLockMakeRecovery, secLockReset, type SecLockStatus } from '../lib/api';

  let { gate = false, onchange = () => {} }: { gate?: boolean; onchange?: () => void } = $props();

  let st = $state<SecLockStatus | null>(null);
  let pass = $state('');
  let pass2 = $state('');
  let oldPass = $state('');
  let note = $state('');
  let error = $state('');
  let busy = $state(false);
  let mode = $state<'' | 'change' | 'remove' | 'recovery'>('');
  let recovery = $state('');
  let forgot = $state(false);
  let code = $state('');

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
    return run(async () => { const r = await secLockSet(pass); recovery = r.recovery; return `The lock is on. ${r.sealed} file${r.sealed === 1 ? ' is' : 's are'} now locked.`; });
  };
  const newRecovery = () => run(async () => { recovery = await secLockMakeRecovery(oldPass); return 'Here is the new recovery code. The old one no longer works.'; });
  const resetPass = () => {
    if (pass !== pass2) { error = 'The two passcodes are not the same.'; return; }
    return run(async () => { await secLockReset(code, pass); code = ''; forgot = false; return 'The passcode is reset and the records are open.'; });
  };
  const unlock = () => run(() => secLockUnlock(pass));
</script>

{#snippet forgotForm()}
  {#if forgot}
    <div class="row">
      <input class="input" autocomplete="off" placeholder="Recovery code" aria-label="Recovery code" bind:value={code} />
      <input class="input" type="password" autocomplete="new-password" placeholder="New passcode (6 or more characters)" aria-label="New passcode" bind:value={pass} />
      <input class="input" type="password" autocomplete="new-password" placeholder="Type it again" aria-label="Type the new passcode again" bind:value={pass2} />
      <button class="btn primary" onclick={resetPass} disabled={busy || !code || !pass} title="Set a new passcode using the recovery code">Reset the passcode</button>
    </div>
    <p class="muted fine">Only the person who kept the recovery code can do this. The records are not changed, only the passcode.</p>
  {:else}
    <div class="row"><button class="btn" onclick={() => (forgot = true)} title="Use the recovery code to set a new passcode">Forgot the passcode?</button></div>
  {/if}
{/snippet}

{#if gate}
  <div class="panel gate">
    <h2>The security records are locked</h2>
    <p class="muted">Type the passcode to open them. Until then nothing in Security Operations can be read, and phones and other devices cannot use the Hub.</p>
    {#if error}<p class="warn" role="alert">{error}</p>{/if}
    <form class="row" onsubmit={(e) => { e.preventDefault(); unlock(); }}>
      <input class="input" type="password" autocomplete="off" placeholder="Passcode" aria-label="Passcode" bind:value={pass} />
      <button class="btn primary" type="submit" disabled={busy || !pass}>Unlock</button>
    </form>
    {@render forgotForm()}
  </div>
{:else}
  <div class="panel">
    <h2>Lock the security records</h2>
    <p class="muted">Locks everything under Security Operations (watchbill, incidents, radio plan, check-ins) and the list of trusted phones and the share PIN, so someone who finds this drive cannot read them. It does not cover your pictures or journal (those have their own lock on the Family page), or reports from phones that are waiting to be accepted. If you forget the passcode, the locked files cannot be opened by anyone, including us. Keep a backup copy of the passcode somewhere safe.</p>
    {#if error}<p class="warn" role="alert">{error}</p>{/if}
    {#if note}<p class="note" role="status">{note}</p>{/if}
    {#if recovery}
      <div class="recovery" role="alert">
        <p><b>Recovery code</b> — write it down and keep it somewhere safe, apart from this drive. It is shown only now.</p>
        <p class="code">{recovery}</p>
        <p class="muted fine">If the passcode is forgotten, this code lets the person who holds it set a new passcode without losing the records. Anyone who has it can do that, so do not share it.</p>
        <button class="btn" onclick={() => (recovery = '')}>I have written it down</button>
      </div>
    {/if}
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
      {@render forgotForm()}
    {:else if st}
      <p>The lock is on and the records are open. They lock again when you press Lock, or when the program closes.</p>
      <div class="row">
        <button class="btn" onclick={() => run(secLockRelock)} disabled={busy} title="Close the records now">Lock now</button>
        <button class="btn" onclick={() => (mode = mode === 'change' ? '' : 'change')} disabled={busy}>Change the passcode</button>
        <button class="btn" onclick={() => (mode = mode === 'remove' ? '' : 'remove')} disabled={busy} title="Write the records back as plain files">Take the lock off</button>
        <button class="btn" onclick={() => (mode = mode === 'recovery' ? '' : 'recovery')} disabled={busy} title="Make a recovery code, or replace the one you have">{st.has_recovery ? 'New recovery code' : 'Make a recovery code'}</button>
      </div>
      {#if !st.has_recovery}<p class="warn fine">This lock has no recovery code yet. Without one, a forgotten passcode cannot be reset.</p>{/if}
      {#if mode === 'change'}
        <div class="row">
          <input class="input" type="password" autocomplete="off" placeholder="Current passcode" aria-label="Current passcode" bind:value={oldPass} />
          <input class="input" type="password" autocomplete="new-password" placeholder="New passcode" aria-label="New passcode" bind:value={pass} />
          <button class="btn primary" onclick={() => run(async () => { await secLockChange(oldPass, pass); return 'The passcode is changed.'; })} disabled={busy || !oldPass || !pass}>Save</button>
        </div>
      {:else if mode === 'recovery'}
        <div class="row">
          <input class="input" type="password" autocomplete="off" placeholder="Current passcode" aria-label="Current passcode" bind:value={oldPass} />
          <button class="btn primary" onclick={newRecovery} disabled={busy || !oldPass}>Make the code</button>
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
  .recovery { border: 1px solid var(--line); border-radius: 8px; padding: 10px 12px; margin: 8px 0; }
  .code { font-size: 22px; font-weight: 700; letter-spacing: .12em; font-family: monospace; }
</style>
