<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { ask, modelStatus, type Reply, type ModelStatus } from '../lib/api';
  import { settings } from '../lib/settings.svelte';

  let { openCard }: { openCard: (id: string) => void } = $props();

  let question = $state('');
  let thread: { who: 'you' | 'app'; text: string; reply?: Reply }[] = $state([]);
  let busy = $state(false);
  let status: ModelStatus | null = $state(null);
  let timer: ReturnType<typeof setInterval> | undefined;

  // what the assistant is allowed to look in (Settings, Assistant)
  const searching = $derived(
    [
      settings.values.searchBooks && 'your books',
      settings.values.searchArchives && 'the archives',
      settings.values.searchFiles && 'files and folders',
      settings.values.searchSavedSites && 'saved websites',
    ].filter((x): x is string => typeof x === 'string'),
  );

  async function refresh() {
    status = await modelStatus();
    if (status.state === 'ready' || status.state === 'error' || status.state === 'none') {
      if (timer) { clearInterval(timer); timer = undefined; }
    }
  }
  onMount(() => { refresh(); timer = setInterval(refresh, 2000); });
  onDestroy(() => { if (timer) clearInterval(timer); });

  async function send() {
    const q = question.trim();
    if (!q || busy) return;
    question = '';
    thread.push({ who: 'you', text: q });
    busy = true;
    thread.push({ who: 'app', text: '' });
    const i = thread.length - 1;
    try {
      const reply = await ask(q, (t, replace) => {
        thread[i].text = replace ? t : thread[i].text + t;
      });
      thread[i] = { who: 'app', text: reply.text, reply };
    } catch (e) {
      thread[i].text = 'Something went wrong: ' + String(e);
    }
    busy = false;
  }
</script>

{#if status}
  <p class="muted status">
    {#if status.state === 'ready'}Model ready: {status.model} ({status.size}).
    {:else if status.state === 'loading'}Loading {status.model}. {status.message}
    {:else}Search-only mode. {status.message}{/if}
  </p>
{/if}
<div class="warn" role="note">
  <strong>This helper can be wrong.</strong> It is a small program that reads a few pages from your library and
  puts them in its own words. It can miss details or get things mixed up, so use it to find the right page,
  and check the page it names before you act. For anything about health, safety or medicine, trust the
  source and a real person, not this answer. It never gives medicine doses.
</div>
<details class="panel how" open={thread.length === 0}>
  <summary>How to use Ask</summary>
  <ul>
    <li>Ask one thing at a time, in plain words: "How do I make water safe to drink?" works better than "water, fire and shelter".</li>
    <li>Use the words a book would use ("purify", "bleeding", "hypothermia"). The search matches words, so a different word may find nothing.</li>
    <li>Look at the <b>Sources</b> under each answer and open them. The source is right; the summary may not be.</li>
    <li>If it says it could not find an answer, the library may not cover it. Try other words, or look in Library, Intel or Academy.</li>
    <li>For an emergency, open the emergency card when one is offered. Those were written and checked by people.</li>
    <li>It is slower on a phone than on a laptop. Wait for the answer to finish.</li>
  </ul>
</details>
<section class="chat">
  {#if thread.length === 0}
    <p class="muted">Ask a question. Answers come only from the library on this drive and show where they came from. It looks in the emergency cards{searching.length ? ', ' + searching.join(', ') : ''}. You can change that in Settings.</p>
  {/if}
  {#each thread as m}
    <div class="msg {m.who}">
      <div class="label">{m.who === 'you' ? 'YOU' : 'CINDRIX'}</div>
      <div class="text">{m.text}</div>
      {#if m.reply?.kind === 'Answered'}
        <div class="muted small">Check this against the source before you rely on it.</div>
      {/if}
      {#if settings.values.showSourceChips && m.reply?.sources.length}
        <div class="sources">Sources: {#each m.reply.sources as s}<span class="chip">{s}</span>{/each}</div>
      {/if}
      {#if m.reply?.card}
        <button class="link" onclick={() => openCard(m.reply!.card!)}>Open the emergency card</button>
      {/if}
    </div>
  {/each}
</section>
<form class="row" onsubmit={(e) => { e.preventDefault(); send(); }}>
  <label class="sr" for="q">Question</label>
  <input id="q" class="input grow" bind:value={question} placeholder="Ask anything. It stays on this device." autocomplete="off" />
  <button class="btn primary" type="submit" disabled={busy}>Send</button>
</form>

<style>
  .chat { display: flex; flex-direction: column; gap: 14px; flex: 1; }
  .msg { padding: 12px 14px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); max-width: 760px; }
  .msg.you { align-self: flex-end; background: #27231e; border-color: #3a342c; }
  .msg.app { border-left: 3px solid var(--accent); }
  .text { margin-top: 4px; white-space: pre-wrap; }
  .sources { margin-top: 10px; font-size: 12px; color: var(--muted); display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
  .chip { padding: 3px 10px; border-radius: 999px; background: #1f2a1d; border: 1px solid #34502f; color: #9fd18f; }
  .link { margin-top: 8px; background: none; border: 0; color: var(--blue); padding: 0; text-decoration: underline; }
  .warn { margin: 0 0 12px; padding: 12px 14px; border-radius: 12px; border: 1px solid #7a5a1c; background: #2a2210; color: #f0d9a0; max-width: 760px; }
  .how { max-width: 760px; margin: 0 0 12px; padding: 10px 14px; }
  .how summary { cursor: pointer; font-weight: 600; }
  .how ul { margin: 8px 0 0; padding-left: 20px; display: flex; flex-direction: column; gap: 6px; }
  .small { font-size: 12px; margin-top: 8px; }
  .status { font-size: 12px; margin: 0 0 8px; }
  .grow { flex: 1 1 280px; padding: 13px 14px; }
</style>
