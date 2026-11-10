<script lang="ts">
  // The family's radio and phone plan: free text that starts from a full template, kept in the person's own folder.
  import { onMount } from 'svelte';
  import { commsPlanLoad, commsPlanSave, commsPlanAddMissing, isLocked, paceLoad, type Pace } from '../lib/api';
  import PrivateLock from '../lib/PrivateLock.svelte';
  import { who } from '../lib/person.svelte';
  import PersonPick from '../lib/PersonPick.svelte';

  let { go }: { go?: (where: string) => void } = $props();
  let pace: Pace | null = $state(null);
  let text = $state('');
  let saved = $state('');
  let note = $state('');
  let locked = $state(false);
  let ta: HTMLTextAreaElement | undefined = $state();
  const dirty = $derived(text !== saved);
  const sections = $derived(text.split('\n').map((l, i) => ({ l, i })).filter((x) => x.l.startsWith('## ')).map((x) => ({ title: x.l.slice(3).trim(), line: x.i })));

  async function load() {
    note = '';
    locked = false;
    try { text = await commsPlanLoad(who.name); saved = text; } catch (e) { if (isLocked(e)) { locked = true; text = ''; saved = ''; } else note = String(e); }
  }
  async function save() {
    note = '';
    try { await commsPlanSave(who.name, text); saved = text; note = 'Saved.'; } catch (e) { if (isLocked(e)) locked = true; else note = String(e); }
  }
  async function addMissing() {
    note = '';
    try {
      const next = await commsPlanAddMissing(text);
      note = next === text ? 'Your plan already has every section.' : 'The missing sections were added at the end. Look them over, then press Save.';
      text = next;
    } catch (e) { note = String(e); }
  }
  function jump(line: number) {
    if (!ta) return;
    const lh = parseFloat(getComputedStyle(ta).lineHeight) || 22;
    ta.scrollTop = Math.max(0, line * lh - 8);
    let pos = 0;
    const lines = text.split('\n');
    for (let i = 0; i < line; i++) pos += lines[i].length + 1;
    ta.focus();
    ta.setSelectionRange(pos, pos);
  }
  onMount(() => { load(); paceLoad().then((p) => (pace = p)).catch(() => {}); });
</script>

<section>
  <h1>Comms plan</h1>
  <p class="sub">A complete plan for when phones and the internet are down: who does what, the order of ways to reach each other (PACE), channels, check-in times, meeting places, what to say, and what to do when the radios fail. Fill in every blank, save it, and print a copy for everyone. Other devices on your hotspot cannot see it, and you can add a passcode so only you can read it. For the rules and the words to use, see Radio reference in the Comms menu.</p>
  {#if pace}
    <div class="panel pace">
      <div class="row head">
        <h2>PACE plan</h2>
        <button class="btn" onclick={() => go?.('settings:PACE plan')} title="Answer or change the questions this plan is built from">{pace.plan.empty ? 'Answer the questions' : 'Change my answers'}</button>
        {#if !pace.plan.empty}<button class="btn" onclick={() => window.print()} title="Print this plan on paper">Print</button>{/if}
      </div>
      {#if pace.plan.empty}
        <p class="muted">Your PACE plan is not made yet. It is built from your answers to a few plain questions, such as who is in the group, which radios and channels you use, and where you meet. <button class="link" onclick={() => go?.('settings:PACE plan')}>Go to Settings > PACE plan to answer them</button>.</p>
      {:else}
        <p class="muted small">{pace.plan.answered} of {pace.plan.total} questions answered. Try each level in order; move to the next one when the one before it does not work.</p>
        <div class="levels">
          {#each pace.plan.levels as l (l.code)}
            <div class="level">
              <h3>{l.name}</h3>
              <p class="muted small">{l.when}</p>
              {#each l.items as i (i.prompt)}<p class="item"><strong>{i.prompt}</strong><br /><span class="ans">{i.answer}</span></p>{/each}
              {#each l.missing as m (m)}<p class="item todo">{m} <em>(not filled in yet)</em></p>{/each}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
  <div class="row"><PersonPick onchange={load} /></div>
  <PrivateLock person={who.name} needsUnlock={locked} onchange={load} />
  {#if note}<p class="notice" role="status">{note}</p>{/if}
  {#if !locked}
  <div class="wrap">
    {#if sections.length}
      <nav class="toc panel" aria-label="Sections of the plan">
        <strong>Sections</strong>
        {#each sections as s (s.line)}<button class="link" onclick={() => jump(s.line)}>{s.title}</button>{/each}
      </nav>
    {/if}
    <div class="panel plan">
      <textarea class="input" rows="32" bind:this={ta} bind:value={text} aria-label="The comms plan"></textarea>
      <div class="row">
        <button class="btn primary" onclick={save} disabled={!dirty}>Save</button>
        <button class="btn" onclick={addMissing} title="Add any sections from the full plan that yours does not have yet. Nothing you wrote is changed.">Add the sections I am missing</button>
        <button class="btn" onclick={() => window.print()} title="Print this plan on paper">Print</button>
        <span class="muted small">{dirty ? 'Not saved yet.' : ''}</span>
      </div>
    </div>
  </div>
  {/if}
</section>

<style>
  .wrap { display: flex; gap: 14px; align-items: flex-start; margin-top: 12px; flex-wrap: wrap; }
  .toc { display: flex; flex-direction: column; gap: 4px; min-width: 220px; max-width: 280px; position: sticky; top: 8px; max-height: 80vh; overflow: auto; }
  .link { text-align: left; background: none; border: 0; padding: 3px 0; color: var(--blue); font-size: 13px; cursor: pointer; }
  .link:hover { text-decoration: underline; }
  .plan { display: flex; flex-direction: column; gap: 10px; flex: 1 1 520px; max-width: 880px; border-left: 3px solid var(--cat-comms); }
  textarea { width: 100%; resize: vertical; font-family: inherit; line-height: 1.55; }
  .small { font-size: 13px; }
  .pace { margin-top: 12px; border-left: 3px solid var(--cat-comms); }
  .head { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .head h2 { margin: 0; margin-right: auto; }
  .levels { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 12px; margin-top: 8px; }
  .level h3 { margin: 0 0 2px; }
  .item { margin: 8px 0 0; }
  .ans { white-space: pre-wrap; }
  .todo { opacity: .6; }
  .notice { margin: 8px 0 0; }
  @media print { .toc, .row, .sub, .head button { display: none; } .plan { max-width: none; border: 0; } textarea { height: auto; } }
</style>
