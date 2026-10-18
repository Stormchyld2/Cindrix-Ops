<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { drillsList, drillSave, type Drill } from '../lib/api';

  let drills: Drill[] = $state([]);
  let error = $state('');
  let saved = $state('');

  // The drill being walked through
  let active: Drill | null = $state(null);
  let checked: boolean[] = $state([]);
  let seconds = $state(0);
  let paused = $state(false);
  let notes = $state('');
  let tick: ReturnType<typeof setInterval> | undefined;

  async function load() {
    try {
      drills = await drillsList();
    } catch (e) {
      error = String(e);
    }
  }
  onMount(load);
  onDestroy(() => { if (tick) clearInterval(tick); });

  const mmss = (s: number) => `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
  let doneCount = $derived(checked.filter(Boolean).length);
  let nextStep = $derived(checked.indexOf(false) === -1 ? checked.length : checked.indexOf(false) + 1);

  function start(d: Drill) {
    active = d;
    checked = d.steps.map(() => false);
    seconds = 0;
    paused = false;
    notes = '';
    saved = '';
    if (tick) clearInterval(tick);
    tick = setInterval(() => { if (!paused) seconds++; }, 1000);
  }

  function stop() {
    if (tick) clearInterval(tick);
    active = null;
  }

  async function finish() {
    if (!active) return;
    try {
      await drillSave(active.id, seconds, doneCount, checked.length, notes);
      saved = `Saved: ${active.title}, ${doneCount} of ${checked.length} steps in ${mmss(seconds)}.`;
      stop();
      await load();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<section class="wrap">
  {#if !active}
    <div>
      <h1>Drills</h1>
      <p class="sub">Practice before it is real. Each drill is a short checklist you walk through, with a timer and a debrief. Your results stay on this drive.</p>
    </div>
    {#if error}<div class="warn">{error}</div>{/if}
    {#if saved}<div class="done">{saved}</div>{/if}
    <div class="grid">
      {#each drills as d (d.id)}
        <article class="panel drill">
          <div class="meta">
            {#if d.times_done > 0}<span class="label">DONE {d.times_done}×</span>{:else}<span class="label">NOT DONE YET</span>{/if}
            {#if d.status !== 'reviewed'}<span class="badge">draft</span>{/if}
          </div>
          <h2>{d.title}</h2>
          <p>{d.summary}</p>
          <p class="muted fine">{d.minutes} min · {d.place} · {d.steps.length} steps</p>
          {#if d.last}
            <p class="fine">Last: {d.last.date}, {d.last.done} of {d.last.total} steps in {mmss(d.last.seconds)}.{d.last.notes ? ' Missing: ' + d.last.notes : ''}</p>
          {/if}
          <div><button class="btn primary" onclick={() => start(d)}>{d.times_done > 0 ? 'Do it again' : 'Start drill'}</button></div>
        </article>
      {/each}
    </div>
    {#if drills.length === 0 && !error}<p class="muted">No drills found in the content folder.</p>{/if}
  {:else}
    <div class="panel run">
      <div class="head">
        <div>
          <div class="label">{active.title.toUpperCase()} · STEP {nextStep} OF {checked.length}</div>
          <div class="timer" aria-live="off">{mmss(seconds)}</div>
        </div>
        <div class="row">
          <button class="btn" onclick={() => (paused = !paused)}>{paused ? 'Resume' : 'Pause'}</button>
        </div>
      </div>
      {#if active.status !== 'reviewed'}<div class="warn">Draft checklist. Check it against your own situation and your doctor's written plans.</div>{/if}
      <ul class="steps">
        {#each active.steps as s, i}
          <li>
            <label class:ticked={checked[i]}>
              <input type="checkbox" bind:checked={checked[i]} />
              <span>{s}</span>
            </label>
          </li>
        {/each}
      </ul>
      <label class="debrief">Debrief: what was missing?
        <textarea class="input" rows="3" bind:value={notes} placeholder="Batteries for the lantern, a can opener, more water…"></textarea>
      </label>
      {#if error}<div class="warn">{error}</div>{/if}
      <div class="row end">
        <button class="btn" onclick={stop}>End drill without saving</button>
        <button class="btn primary" onclick={finish}>Save result ({doneCount} of {checked.length})</button>
      </div>
    </div>
  {/if}
</section>

<style>
  .wrap { display: flex; flex-direction: column; gap: 14px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 14px; }
  .drill { display: flex; flex-direction: column; gap: 6px; }
  .drill h2 { margin: 0; font-size: 18px; }
  .drill p { margin: 0; }
  .meta { display: flex; justify-content: space-between; align-items: center; }
  .fine { font-size: 12px; }
  .done { padding: 10px 12px; border: 1px solid var(--line2); border-radius: 8px; background: var(--panel2); }
  .head { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 10px; }
  .timer { font-family: var(--mono); font-size: 34px; color: var(--blue); }
  .steps { list-style: none; padding: 0; margin: 12px 0; display: flex; flex-direction: column; gap: 4px; }
  .steps label { display: flex; gap: 12px; align-items: flex-start; padding: 12px; border: 1px solid var(--line); border-radius: 8px; cursor: pointer; }
  .steps label.ticked { opacity: .6; text-decoration: line-through; }
  .steps input { width: 20px; height: 20px; margin-top: 1px; flex: none; }
  .debrief { display: flex; flex-direction: column; gap: 6px; margin-bottom: 12px; }
  .end { justify-content: flex-end; }
</style>
