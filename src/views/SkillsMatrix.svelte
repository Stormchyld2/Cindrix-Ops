<script lang="ts">
  import { onMount } from 'svelte';
  import { skillsGrid, skillsSignoff, type SkillsGrid } from '../lib/api';

  let { people }: { people: string[] } = $props();

  let grid: SkillsGrid | null = $state(null);
  let error = $state('');
  let signer = $state('');
  let only = $state<'all' | 'must' | 'weak'>('must');

  async function load() {
    try {
      grid = await skillsGrid(people);
      error = '';
    } catch (e) {
      error = 'The skills could not be read: ' + String(e);
    }
  }
  onMount(load);

  const rows = $derived((grid?.rows ?? []).filter((r) => only === 'all' || (only === 'must' ? r.must_have : r.flags.length > 0)));
  const areas = $derived([...new Set(rows.map((r) => r.area))]);
  const mark: Record<string, string> = { signed: '●', learning: '◐', none: '○' };
  const word: Record<string, string> = { signed: 'Can do this', learning: 'Learning', none: 'Not started' };

  async function toggle(person: string, skill: string, state: string) {
    try {
      if (state !== 'signed' && !signer.trim()) { error = 'Write who is signing this off, above the grid.'; return; }
      await skillsSignoff(person, skill, signer, state !== 'signed');
      await load();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<section class="panel" aria-label="Skills of the whole group">
  <div class="label">SKILLS OF THE GROUP</div>
  {#if grid}
    <p class="lead"><strong>{grid.must_covered} of {grid.must_total}</strong> must-have skills are covered by {grid.bar === 1 ? 'at least one person' : `${grid.bar} or more people`}.</p>
    <p class="muted small">Click a box to sign a skill off for that person, or take the sign-off back. A skill backed by a course fills in by itself when the course is signed off or finished. A drill that has not been done this year is flagged.</p>
    <div class="row ctl">
      <label class="small">Signed off by <input class="input" bind:value={signer} placeholder="Your name" aria-label="Who is signing off" /></label>
      <div class="seg" role="group" aria-label="Which skills to show">
        <button class="btn sm" class:primary={only === 'must'} onclick={() => (only = 'must')}>Must-have</button>
        <button class="btn sm" class:primary={only === 'weak'} onclick={() => (only = 'weak')}>Needs a look</button>
        <button class="btn sm" class:primary={only === 'all'} onclick={() => (only = 'all')}>All</button>
      </div>
    </div>
    {#if error}<div class="warn" role="alert">{error}</div>{/if}
    <div class="scroll">
      <table>
        <thead><tr><th>Skill</th>{#each grid.people as p}<th class="pc">{p}</th>{/each}<th>Look at</th></tr></thead>
        <tbody>
          {#each areas as a}
            <tr class="area"><td colspan={grid.people.length + 2}>{a}</td></tr>
            {#each rows.filter((r) => r.area === a) as r (r.skill)}
              <tr>
                <td title={r.proof}>{r.skill}{r.must_have ? '' : ' (extra)'}</td>
                {#each r.cells as c, i}
                  <td class="pc"><button class="cell {c}" onclick={() => toggle(grid!.people[i], r.skill, c)} title={`${grid!.people[i]}: ${word[c]}. Click to ${c === 'signed' ? 'take the sign-off back' : 'sign off'}.`} aria-label={`${grid!.people[i]}, ${r.skill}: ${word[c]}`}>{mark[c]}</button></td>
                {/each}
                <td class="flags">{#each r.flags as f}<span class="flag">{f}</span>{/each}</td>
              </tr>
            {/each}
          {/each}
        </tbody>
      </table>
    </div>
    {#if rows.length === 0}<p class="muted">Nothing to show here.</p>{/if}
    <p class="muted small">● can do this &nbsp; ◐ learning &nbsp; ○ not started. Add your own skills in your user folder (skills.csv).</p>
  {:else if error}
    <div class="warn" role="alert">{error}</div>
  {:else}
    <p class="muted">Reading the skills…</p>
  {/if}
</section>

<style>
  .lead { font-size: 18px; margin: 6px 0; }
  .ctl { gap: 12px; align-items: center; flex-wrap: wrap; margin: 8px 0; }
  .scroll { overflow-x: auto; }
  table { border-collapse: collapse; width: 100%; font-size: 14px; }
  th, td { padding: 6px 8px; border-top: 1px solid var(--line, #333); text-align: left; }
  th.pc, td.pc { text-align: center; }
  tr.area td { background: var(--blue-soft, #1a2535); font-family: var(--mono); font-size: 11px; letter-spacing: .08em; text-transform: uppercase; color: var(--muted); }
  .cell { background: none; border: 0; font-size: 20px; cursor: pointer; color: var(--muted); }
  .cell.signed { color: #4ade80; }
  .cell.learning { color: #fbbf24; }
  .flag { display: inline-block; margin: 2px 4px 2px 0; padding: 1px 8px; border-radius: 999px; border: 1px solid #b45309; color: #fbbf24; font-size: 12px; }
</style>
