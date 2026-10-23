<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { ask, modelStatus, chatSave, chatList, chatRead, chatDelete, type ModelStatus, type ChatInfo } from '../lib/api';
  import { settings } from '../lib/settings.svelte';
  import { chat, chatText, newChatFile, resetChat, parseChat, type Msg } from '../lib/chat.svelte';

  let { openCard, openSource }: { openCard: (id: string) => void; openSource: (link: string) => void } = $props();

  let status: ModelStatus | null = $state(null);
  let timer: ReturnType<typeof setInterval> | undefined;
  let past: ChatInfo[] = $state([]);
  let viewing: { name: string; msgs: Msg[] } | null = $state(null);
  let excerpt: { title: string; text: string } | null = $state(null);
  let scroller: HTMLDivElement | undefined = $state();
  let note = $state('');
  let section: 'chat' | 'projects' | 'calendar' | 'todo' = $state('chat');

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
    section = 'chat';
  }
  async function view(name: string) {
    section = 'chat';
    try { viewing = { name, msgs: parseChat(await chatRead(name)) }; } catch (e) { note = String(e); }
  }
  /** Put a saved chat back on screen so the talk can carry on from it. */
  function carryOn() {
    if (!viewing || chat.busy) return;
    chat.thread = viewing.msgs;
    chat.file = viewing.name;
    viewing = null;
  }
  /** A source with a page opens it; one without shows the words it gave. */
  function openChip(m: Msg, k: number) {
    const link = m.reply?.links?.[k];
    if (link) { openSource(link); return; }
    const text = m.reply?.excerpts?.[k];
    excerpt = { title: m.reply?.sources[k] ?? 'Source', text: text || 'This source has no page to open and no saved words to show.' };
  }
  // keep the newest words in view as an answer arrives
  $effect(() => {
    chat.thread.length; chat.thread[chat.thread.length - 1]?.text;
    if (scroller) queueMicrotask(() => { if (scroller) scroller.scrollTop = scroller.scrollHeight; });
  });
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

{#snippet messages(list: Msg[])}
  {#each list as m}
    <div class="msg {m.who}">
      <div class="label">{m.who === 'you' ? 'YOU' : 'CINDRIX'}</div>
      <div class="text">{m.text}</div>
      {#if m.reply?.kind === 'Answered'}
        <div class="muted small">Check this against the source before you rely on it.</div>
      {/if}
      {#if settings.values.showSourceChips && m.reply?.sources.length}
        <div class="sources">Sources: {#each m.reply.sources as s, k}
          <button class="chip open" title={m.reply?.links?.[k] ? 'Open this page' : 'Show the words this source gave'} onclick={() => openChip(m, k)}>{s} ↗</button>
        {/each}</div>
      {/if}
      {#if m.reply?.card}
        <button class="link" onclick={() => openCard(m.reply!.card!)}>Open the emergency card</button>
      {/if}
    </div>
  {/each}
{/snippet}

<div class="askpage">
<nav class="side" aria-label="Chats">
  <button class="btn primary" title="Start a new chat. The one on screen is already saved." disabled={chat.busy} onclick={newChat}>＋ New chat</button>
  <button class="navbtn" class:on={section === 'projects'} title="Projects" onclick={() => (section = 'projects')}>Projects</button>
  <button class="navbtn" class:on={section === 'calendar'} title="Calendar" onclick={() => (section = 'calendar')}>Calendar</button>
  <button class="navbtn" class:on={section === 'todo'} title="To-do list" onclick={() => (section = 'todo')}>To-do list</button>
  <div class="sidehead">Projects</div>
  <p class="muted small none">No projects yet.</p>
  <div class="sidehead">Chats</div>
  <ul class="pastlist">
    {#each past as c (c.name)}
      <li class:on={(viewing?.name ?? chat.file) === c.name}>
        <button class="chatbtn" title={nice(c.name)} onclick={() => view(c.name)}>{nice(c.name)}</button>
        <button class="x" title="Delete this chat" aria-label={'Delete ' + nice(c.name)} onclick={() => remove(c.name)}>✕</button>
      </li>
    {:else}
      <li class="muted small none">Your chats will be listed here.</li>
    {/each}
  </ul>
</nav>
<div class="main">
{#if section !== 'chat'}
  <section class="panel soon">
    <strong>{section === 'projects' ? 'Projects' : section === 'calendar' ? 'Calendar' : 'To-do list'}</strong>
    <p class="muted">This part is not built yet. The place for it is ready here beside your chats.</p>
    <button class="btn" onclick={() => (section = 'chat')}>Back to the chat</button>
  </section>
{:else}
<div class="scroller" bind:this={scroller}>
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
  {@render messages(chat.thread)}
</section>
{#if viewing}
  <section class="panel past">
    <div class="row between"><strong>{nice(viewing.name)}</strong>
      <span class="row"><button class="btn" disabled={chat.busy} title="Put this chat back on screen and carry on from it" onclick={carryOn}>Carry on this chat</button><button class="btn" onclick={() => (viewing = null)}>Close</button></span></div>
    <div class="chat">{@render messages(viewing.msgs)}</div>
  </section>
{/if}
{#if note}<div class="warn" role="alert">{note}</div>{/if}
</div>
<form class="row bar" onsubmit={(e) => { e.preventDefault(); send(); }}>
  <label class="sr" for="q">Question</label>
  <input id="q" class="input grow" bind:value={chat.question} placeholder="Ask anything. It stays on this device." autocomplete="off" />
  <button class="btn primary" type="submit" disabled={chat.busy}>Send</button>
</form>
{/if}
</div>
</div>
{#if excerpt}
  <div class="scrim" role="presentation" onclick={() => (excerpt = null)}>
    <div class="dialog" role="dialog" aria-modal="true" aria-label="Source" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === 'Escape') excerpt = null; }}>
      <strong>{excerpt.title}</strong>
      <pre class="transcript">{excerpt.text}</pre>
      <button class="btn" onclick={() => (excerpt = null)}>Close</button>
    </div>
  </div>
{/if}

<style>
  .askpage { display: grid; grid-template-columns: 250px 1fr; height: calc(100vh - 120px); min-height: 440px; gap: 14px; }
  .main { display: flex; flex-direction: column; min-width: 0; min-height: 0; gap: 10px; }
  .side { display: flex; flex-direction: column; gap: 6px; min-height: 0; overflow-y: auto; scrollbar-width: none; padding-right: 10px; border-right: 1px solid var(--line); }
  .side::-webkit-scrollbar { display: none; }
  .navbtn { text-align: left; background: none; border: 0; color: inherit; padding: 8px 10px; border-radius: 8px; cursor: pointer; }
  .navbtn:hover, .navbtn.on { background: var(--panel); }
  .sidehead { margin-top: 10px; font-size: 11px; letter-spacing: .08em; text-transform: uppercase; color: var(--muted); padding: 0 10px; }
  .none { padding: 0 10px; margin: 0; }
  .side .pastlist { margin: 0; gap: 2px; }
  .side .pastlist li { border-radius: 8px; padding-right: 4px; }
  .side .pastlist li.on, .side .pastlist li:hover { background: var(--panel); }
  .chatbtn { flex: 1; min-width: 0; text-align: left; background: none; border: 0; color: inherit; padding: 7px 10px; cursor: pointer; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .x { background: none; border: 0; color: var(--muted); cursor: pointer; opacity: 0; }
  .pastlist li:hover .x, .pastlist li.on .x { opacity: 1; }
  .soon { max-width: 560px; }
  @media (max-width: 800px) { .askpage { grid-template-columns: 1fr; height: auto; } .side { max-height: 30vh; border-right: 0; } .main { height: 70vh; } }
  .scroller { flex: 1 1 auto; overflow-y: auto; min-height: 0; scrollbar-width: none; -ms-overflow-style: none; }
  .scroller::-webkit-scrollbar, .dialog::-webkit-scrollbar { display: none; }
  .bar { flex: none; }
  .scrim { position: fixed; inset: 0; background: rgba(0,0,0,.55); display: flex; align-items: center; justify-content: center; z-index: 50; }
  .dialog { background: var(--panel, #1b1f24); border: 1px solid var(--line, #444); border-radius: 10px; padding: 16px; width: min(640px, 92vw); max-height: 80vh; overflow-y: auto; scrollbar-width: none; display: flex; flex-direction: column; gap: 10px; }
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
