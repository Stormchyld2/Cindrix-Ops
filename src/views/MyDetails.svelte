<script lang="ts">
  import { onMount } from 'svelte';
  import { profileLoad, profileSave, placeSearch, emptyDetails, type MyDetails, type MyPlace } from '../lib/api';

  let d: MyDetails = $state(emptyDetails());
  let loaded = $state(false);
  let note = $state('');
  let err = $state('');
  let busy = $state('');
  let lastSaved = '';

  // Everything is kept as soon as it is typed (a moment after the last change), so nothing is lost by forgetting to press Save.
  $effect(() => {
    if (!loaded) return;
    const snap = JSON.stringify($state.snapshot(d));
    if (snap === lastSaved) return;
    const t = setTimeout(async () => {
      try {
        // an address typed without pressing Find it is looked up now, so the map can use it
        const data = JSON.parse(snap) as MyDetails;
        for (const pl of [data.home, ...data.bugout]) {
          if (pl.address.trim() && (pl.lat === null || pl.lon === null)) {
            try { const h = (await placeSearch(pl.address.trim())).places[0]; if (h) { pl.lat = h.lat; pl.lon = h.lon; } } catch { /* saved without a position */ }
          }
        }
        await profileSave(data);
        lastSaved = JSON.stringify(data);
        if (lastSaved !== snap) { d = data; }
        note = 'Saved on the drive.'; err = '';
      } catch (e) { err = String(e); }
    }, 700);
    return () => clearTimeout(t);
  });

  onMount(async () => {
    try { d = await profileLoad(); lastSaved = JSON.stringify($state.snapshot(d)); } catch (e) { err = String(e); }
    loaded = true;
  });

  async function save() {
    err = ''; note = '';
    try { const snap = JSON.stringify($state.snapshot(d)); await profileSave(JSON.parse(snap) as MyDetails); lastSaved = snap; note = 'Saved on the drive.'; } catch (e) { err = String(e); }
  }

  // Find the position of an address (or a position typed in any form) with the same finder the map uses.
  async function locate(p: MyPlace, key: string) {
    err = ''; note = '';
    const q = p.address.trim();
    if (!q) { err = 'Type an address, a town, a ZIP code or a position first.'; return; }
    busy = key;
    try {
      const r = await placeSearch(q);
      const hit = r.places[0];
      if (!hit) { err = 'That place was not found. Try a town and state, a ZIP code, or a position like 39.63, -79.96.'; return; }
      p.lat = hit.lat; p.lon = hit.lon;
      if (!p.name.trim()) p.name = hit.name;
      note = 'Found: ' + hit.name + (hit.state ? ', ' + hit.state : '') + '. Check it is the right place, then Save.';
    } catch (e) { err = String(e); } finally { busy = ''; }
  }
  const where = (p: MyPlace) => (p.lat !== null && p.lon !== null ? p.lat.toFixed(4) + ', ' + p.lon.toFixed(4) : 'no position yet');
  const num = (v: number) => Math.max(0, Math.min(99, Math.round(Number(v) || 0)));
</script>

{#snippet placeRow(p: MyPlace, key: string, nameHint: string, remove: (() => void) | null)}
  <div class="prow">
    <input class="input" placeholder={nameHint} aria-label="Name" title="A short name for this place" bind:value={p.name} />
    <input class="input wide" placeholder="Address, town, ZIP code or position" aria-label="Address" title="An address, a town, a ZIP code, or a position in any form" bind:value={p.address}
      oninput={() => { p.lat = null; p.lon = null; }} onkeydown={(e) => { if (e.key === 'Enter') void locate(p, key); }} />
    <button class="btn" disabled={busy === key} title="Look the place up on the map files and keep its position" onclick={() => void locate(p, key)}>{busy === key ? 'Looking...' : 'Find it'}</button>
    {#if remove}<button class="btn" title="Take this place out of the list" onclick={remove}>Remove</button>{/if}
  </div>
  <p class="muted small">{where(p)}</p>
{/snippet}

{#if !loaded}
  <div class="panel"><p class="muted">Loading...</p></div>
{:else}
  <div class="panel">
    <div class="label">ABOUT THIS PAGE</div>
    <p class="muted small">Everything here is optional and is saved as you type. It is kept in one plain file on the drive (user/profile.txt), is never sent anywhere, and is not shown to guests on the sharing server. The map's Home and Bug-out buttons, the stock plan and the assistant can use it. Leave anything blank that you would rather not say.</p>
  </div>

  <div class="panel">
    <div class="label">HOME</div>
    {@render placeRow(d.home, 'home', 'Home', null)}
  </div>

  <div class="panel">
    <div class="label">PLACES TO BUG OUT TO</div>
    {#each d.bugout as b, i}
      {@render placeRow(b, 'b' + i, 'Name, such as the cabin', () => (d.bugout = d.bugout.filter((_, j) => j !== i)))}
    {/each}
    <button class="btn" title="Add another place" onclick={() => (d.bugout = [...d.bugout, { name: '', address: '', lat: null, lon: null }])}>Add a place</button>
  </div>

  <div class="panel">
    <div class="label">WHO LIVES HERE</div>
    <div class="nums">
      <label>Adults <input class="input num" type="number" min="0" max="99" bind:value={d.adults} onchange={() => (d.adults = num(d.adults))} /></label>
      <label>Children <input class="input num" type="number" min="0" max="99" bind:value={d.children} onchange={() => (d.children = num(d.children))} /></label>
      <label>Babies <input class="input num" type="number" min="0" max="99" bind:value={d.babies} onchange={() => (d.babies = num(d.babies))} /></label>
      <label>Small pets <input class="input num" type="number" min="0" max="99" bind:value={d.small_pets} onchange={() => (d.small_pets = num(d.small_pets))} /></label>
      <label>Large pets <input class="input num" type="number" min="0" max="99" bind:value={d.large_pets} onchange={() => (d.large_pets = num(d.large_pets))} /></label>
    </div>
    <p class="muted small">The names of the people are on the Family page. These numbers help work out how much to keep in stock.</p>
  </div>

  <div class="panel">
    <div class="label">PEOPLE TO REACH AND PLACES TO MEET</div>
    {#each d.contacts as c, i}
      <div class="prow">
        <input class="input" placeholder="Name" aria-label="Contact name" title="Who this is" bind:value={c.name} />
        <input class="input" placeholder="Phone, radio channel or where to find them" aria-label="How to reach" title="How to reach them" bind:value={c.how} />
        <input class="input wide" placeholder="Notes, such as meeting place or out-of-area contact" aria-label="Notes" title="Anything useful about them" bind:value={c.notes} />
        <button class="btn" title="Take this person out of the list" onclick={() => (d.contacts = d.contacts.filter((_, j) => j !== i))}>Remove</button>
      </div>
    {/each}
    <button class="btn" title="Add a person or a meeting point" onclick={() => (d.contacts = [...d.contacts, { name: '', how: '', notes: '' }])}>Add</button>
  </div>

  <div class="panel">
    <div class="label">NEEDS, SKILLS AND NOTES</div>
    <label class="area">Needs <span class="muted small">(walking limits, equipment to carry, vehicle and how far it goes on a tank; no medicine doses)</span>
      <textarea class="input" rows="3" title="Things that change what a plan has to allow for" bind:value={d.needs}></textarea></label>
    <label class="area">Skills <span class="muted small">(first aid, trades, languages, radio licence)</span>
      <textarea class="input" rows="3" title="What the household can do" bind:value={d.skills}></textarea></label>
    <label class="area">Anything else
      <textarea class="input" rows="3" title="Any other notes worth having" bind:value={d.notes}></textarea></label>
  </div>

  <div class="panel">
    <button class="btn" title="Keep these details on the drive" onclick={() => void save()}>Save my details</button>
    {#if note}<p class="small" role="status">{note}</p>{/if}
    {#if err}<p class="small" role="alert">{err}</p>{/if}
  </div>
{/if}

<style>
  .prow { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-bottom: 4px; }
  .prow .input { min-width: 140px; flex: 1 1 160px; }
  .prow .wide { flex: 3 1 280px; }
  .nums { display: flex; flex-wrap: wrap; gap: 14px; }
  .nums label { display: flex; flex-direction: column; gap: 4px; font-size: 0.85rem; }
  .num { width: 80px; flex: none; min-width: 0; }
  .area { display: flex; flex-direction: column; gap: 4px; margin-bottom: 10px; }
</style>
