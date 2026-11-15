<script lang="ts">
  // Settings > PACE plan: plain questions with an answer box each. The structured plan is built from these on the Comms plan page.
  import { onMount } from 'svelte';
  import { paceLoad, paceSave, type Pace } from '../lib/api';

  let { go }: { go?: (where: string) => void } = $props();
  let pace: Pace | null = $state(null);
  let answers: Record<string, string> = $state({});
  let note = $state('');
  let err = $state('');
  let saved = '';
  const dirty = $derived(JSON.stringify($state.snapshot(answers)) !== saved);

  function read(p: Pace) {
    pace = p;
    answers = Object.fromEntries(p.answers);
    saved = JSON.stringify($state.snapshot(answers));
  }
  onMount(async () => { try { read(await paceLoad()); } catch (e) { err = String(e); } });

  async function save() {
    err = ''; note = '';
    try {
      read(await paceSave(Object.entries($state.snapshot(answers)) as [string, string][]));
      note = 'Saved on the drive. Your PACE plan on the Comms plan page is up to date.';
    } catch (e) { err = String(e); }
  }

  const groups = $derived.by(() => {
    if (!pace) return [];
    const out: { code: string; name: string; when: string; qs: Pace['questions'] }[] = [];
    for (const q of pace.questions) {
      let g = out.find((x) => x.code === q.tier);
      if (!g) { g = { code: q.tier, name: q.tier_name, when: q.tier_when, qs: [] }; out.push(g); }
      g.qs.push(q);
    }
    return out;
  });
</script>

<section>
  <h2>PACE plan</h2>
  <p class="muted">PACE is four ways to reach each other, tried in order: Primary, Alternate, Contingency, Emergency. Answer what you can. Leave a box empty if you do not know yet. Your answers become a plan on the Comms plan page.</p>
  {#if err}<p class="notice" role="alert">{err}</p>{/if}
  {#if note}<p class="notice" role="status">{note}</p>{/if}
  {#each groups as g (g.code)}
    <div class="panel grp">
      <h3>{g.name}</h3>
      <p class="muted small">{g.when}</p>
      {#each g.qs as q (q.key)}
        <label class="q">
          <span class="p">{q.prompt}</span>
          <span class="muted small">{q.hint}</span>
          <textarea class="input" rows="3" maxlength="1000" placeholder={'For example: ' + q.example} bind:value={answers[q.key]}></textarea>
        </label>
      {/each}
    </div>
  {/each}
  <div class="row">
    <button class="btn primary" onclick={save} disabled={!dirty} title="Keep these answers and rebuild the PACE plan">Save my answers</button>
    <button class="btn" onclick={() => go?.('CommsPlan')} title="See the PACE plan made from your answers">Open the PACE plan</button>
    <span class="muted small">{dirty ? 'Not saved yet.' : ''}</span>
  </div>
</section>

<style>
  .grp { margin-top: 12px; display: flex; flex-direction: column; gap: 10px; border-left: 3px solid var(--cat-comms); }
  .grp h3 { margin: 0; }
  .q { display: flex; flex-direction: column; gap: 4px; }
  .p { font-weight: 600; }
  textarea { width: 100%; resize: vertical; font-family: inherit; }
  .small { font-size: 13px; }
  .row { margin-top: 12px; display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
</style>
