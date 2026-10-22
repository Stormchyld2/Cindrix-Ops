<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { ask, modelStatus, chatSave, chatList, chatRead, chatDelete, type ModelStatus, type ChatInfo } from '../lib/api';
  import { settings } from '../lib/settings.svelte';
  import { chat, chatText, newChatFile, resetChat } from '../lib/chat.svelte';

  let { openCard, openSource }: { openCard: (id: string) => void; openSource: (link: string) => void } = $props();

  let status: ModelStatus | null = $state(null);
  let timer: ReturnType<typeof setInterval> | undefined;
  let past: ChatInfo[] = $state([]);
  let viewing: { name: string; text: string } | null = $state(null);
  let note = $state('');

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
  async function loadPast() {
    try { past = await chatList(); } catch { past = []; }
  }
  onMount(() => { refresh(); timer = setInterval(refresh, 2000); void loadPast(); });
  onDestroy(() => { if (timer) clearInterval(timer); });

  async function send() {
    const q = chat.question.trim();
    if (!q || chat.busy) return;
    chat.question = '';
    viewing = null;
    chat.thread.push({ who: 'you', text: q });
    chat.busy = true;
    chat.thread.push({ who: 'app', text: '' });
    const i = chat.thread.length - 1;
    try {
      const reply = await ask(q, (t, replace) => {
        chat.thread[i].text = replace ? t : chat.thread[i].text + t;
      });
      chat.thread[i] = { who: 'app', text: reply.text, reply };
    } catch (e) {
      chat.thread[i].text = 'Something went wrong: ' + String(e);
    }
    chat.busy = false;
    await keep();
  }

  /** Save the chat as a small text file, so it can be read again later. */
  async function keep() {
    try {
      if (!chat.file) chat.file = newChatFile(chat.thread.find((m) => m.who === 'you')?.text ?? 'chat');
      await chatSave(chat.file, chatText(chat.thread));
      await loadPast();
    } catch (e) {
      note = 'This chat could not be saved: ' + String(e);
    }
  }

  function newChat() {
    if (chat.busy) return;
    resetChat();
    viewing = null;
  }
  async function view(name: string) {
    try { viewing = { name, text: await chatRead(name) }; } catch (e) { note = String(e); }
  }
  async function remove(name: string) {
    try {
      await chatDelete(name);
      if (viewing?.name === name) viewing = null;
      if (chat.file === name) chat.file = '';
      await loadPast();
    } catch (e) { note = String(e); }
  }
  const nice = (name: string) => name.replace(/\.txt$/, '').replace(/^(\d{4}-\d{2}-\d{2})-(\d{2})(\d{2})-/, '$1 $2:$3  ').replace(/-/g, ' ');
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
<details class="panel how" open={chat.thread.length === 0}>
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
  {#if chat.thread.length === 0 && !viewing}
    <p class="muted">Ask a question. Answers come only from the library on this drive and show where they came from. It looks in the emergency cards{searching.length ? ', ' + searching.join(', ') : ''}. You can change that in Settings.</p>
  {/if}
  {#each chat.thread as m}
    <div class="msg {m.who}">
      <div class="label">{m.who === 'you' ? 'YOU' : 'CINDRIX'}</div>
      <div class="text">{m.text}</div>
      {#if m.reply?.kind === 'Answered'}
        <div class="muted small">Check this against the source before you rely on it.</div>
      {/if}
      {#if settings.values.showSourceChips && m.reply?.sources.length}
        <div class="sources">Sources: {#each m.reply.sources as s, k}
          {#if m.reply?.links?.[k]}<button class="chip open" title="Open this page" onclick={() => openSource(m.reply!.links![k])}>{s} ↗</button>{:else}<span class="chip">{s}</span>{/if}
        {/each}</div>
      {/if}
      {#if m.reply?.card}
        <button class="link" onclick={() => openCard(m.reply!.card!)}>Open the emergency card</button>
      {/if}
    </div>
  {/each}
</section>
{#if viewing}
  <section class="panel past">
    <div class="row between"><strong>{nice(viewing.name)}</strong><button class="btn" onclick={() => (viewing = null)}>Close</button></div>
    <pre class="transcript">{viewing.text}</pre>
  </section>
{/if}
{#if note}<div class="warn" role="alert">{note}</div>{/if}
<form class="row" onsubmit={(e) => { e.preventDefault(); send(); }}>
  <label class="sr" for="q">Question</label>
  <input id="q" class="input grow" bind:value={chat.question} placeholder="Ask anything. It stays on this device." autocomplete="off" />
  <button class="btn primary" type="submit" disabled={chat.busy}>Send</button>
  <button class="btn" type="button" title="Start a new chat. The one on screen is already saved." disabled={chat.busy || chat.thread.length === 0} onclick={newChat}>New chat</button>
</form>
{#if past.length}
  <details class="panel how">
    <summary>Past chats ({past.length})</summary>
    <p class="muted small">Saved on the drive in <code>user\chats</code> as plain text. A chat takes only a few kilobytes.</p>
    <ul class="pastlist">
      {#each past as c (c.name)}
        <li><button class="link" onclick={() => view(c.name)}>{nice(c.name)}</button><button class="btn" aria-label={'Delete ' + nice(c.name)} onclick={() => remove(c.name)}>Delete</button></li>
      {/each}
    </ul>
  </details>
{/if}

<style>
  .chat { display: flex; flex-direction: column; gap: 14px; flex: 1; }
  .msg { padding: 12px 14px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); max-width: 760px; }
  .msg.you { align-self: flex-end; background: #27231e; border-color: #3a342c; }
  .msg.app { border-left: 3px solid var(--accent); }
  .text { margin-top: 4px; white-space: pre-wrap; }
  .sources { margin-top: 10px; font-size: 12px; color: var(--muted); display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
  .chip.open { cursor: pointer; text-decoration: underline; }
  .chip.open:hover { border-color: #6fcf8d; }
  .between { justify-content: space-between; }
  .transcript { white-space: pre-wrap; font-family: inherit; margin: 8px 0 0; }
  .pastlist { list-style: none; padding: 0; margin: 8px 0 0; display: flex; flex-direction: column; gap: 6px; }
  .pastlist li { display: flex; justify-content: space-between; gap: 10px; align-items: center; }
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
