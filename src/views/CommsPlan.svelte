<script lang="ts">
  // The family's radio and phone plan: free text that starts from a template, kept in the person's own folder.
  import { onMount } from 'svelte';
  import { commsPlanLoad, commsPlanSave } from '../lib/api';
  import { who } from '../lib/person.svelte';
  import PersonPick from '../lib/PersonPick.svelte';

  let text = $state('');
  let saved = $state('');
  let note = $state('');
  const dirty = $derived(text !== saved);

  async function load() {
    note = '';
    try { text = await commsPlanLoad(who.name); saved = text; } catch (e) { note = String(e); }
  }
  async function save() {
    note = '';
    try { await commsPlanSave(who.name, text); saved = text; note = 'Saved.'; } catch (e) { note = String(e); }
  }
  onMount(load);
</script>

<section>
  <h1>Comms plan</h1>
  <p class="sub">Decide before you need it: who carries a radio, which channel, when you check in and where you meet. Fill it in, save it, and print a copy for everyone. Other devices on your hotspot cannot see it. For the rules and the words to use, see Radio reference in the Comms menu.</p>
  <div class="row"><PersonPick onchange={load} /></div>
  {#if note}<p class="notice" role="status">{note}</p>{/if}
  <div class="panel plan">
    <textarea class="input" rows="20" bind:value={text} aria-label="The comms plan"></textarea>
    <div class="row">
      <button class="btn primary" onclick={save} disabled={!dirty}>Save</button>
      <button class="btn" onclick={() => window.print()} title="Print this plan on paper">Print</button>
      <span class="muted small">{dirty ? 'Not saved yet.' : ''}</span>
    </div>
  </div>
</section>

<style>
  .plan { margin-top: 12px; display: flex; flex-direction: column; gap: 10px; max-width: 820px; border-left: 3px solid var(--cat-comms); }
  textarea { width: 100%; resize: vertical; font-family: inherit; line-height: 1.55; }
  .small { font-size: 13px; }
  .notice { margin: 8px 0 0; }
</style>
