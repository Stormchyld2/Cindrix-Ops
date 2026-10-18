<script lang="ts">
  import { ask, type Reply } from '../lib/api';

  let { openCard }: { openCard: (id: string) => void } = $props();

  let question = $state('');
  let thread: { who: 'you' | 'app'; text: string; reply?: Reply }[] = $state([]);
  let busy = $state(false);

  async function send() {
    const q = question.trim();
    if (!q || busy) return;
    question = '';
    thread.push({ who: 'you', text: q });
    busy = true;
    const reply = await ask(q);
    thread.push({ who: 'app', text: reply.text, reply });
    busy = false;
  }
</script>

<section class="chat">
  {#if thread.length === 0}
    <p class="muted">Ask a question. Answers come only from the library on this drive and show where they came from.</p>
  {/if}
  {#each thread as m}
    <div class="msg {m.who}">
      <div class="label">{m.who === 'you' ? 'YOU' : 'CINDRIX'}</div>
      <div class="text">{m.text}</div>
      {#if m.reply?.sources.length}
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
  .link { margin-top: 8px; background: none; border: 0; color: var(--accent); padding: 0; text-decoration: underline; }
  .grow { flex: 1 1 280px; padding: 13px 14px; }
</style>
