<script lang="ts">
  import { familyStatus, familyLoad, familySave, exportText, birthdaysSync, type Family, type Person } from '../lib/api';
  import { onMount } from 'svelte';
  import PageHeader from '../lib/PageHeader.svelte';
  import Icon from '../lib/Icon.svelte';

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

  const named = $derived(family.people.filter((p) => p.name.trim()).length);
  const withMeds = $derived(family.people.filter((p) => p.medicines.trim()).length);
  const withAllergy = $derived(family.people.filter((p) => p.allergies.trim()).length);
  const withConditions = $derived(family.people.filter((p) => p.conditions.trim()).length);
  let contactsBox: HTMLTextAreaElement | undefined = $state();
  function jump(id: string) { document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'center' }); if (id === 'contacts') contactsBox?.focus(); }
  function addContact() {
    family.contacts = (family.contacts.trim() ? family.contacts.replace(/\s+$/, '') + '\n' : '') + 'Name and phone: ';
    setTimeout(() => { contactsBox?.focus(); contactsBox?.setSelectionRange(contactsBox.value.length, contactsBox.value.length); }, 0);
  }
  let sumDlg: HTMLDialogElement;
  function summaryText(): string {
    const out: string[] = ['MEDICAL SUMMARY', 'Printed ' + new Date().toLocaleDateString(), ''];
    family.people.filter((p) => p.name.trim() || p.conditions.trim() || p.medicines.trim() || p.allergies.trim()).forEach((p) => {
      out.push((p.name || '(no name)').toUpperCase());
      if (p.birthday) out.push('Birthday: ' + p.birthday);
      out.push('Doctor and phone: ' + (p.doctor || '-'), 'Conditions: ' + (p.conditions || '-'), 'Allergies: ' + (p.allergies || '-'), "Medicines, as the doctor's plan says:", p.medicines || '-', '');
    });
    out.push('EMERGENCY CONTACTS', family.contacts || '-');
    return out.join('\n');
  }
  function openSummary() { sumDlg.showModal(); }
  function printSummary() {
    const f = document.createElement('iframe');
    f.style.cssText = 'position:fixed;right:0;bottom:0;width:0;height:0;border:0';
    document.body.appendChild(f);
    const d = f.contentDocument;
    if (!d) { f.remove(); return; }
    d.open();
    d.write('<pre style="font:14px/1.5 Arial,sans-serif;white-space:pre-wrap;margin:24px"></pre>');
    d.close();
    d.querySelector('pre')!.textContent = summaryText();
    f.contentWindow?.focus();
    f.contentWindow?.print();
    setTimeout(() => f.remove(), 2000);
  }
  const blank = (): Person => ({ name: '', conditions: '', medicines: '', allergies: '', doctor: '', birthday: '' });

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
      // birthdays go on the Calendar as yearly events (the Calendar is not locked, so only name and day go there)
      try { await birthdaysSync(family.people.map((p) => ({ name: p.name, birthday: p.birthday ?? '' }))); } catch { /* the page itself is saved */ }
      status = await familyStatus();
      saved = 'Saved' + (passphrase ? ' (locked)' : '');
      setTimeout(() => (saved = ''), 3000);
    } catch (e) {
      error = String(e);
    }
  }

  function readable(): string {
    const out = ['FAMILY PAGE (readable copy, not locked)', 'Saved ' + new Date().toLocaleDateString(), ''];
    family.people.forEach((p, n) => {
      out.push('PERSON ' + (n + 1) + ': ' + (p.name || '(no name)'), 'Doctor and phone: ' + p.doctor, 'Conditions: ' + p.conditions, 'Allergies: ' + p.allergies, 'Medicines, as the doctor\'s plan says:', p.medicines || '', '');
    });
    out.push('EMERGENCY CONTACTS', family.contacts || '', '', 'OUR PLAN (where we meet, who calls whom)', family.plan || '');
    return out.join('\r\n');
  }
  async function exportReadable() {
    error = '';
    try {
      const where = await exportText('family-readable.txt', readable());
      saved = 'A readable copy was saved to ' + where + '. It is not locked, so keep it safe.';
      setTimeout(() => (saved = ''), 8000);
    } catch (e) { error = String(e); }
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

<PageHeader icon="users" title="Family" tagline="Names, conditions, medicines as your doctor wrote them, allergies and contacts.">
  {#if unlocked}
    <button class="btn ico" onclick={addPerson}><Icon name="plus" size={18} /> Add person</button>
    <button class="btn ico" onclick={() => jump('contacts')}><Icon name="users" size={18} /> Contacts</button>
    <button class="btn ico" onclick={exportReadable} title="Save a plain text copy anyone can read, even without this program"><Icon name="save" size={18} /> Save a copy</button>
    <button class="btn ico primary" onclick={save}><Icon name="save" size={18} /> Save to drive</button>
  {/if}
</PageHeader>

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
  <div class="fgrid">
    <div class="leftcol">
      <section class="dbl" style="--c: var(--glow-blue)">
        <div class="pnl-h nosl"><Icon name="users" size={24} /> Family overview</div>
        <div class="ov4">
          <div class="ot" style="--c: var(--glow-blue)"><span>Family members</span><Icon name="user" size={34} /><b>{named}</b></div>
          <div class="ot" style="--c: var(--glow-green)"><span>Medications</span><Icon name="pills" size={34} /><b>{withMeds}</b></div>
          <div class="ot" style="--c: var(--glow-amber)"><span>Allergies</span><Icon name="warn" size={34} /><b>{withAllergy}</b></div>
          <div class="ot" style="--c: var(--glow-red)"><span>Conditions</span><Icon name="heart" size={34} /><b>{withConditions}</b></div>
        </div>
      </section>
      <section class="dbl" style="--c: var(--glow-blue)">
        <div class="pnl-h nosl"><Icon name="users" size={24} /> Quick access</div>
        <div class="qa4">
          <button class="qt" onclick={() => jump('contacts')}><Icon name="phone" size={32} /><span>Emergency Contacts</span></button>
          <button class="qt" onclick={() => jump('plan')}><Icon name="pin" size={32} /><span>Our Plan (Meet / Call)</span></button>
          <button class="qt" onclick={openSummary}><Icon name="cal" size={32} /><span>Medical Summary</span></button>
          <button class="qt" onclick={() => jump('lock')}><Icon name="lock" size={32} /><span>Passphrase Lock</span></button>
        </div>
      </section>
    </div>
    <div class="people">
      {#each family.people as p, n}
        <section class="dbl person" style="--c: var(--glow-blue)">
          <div class="row between">
            <div class="pnl-h nosl"><Icon name="user" size={22} /> Person {n + 1}</div>
            {#if family.people.length > 1}<button class="ib red" aria-label="Remove this person" title="Remove this person" onclick={() => removePerson(n)}><Icon name="trash" size={18} /></button>{/if}
          </div>
          <div class="grid">
            <label>Name<input class="input" bind:value={p.name} /></label>
            <label>Doctor and phone<input class="input" bind:value={p.doctor} /></label>
            <label>Conditions<input class="input" bind:value={p.conditions} /></label>
            <label>Allergies<input class="input" bind:value={p.allergies} /></label>
          </div>
          <div class="grid2">
            <label>Birthday <span class="hint">(shown on the Calendar every year)</span><input class="input" type="date" aria-label="Birthday" bind:value={p.birthday} /></label>
            <label class="block">Medicines, exactly as your doctor's plan says
              <textarea class="input" bind:value={p.medicines}></textarea>
            </label>
          </div>
        </section>
      {/each}
      <div class="two">
        <section class="dbl" id="contacts" style="--c: var(--glow-blue)">
          <div class="pnl-h nosl"><Icon name="phone" size={22} /> Emergency contacts</div>
          <textarea class="input" bind:this={contactsBox} aria-label="Emergency contacts" bind:value={family.contacts}></textarea>
          <div class="row mid"><button class="btn ico" onclick={addContact}><Icon name="plus" size={16} /> Add contact</button></div>
        </section>
        <section class="dbl" id="plan" style="--c: var(--glow-blue)">
          <div class="pnl-h nosl"><Icon name="pin" size={22} /> Our plan: where we meet, who calls whom</div>
          <textarea class="input" aria-label="Our plan" bind:value={family.plan}></textarea>
        </section>
      </div>
    </div>
  </div>

  <section class="dbl" id="lock" style="--c: var(--glow-blue)">
    <div class="pnl-h nosl"><Icon name="lock" size={22} /> Passphrase lock (optional)</div>
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

  {#if saved}<p class="ok" role="status">{saved}</p>{/if}

  <dialog bind:this={sumDlg} aria-label="Medical summary">
    <div class="sumbox">
      <h2>Medical summary</h2>
      <p class="muted small">One page for each person: conditions, allergies, medicines as written above, and the doctor. Print it and keep a copy with your go-bag.</p>
      <pre class="sumtext">{summaryText()}</pre>
      <div class="row end"><button class="btn" onclick={() => sumDlg.close()}>Close</button><button class="btn primary ico" onclick={printSummary}><Icon name="save" size={16} /> Print</button></div>
    </div>
  </dialog>
{/if}

<style>
  .between { justify-content: space-between; }
  .narrow { max-width: 520px; }
  .grow { flex: 1 1 200px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 12px; margin: 8px 0; }
  .grid2 { display: grid; grid-template-columns: minmax(180px, 0.6fr) minmax(0, 1.4fr); gap: 12px; }
  @media (max-width: 760px) { .grid2 { grid-template-columns: 1fr; } }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: #cfd8e3; }
  .block { margin-top: 0; }
  .hint { font-size: 11px; }
  .fgrid { display: grid; grid-template-columns: 380px minmax(0, 1fr); gap: 14px; align-items: start; }
  @media (max-width: 1100px) { .fgrid { grid-template-columns: 1fr; } }
  .leftcol, .people { display: flex; flex-direction: column; gap: 14px; }
  .fgrid .dbl { padding: 14px 16px; }
  .ov4 { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; }
  .ot { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 8px 4px 10px; border: 1.5px solid var(--c); border-radius: 10px; background: linear-gradient(180deg, color-mix(in srgb, var(--c) 12%, #0c141e), #0a1018); box-shadow: 0 0 12px color-mix(in srgb, var(--c) 28%, transparent); color: var(--c); text-align: center; }
  .ot span { font-family: var(--mono); font-size: 9.5px; letter-spacing: .06em; text-transform: uppercase; color: #e6edf5; min-height: 24px; }
  .ot b { font-size: 26px; color: #fff; line-height: 1; }
  .qa4 { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; }
  .qt { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; min-height: 96px; padding: 8px 4px; text-align: center; border: 1.5px solid #2a4a70; border-radius: 10px; background: linear-gradient(180deg, #131f2e, #0c141e); color: #4db0ff; }
  .qt span { color: #e6edf5; font-size: 13px; line-height: 1.2; }
  .qt:hover { border-color: #4db0ff; box-shadow: 0 0 12px rgba(61, 155, 255, .35); }
  .ib { width: 34px; height: 34px; display: inline-flex; align-items: center; justify-content: center; border: 1.5px solid #4db0ff; border-radius: 8px; background: #0b1520; color: #4db0ff; cursor: pointer; }
  .ib.red { border-color: #c0432f; color: #ff7b6a; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
  @media (max-width: 900px) { .two { grid-template-columns: 1fr; } }
  .two textarea.input { min-height: 90px; }
  .mid { justify-content: center; margin-top: 8px; }
  .sumbox { padding: 18px; display: flex; flex-direction: column; gap: 10px; min-width: min(560px, 92vw); }
  .sumbox h2 { margin: 0; }
  .sumtext { margin: 0; padding: 12px; max-height: 50vh; overflow: auto; white-space: pre-wrap; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; font: 13px/1.5 var(--mono); }
  .end { justify-content: flex-end; }
  .small { font-size: 13px; }
</style>
