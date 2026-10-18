<script lang="ts">
  import { onMount } from 'svelte';
  import { familyStatus, familyLoad, inventoryList, listCards, drillsList, type Family, type Item, type Card, type Drill } from '../lib/api';

  let family = $state<Family | null>(null);
  let locked = $state(false);
  let pass = $state('');
  let passError = $state('');
  let items: Item[] = $state([]);
  let cards: Card[] = $state([]);
  let drills: Drill[] = $state([]);
  let error = $state('');

  let incFamily = $state(true);
  let incContacts = $state(true);
  let incSupplies = $state(true);
  let pickedCards: Record<string, boolean> = $state({});
  let pickedDrills: Record<string, boolean> = $state({});
  const today = new Date().toISOString().slice(0, 10);

  onMount(async () => {
    try {
      const st = await familyStatus();
      locked = st.locked;
      if (st.exists && !st.locked) family = await familyLoad();
      items = await inventoryList();
      cards = await listCards();
      drills = await drillsList();
      for (const c of cards) pickedCards[c.id] = true;
      for (const d of drills) pickedDrills[d.id] = false;
      if (drills[0]) pickedDrills[drills[0].id] = true;
    } catch (e) {
      error = String(e);
    }
  });

  async function unlock() {
    passError = '';
    try {
      family = await familyLoad(pass);
      locked = false;
      pass = '';
    } catch {
      passError = 'Wrong passphrase, or the file is damaged.';
    }
  }

  let chosenCards = $derived(cards.filter((c) => pickedCards[c.id]));
  let chosenDrills = $derived(drills.filter((d) => pickedDrills[d.id]));
  let anyDraft = $derived(chosenCards.some((c) => c.status !== 'reviewed') || chosenDrills.some((d) => d.status !== 'reviewed'));
  let people = $derived((family?.people ?? []).filter((p) => p.name.trim() || p.conditions.trim() || p.medicines.trim() || p.allergies.trim() || p.doctor.trim()));

  // Plain rendering of a card's Markdown (bold, numbered and bullet lines) without any HTML injection.
  function lines(body: string): { text: string; list: boolean }[] {
    return body.split('\n').filter((l) => l.trim()).map((l) => ({ text: l.replace(/\*\*/g, '').replace(/^\s*(\d+\.|-)\s+/, (m) => (m.includes('.') ? m.trim() + ' ' : '• ')), list: /^\s*(\d+\.|-)\s+/.test(l) }));
  }
</script>

<section class="wrap">
  <div class="no-print">
    <h1>Binder</h1>
    <p class="sub">A paper copy of your plan, for when the screens are dark. Choose what goes in, check the preview, then print it or save it as a PDF.</p>
  </div>
  {#if error}<div class="warn no-print">{error}</div>{/if}

  <div class="layout">
    <div class="panel controls no-print">
      <div class="label">INCLUDE</div>
      <label class="opt"><input type="checkbox" bind:checked={incFamily} /> Family information (medical details)</label>
      <label class="opt"><input type="checkbox" bind:checked={incContacts} /> Contacts and meeting points</label>
      <label class="opt"><input type="checkbox" bind:checked={incSupplies} /> Supply list ({items.length} items)</label>
      <label class="opt off"><input type="checkbox" disabled /> Saved maps <span class="muted">(arrive with Maps)</span></label>

      {#if locked}
        <div class="lock">
          <p class="muted">The Family page is locked with your passphrase. Enter it to include it.</p>
          <div class="row">
            <input class="input" type="password" placeholder="Passphrase" bind:value={pass} onkeydown={(e) => e.key === 'Enter' && unlock()} />
            <button class="btn" onclick={unlock}>Unlock</button>
          </div>
          {#if passError}<p class="bad">{passError}</p>{/if}
        </div>
      {/if}

      <div class="label sp">EMERGENCY CARDS</div>
      {#each cards as c (c.id)}
        <label class="opt"><input type="checkbox" bind:checked={pickedCards[c.id]} /> {c.title}{c.status !== 'reviewed' ? ' (draft)' : ''}</label>
      {/each}

      <div class="label sp">DRILL CHECKLISTS</div>
      {#each drills as d (d.id)}
        <label class="opt"><input type="checkbox" bind:checked={pickedDrills[d.id]} /> {d.title}</label>
      {/each}

      <div class="row sp">
        <button class="btn primary" onclick={() => window.print()}>Print</button>
        <button class="btn" onclick={() => window.print()}>Save as PDF</button>
      </div>
      <p class="muted fine">In the print window, choose your printer, or “Save as PDF”. The binder is black and white friendly. It holds medical details: keep the paper somewhere private and safe.</p>
    </div>

    <article class="sheet" aria-label="Binder preview">
      <header class="sh">
        <h1>FAMILY EMERGENCY BINDER</h1>
        <div>Updated {today}</div>
      </header>
      <p class="rule">In an emergency, call 911. This binder is reference only, not medical advice.{anyDraft ? ' Cards and checklists marked DRAFT have not been reviewed by a clinician: check them against your doctor’s written plans.' : ''}</p>

      {#if incFamily}
        <section>
          <h2>Family information</h2>
          {#if locked}
            <p class="muted">Locked. Unlock it on the left to include it.</p>
          {:else if people.length === 0}
            <p class="muted">Nothing entered yet. Fill in the Family page first.</p>
          {:else}
            {#each people as p}
              <div class="person">
                <h3>{p.name || '(no name)'}</h3>
                <table>
                  <tbody>
                    {#if p.conditions}<tr><th>Conditions</th><td>{p.conditions}</td></tr>{/if}
                    {#if p.medicines}<tr><th>Medicines (as written by the doctor)</th><td>{p.medicines}</td></tr>{/if}
                    {#if p.allergies}<tr><th>Allergies</th><td>{p.allergies}</td></tr>{/if}
                    {#if p.doctor}<tr><th>Doctor</th><td>{p.doctor}</td></tr>{/if}
                  </tbody>
                </table>
              </div>
            {/each}
          {/if}
        </section>
      {/if}

      {#if incContacts && family}
        <section>
          <h2>Contacts and meeting points</h2>
          {#if family.contacts.trim()}<pre>{family.contacts}</pre>{/if}
          {#if family.plan.trim()}<h3>Our plan</h3><pre>{family.plan}</pre>{/if}
          {#if !family.contacts.trim() && !family.plan.trim()}<p class="muted">Nothing entered yet.</p>{/if}
        </section>
      {/if}

      {#if incSupplies}
        <section>
          <h2>Supply list</h2>
          {#if items.length === 0}
            <p class="muted">No supplies entered yet.</p>
          {:else}
            <table class="grid">
              <thead><tr><th>Item</th><th>Qty</th><th>Where</th><th>Use by</th></tr></thead>
              <tbody>
                {#each items as i}
                  <tr><td>{i.name}</td><td>{i.quantity} {i.unit}</td><td>{i.location}</td><td>{i.use_by}</td></tr>
                {/each}
              </tbody>
            </table>
          {/if}
        </section>
      {/if}

      {#each chosenCards as c (c.id)}
        <section class="card">
          <h2>{c.title}{c.status !== 'reviewed' ? ' (DRAFT)' : ''}</h2>
          {#each lines(c.body) as l}<p class:li={l.list}>{l.text}</p>{/each}
        </section>
      {/each}

      {#each chosenDrills as d (d.id)}
        <section class="card">
          <h2>Checklist: {d.title}{d.status !== 'reviewed' ? ' (DRAFT)' : ''}</h2>
          <p class="muted">{d.summary}</p>
          <ul class="boxes">{#each d.steps as s}<li><span class="box"></span> {s}</li>{/each}</ul>
        </section>
      {/each}
    </article>
  </div>
</section>

<style>
  .wrap { display: flex; flex-direction: column; gap: 14px; }
  .layout { display: grid; grid-template-columns: minmax(250px, 320px) 1fr; gap: 16px; align-items: start; }
  @media (max-width: 860px) { .layout { grid-template-columns: 1fr; } }
  .controls { display: flex; flex-direction: column; gap: 8px; }
  .opt { display: flex; gap: 10px; align-items: flex-start; font-size: 14px; }
  .opt.off { opacity: .55; }
  .sp { margin-top: 12px; }
  .fine { font-size: 12px; }
  .bad { color: #f08a7a; margin: 6px 0 0; }
  .lock { padding: 8px 0; }

  /* The paper */
  .sheet { background: #fff; color: #111; border-radius: 6px; padding: 28px 32px; font-family: Georgia, 'Times New Roman', serif; line-height: 1.45; }
  .sheet h1 { font-family: inherit; font-size: 22px; letter-spacing: .08em; margin: 0; color: #111; }
  .sheet h2 { font-size: 17px; margin: 22px 0 6px; border-bottom: 2px solid #111; padding-bottom: 2px; color: #111; font-family: inherit; }
  .sheet h3 { font-size: 15px; margin: 10px 0 4px; color: #111; }
  .sh { display: flex; justify-content: space-between; align-items: baseline; border-bottom: 3px solid #111; padding-bottom: 6px; }
  .rule { font-size: 13px; border: 1px solid #111; padding: 6px 10px; margin: 10px 0; }
  .sheet p { margin: 4px 0; color: #111; }
  .sheet .muted { color: #555; }
  .sheet pre { white-space: pre-wrap; font: inherit; margin: 4px 0; }
  .sheet table { border-collapse: collapse; width: 100%; font-size: 14px; }
  .sheet th, .sheet td { text-align: left; vertical-align: top; padding: 3px 8px 3px 0; color: #111; }
  .sheet table.grid th, .sheet table.grid td { border-bottom: 1px solid #999; }
  .sheet th { width: 34%; font-weight: 600; }
  .sheet table.grid th { width: auto; }
  .li { padding-left: 14px; text-indent: -14px; }
  .boxes { list-style: none; padding: 0; margin: 6px 0; }
  .boxes li { margin: 5px 0; display: flex; gap: 10px; align-items: flex-start; }
  .box { width: 14px; height: 14px; border: 2px solid #111; flex: none; margin-top: 3px; }
  .person { break-inside: avoid; }
  .card { break-before: page; }
</style>
