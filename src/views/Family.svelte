<script lang="ts">
  import { familyStatus, familyLoad, familySave, type Family, type Person } from '../lib/api';
  import { onMount } from 'svelte';

  let family: Family = $state({ people: [], contacts: '', plan: '' });
  let status = $state({ exists: false, locked: false });
  let unlocked = $state(false);
  let passphrase = $state(''); // the passphrase for this session, kept in memory only
  let entry = $state('');
  let error = $state('');
  let saved = $state('');
  let setting = $state(false);
  let newPass = $state('');
  let newPass2 = $state('');

  const blank = (): Person => ({ name: '', conditions: '', medicines: '', allergies: '', doctor: '' });

  onMount(refresh);

  async function refresh() {
    try {
      status = await familyStatus();
      if (!status.locked) {
        family = await familyLoad();
        if (family.people.length === 0) family.people.push(blank());
        unlocked = true;
      }
    } catch (e) {
      error = String(e);
    }
  }

  async function unlock() {
    error = '';
    try {
      family = await familyLoad(entry);
      if (family.people.length === 0) family.people.push(blank());
      passphrase = entry;
      entry = '';
      unlocked = true;
    } catch {
      error = 'Wrong passphrase, or the file is damaged.';
    }
  }

  async function save() {
    error = '';
    try {
      await familySave(family, passphrase || undefined);
      status = await familyStatus();
      saved = 'Saved' + (passphrase ? ' (locked)' : '');
      setTimeout(() => (saved = ''), 3000);
    } catch (e) {
      error = String(e);
    }
  }

  async function turnOnLock() {
    error = '';
    if (newPass.length < 8) { error = 'Use at least 8 characters.'; return; }
    if (newPass !== newPass2) { error = 'The two passphrases do not match.'; return; }
    passphrase = newPass;
    newPass = ''; newPass2 = ''; setting = false;
    await save();
  }

  async function turnOffLock() {
    passphrase = '';
    await save();
  }

  function addPerson() {
    family.people.push(blank());
  }
  function removePerson(n: number) {
    family.people.splice(n, 1);
  }
</script>

<section>
  <h1>Family</h1>
  <p class="sub">Names, conditions, medicines as your doctor wrote them, allergies and contacts. Saved on this drive only.</p>
</section>

{#if error}<div class="warn" role="alert">{error}</div>{/if}

{#if status.locked && !unlocked}
  <section class="panel narrow">
    <h2>Locked</h2>
    <p class="muted">Enter the passphrase to open this page. There is no way to recover it if it is forgotten.</p>
    <form class="row" onsubmit={(e) => { e.preventDefault(); unlock(); }}>
      <label class="sr" for="pp">Passphrase</label>
      <input id="pp" class="input grow" type="password" bind:value={entry} autocomplete="off" />
      <button class="btn primary" type="submit">Unlock</button>
    </form>
  </section>
{:else if unlocked}
  {#each family.people as p, n}
    <section class="panel">
      <div class="row between">
        <span class="label">PERSON {n + 1}</span>
        {#if family.people.length > 1}<button class="btn" onclick={() => removePerson(n)}>Remove</button>{/if}
      </div>
      <div class="grid">
        <label>Name<input class="input" bind:value={p.name} /></label>
        <label>Doctor and phone<input class="input" bind:value={p.doctor} /></label>
        <label>Conditions<input class="input" bind:value={p.conditions} /></label>
        <label>Allergies<input class="input" bind:value={p.allergies} /></label>
      </div>
      <label class="block">Medicines, exactly as your doctor's plan says
        <textarea class="input" bind:value={p.medicines}></textarea>
      </label>
    </section>
  {/each}
  <div class="row"><button class="btn" onclick={addPerson}>Add a person</button></div>

  <section class="panel">
    <label class="block">Emergency contacts
      <textarea class="input" bind:value={family.contacts}></textarea>
    </label>
    <label class="block">Our plan: where we meet, who calls whom
      <textarea class="input" bind:value={family.plan}></textarea>
    </label>
  </section>

  <section class="panel">
    <div class="label">PASSPHRASE LOCK (OPTIONAL)</div>
    {#if passphrase}
      <p class="muted">This page is locked. Anyone opening the file without the passphrase sees only scrambled text.</p>
      <button class="btn" onclick={turnOffLock}>Turn the lock off</button>
    {:else if setting}
      <p class="warn">If you forget this passphrase, the information cannot be recovered. Print a plain copy first.</p>
      <div class="grid">
        <label>Passphrase<input class="input" type="password" bind:value={newPass} autocomplete="off" /></label>
        <label>Again<input class="input" type="password" bind:value={newPass2} autocomplete="off" /></label>
      </div>
      <div class="row"><button class="btn primary" onclick={turnOnLock}>Lock this page</button><button class="btn" onclick={() => (setting = false)}>Cancel</button></div>
    {:else}
      <p class="muted">Off. The file on this drive is not encrypted. The lock covers this page only, not the library or maps.</p>
      <button class="btn" onclick={() => (setting = true)}>Set a passphrase</button>
    {/if}
  </section>

  <div class="row">
    <button class="btn primary" onclick={save}>Save to this drive</button>
    {#if saved}<span class="ok">{saved}</span>{/if}
  </div>
{/if}

<style>
  .between { justify-content: space-between; }
  .narrow { max-width: 520px; }
  .grow { flex: 1 1 200px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 12px; margin: 12px 0; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--muted); }
  .block { margin-top: 8px; }
</style>
