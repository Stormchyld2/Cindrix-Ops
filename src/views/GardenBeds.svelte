<script lang="ts">
  import { onMount } from 'svelte';
  import { bedsList, bedsSave, type BedRow, type BedReview } from '../lib/api';

  let review: BedReview = $state({ rows: [], clashes: [], hints: [] });
  let error = $state('');
  let saved = $state('');
  let bed = $state('');
  let crop = $state('');
  let year = $state(new Date().getFullYear());
  let notes = $state('');

  onMount(async () => {
    try { review = await bedsList(); } catch (e) { error = String(e); }
  });

  const beds = $derived([...new Set(review.rows.map((r) => r.bed))].sort((a, b) => a.localeCompare(b, undefined, { numeric: true })));
  const years = $derived([...new Set(review.rows.map((r) => r.year))].sort((a, b) => b - a));
  const clashKey = $derived(new Set(review.clashes.map((c) => `${c.bed.toLowerCase()}|${c.year}|${c.crop.toLowerCase()}`)));
  const isClash = (r: BedRow) => clashKey.has(`${r.bed.toLowerCase()}|${r.year}|${r.crop.toLowerCase()}`);
  const cell = (b: string, y: number) => review.rows.filter((r) => r.bed.toLowerCase() === b.toLowerCase() && r.year === y);

  async function persist(rows: BedRow[], msg: string) {
    try {
      review = await bedsSave(rows);
      error = '';
      saved = msg;
    } catch (e) { error = String(e); }
  }
  async function add() {
    if (!bed.trim() || !crop.trim()) return;
    const y = Math.round(Number(year)) || new Date().getFullYear();
    const row: BedRow = { bed: bed.trim(), year: y, crop: crop.trim(), notes: notes.trim() };
    await persist([...review.rows, row], `Saved ${row.crop} in ${row.bed}, ${row.year}.`);
    crop = '';
    notes = '';
  }
  async function remove(r: BedRow) {
    await persist(review.rows.filter((x) => x !== r), `Removed ${r.crop} from ${r.bed}, ${r.year}.`);
  }
</script>

<section class="head row">
  <div class="grow">
    <h1>Garden beds</h1>
    <p class="sub">Write down what grew in each bed each year. The page warns you when a plant family would come back to a bed too soon, and suggests what could go in next.</p>
  </div>
</section>

{#if error}<div class="warn" role="alert">{error}</div>{/if}

<section class="panel">
  <h2>Add a planting</h2>
  <form class="row" onsubmit={(e) => { e.preventDefault(); add(); }}>
    <label>Bed <input list="bed-names" bind:value={bed} placeholder="Bed 1" required /></label>
    <datalist id="bed-names">{#each beds as b}<option value={b}></option>{/each}</datalist>
    <label>Year <input type="number" bind:value={year} min="1900" max="2200" style="width:6em" /></label>
    <label>Crop <input bind:value={crop} placeholder="Tomato" required /></label>
    <label class="grow">Notes <input bind:value={notes} placeholder="Optional" /></label>
    <button class="btn primary" type="submit">Save</button>
  </form>
  {#if saved}<p class="muted small" role="status">{saved}</p>{/if}
</section>

{#if review.clashes.length}
  <section class="panel">
    <h2>Rotation warnings</h2>
    <ul>
      {#each review.clashes as c}
        <li class="warn-text"><strong>{c.bed}, {c.year}:</strong> {c.crop} is in the same family as {c.earlier_crop} ({c.earlier_year}), a {c.family}. Better to wait until {c.wait_until}.</li>
      {/each}
    </ul>
  </section>
{/if}

<section class="panel">
  <h2>What grew where</h2>
  {#if !review.rows.length}
    <p class="muted">No beds yet. Add the first planting above. Use the same bed name each year so the page can compare them.</p>
  {:else}
    <div class="tablewrap">
      <table>
        <thead><tr><th scope="col">Bed</th>{#each years as y}<th scope="col">{y}</th>{/each}</tr></thead>
        <tbody>
          {#each beds as b}
            <tr>
              <th scope="row">{b}</th>
              {#each years as y}
                <td>
                  {#each cell(b, y) as r}
                    <span class="badge" class:warn={isClash(r)} title={r.notes}>{r.crop}
                      <button class="linklike" aria-label={`Remove ${r.crop} from ${r.bed}, ${r.year}`} onclick={() => remove(r)}>×</button>
                    </span>
                  {/each}
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

{#if review.hints.length}
  <section class="panel">
    <h2>What could go in next</h2>
    <p class="muted small">Families that are not still resting in that bed, best first. A rule of thumb, not a guarantee.</p>
    <ul>
      {#each review.hints as h}
        <li><strong>{h.bed}:</strong> {h.next.length ? h.next.join(', then ') : 'every family is still resting here; pick whichever clashes least'}</li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .tablewrap { overflow-x: auto; }
  table { border-collapse: collapse; width: 100%; }
  th, td { text-align: left; padding: 0.4rem 0.6rem; border-bottom: 1px solid var(--line, #3a3f47); vertical-align: top; }
  .badge { display: inline-block; margin: 0 0.3rem 0.2rem 0; }
  .linklike { background: none; border: 0; color: inherit; cursor: pointer; padding: 0 0.2rem; }
  .warn-text { color: var(--warn); }
</style>
