<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { ask, modelStatus, chatSave, chatList, chatRead, chatDelete, opsLoad, projectSave, projectFileChat, inShare, type ModelStatus, type ChatInfo, type ProjectRec } from '../lib/api';
  import Todo from './Todo.svelte';
  import Calendar from './Calendar.svelte';
  import Projects from './Projects.svelte';
  import { settings } from '../lib/settings.svelte';
  import { chat, chatText, newChatFile, resetChat, parseChat, type Msg } from '../lib/chat.svelte';

  let { openCard, openSource }: { openCard: (id: string) => void; openSource: (link: string) => void } = $props();

  let status: ModelStatus | null = $state(null);
  let timer: ReturnType<typeof setInterval> | undefined;
  let past: ChatInfo[] = $state([]);
  let viewing: { name: string; msgs: Msg[] } | null = $state(null);
  let excerpt: { title: string; text: string; link: string } | null = $state(null);
  let scroller: HTMLDivElement | undefined = $state();
  let note = $state('');
  let menuOpen = $state(false);
  let section: 'chat' | 'projects' | 'calendar' | 'todo' = $state('chat');
  // projects group chats, notes, tasks and dates (docs/37). They are the person's own, so a guest on the hotspot has none.
  let projects: ProjectRec[] = $state([]);
  let adding = $state(false);
  let newProject = $state('');
  let opened: Record<string, boolean> = $state({});
  // asks the Projects screen to open one project, on its work page or its settings page
  let request: { id: string; tab: 'work' | 'settings'; n: number } | null = $state(null);
  let requestN = 0;
  const openProjects = $derived(projects.filter((p) => p.status !== 'done'));
  const active = $derived(projects.find((p) => p.id === chat.project) ?? null);
  const filed = $derived(new Set(projects.flatMap((p) => p.chats)));
  const unfiled = $derived(past.filter((c) => !filed.has(c.name)));
  const projectOf = (name: string) => projects.find((p) => p.chats.includes(name))?.id ?? '';
  async function loadProjects() {
    if (inShare) return;
    try { projects = (await opsLoad()).projects; if (chat.project && !projects.some((p) => p.id === chat.project)) chat.project = ''; } catch { projects = []; }
  }
  async function makeProject() {
    const name = newProject.trim();
    if (!name) { adding = false; return; }
    try { projects = await projectSave({ id: '', name, status: 'open', notes: '', chats: [], instructions: '', docs: [] }); const p = projects[projects.length - 1]; newProject = ''; adding = false; openProject(p.id, 'settings'); } catch (e) { note = String(e); }
  }
  /** Click a project: its page opens (work, or settings), and new chats are filed in it until you leave it. */
  function openProject(id: string, tab: 'work' | 'settings' = 'work') {
    chat.project = id;
    opened[id] = true;
    section = 'projects';
    request = { id, tab, n: ++requestN };
  }
  async function moveChat(name: string, to: string) {
    try { projects = await projectFileChat(to, name); } catch (e) { note = String(e); }
  }

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
  $effect(() => { section; viewing; chat.file; chat.thread.length === 0; menuOpen = false; });
  // the project list is read again whenever the screen changes, so a project made on its own page shows here at once
  $effect(() => { section; void loadProjects(); });
  async function loadPast() {
    try { past = await chatList(); } catch { past = []; }
  }
  onMount(() => {
    refresh(); timer = setInterval(refresh, 2000); void loadPast(); void loadProjects();
    // a chat opened from a link (a project, for example)
    try { const n = sessionStorage.getItem('cindrix.openChat'); if (n) { sessionStorage.removeItem('cindrix.openChat'); void view(n); } } catch { /* nothing to open */ }
  });
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
      }, chat.project);
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
      const first = !past.some((c) => c.name === chat.file);
      await chatSave(chat.file, chatText(chat.thread));
      // a new chat started inside a project is filed there
      if (first && chat.project) { try { projects = await projectFileChat(chat.project, chat.file); } catch { /* the chat is saved; it just stays unfiled */ } }
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
  /** A source shows the words it gave, and offers to open the page itself when it has one. */
  function openChip(m: Msg, k: number) {
    const link = m.reply?.links?.[k] ?? '';
    const text = m.reply?.excerpts?.[k];
    excerpt = { title: m.reply?.sources[k] ?? 'Source', text: text || 'No saved words for this source.', link };
  }
  function goToSource() {
    if (!excerpt?.link) return;
    const link = excerpt.link;
    excerpt = null;
    openSource(link);
  }
  // keep the newest words in view as an answer arrives
  $effect(() => {
    chat.thread.length; chat.thread[chat.thread.length - 1]?.text;
    if (scroller) queueMicrotask(() => { if (scroller) scroller.scrollTop = scroller.scrollHeight; });
  });
  async function remove(name: string) {
    try {
      if (filed.has(name)) { try { projects = await projectFileChat('', name); } catch { /* the chat is deleted anyway */ } }
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
          <button class="chip open" title="Show the words this source gave" onclick={() => openChip(m, k)}>{s} ↗</button>
        {/each}</div>
      {/if}
      {#if m.reply?.card}
        <button class="link" onclick={() => openCard(m.reply!.card!)}>Open the emergency card</button>
      {/if}
    </div>
  {/each}
{/snippet}

<div class="askpage">
<button class="burger" aria-label="Chats and menu" aria-expanded={menuOpen} title="Chats and menu" onclick={() => (menuOpen = !menuOpen)}>☰ Chats</button>
<nav class="side" class:open={menuOpen} aria-label="Chats">
  <button class="btn primary" title="Start a new chat. The one on screen is already saved." disabled={chat.busy} onclick={newChat}>＋ New chat</button>
  {#if !inShare}
  <button class="navbtn" class:on={section === 'projects'} title="All projects, with their tasks, dates and chats" onclick={() => { request = null; section = 'projects'; }}>Projects</button>
  <button class="navbtn" class:on={section === 'calendar'} title="Calendar" onclick={() => (section = 'calendar')}>Calendar</button>
  <button class="navbtn" class:on={section === 'todo'} title="To-do list" onclick={() => (section = 'todo')}>To-do list</button>
  <div class="sidehead row between"><span>Projects</span><button class="x show" title="Start a project" aria-label="Start a project" onclick={() => (adding = !adding)}>＋</button></div>
  {#if adding}
    <form class="padx" onsubmit={(e) => { e.preventDefault(); makeProject(); }}>
      <input class="input" placeholder="Project name" bind:value={newProject} aria-label="Project name" />
    </form>
  {/if}
  {#each openProjects as p (p.id)}
    <div class="proj" class:on={chat.project === p.id}>
      <button class="caret" title={opened[p.id] ? 'Hide this project\'s chats' : 'Show this project\'s chats'} aria-label={(opened[p.id] ? 'Hide chats of ' : 'Show chats of ') + p.name} onclick={() => (opened[p.id] = !opened[p.id])}>{opened[p.id] ? '▾' : '▸'}</button>
      <button class="chatbtn" title="Open this project: its tasks, dates, chats and settings. New chats are filed in it." onclick={() => openProject(p.id)}>{p.name}</button>
      <button class="x show" title="Settings for this project: instructions and documents" aria-label={'Settings for ' + p.name} onclick={() => openProject(p.id, 'settings')}>⚙</button>
    </div>
    {#if opened[p.id]}
      <ul class="pastlist indent">
        {#each past.filter((c) => p.chats.includes(c.name)) as c (c.name)}
          <li class:on={(viewing?.name ?? chat.file) === c.name}>
            <button class="chatbtn" title={nice(c.name)} onclick={() => view(c.name)}>{nice(c.name)}</button>
            <button class="x" title="Delete this chat" aria-label={'Delete ' + nice(c.name)} onclick={() => remove(c.name)}>✕</button>
          </li>
        {:else}
          <li class="muted small none">No chats yet.</li>
        {/each}
      </ul>
    {/if}
  {:else}
    <p class="muted small none">A project keeps chats, tasks and dates for one piece of work together.</p>
  {/each}
  {/if}
  <div class="sidehead">Chats</div>
  <ul class="pastlist">
    {#each (inShare ? past : unfiled) as c (c.name)}
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
  <div class="scroller">
    <button class="btn" onclick={() => (section = 'chat')}>← Back to the chat</button>
    <svelte:boundary>
      {#if section === 'projects'}<Projects {request} onchange={loadProjects} openChat={(n) => { section = 'chat'; void view(n); }} />
      {:else if section === 'calendar'}<Calendar />
      {:else}<Todo />{/if}
      {#snippet failed(error)}
        <div class="warn" role="alert">This page could not open. {String((error as Error)?.message ?? error)}</div>
      {/snippet}
    </svelte:boundary>
  </div>
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
{#if active}
  <div class="panel inproj">
    <div class="row between"><span><span class="label">IN PROJECT</span> <strong>{active.name}</strong></span><button class="btn" onclick={() => (chat.project = '')} title="Stop filing new chats in this project">Leave project</button></div>
    {#if active.notes}<p class="muted small notes">{active.notes}</p>{/if}
  </div>
{/if}
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
    {#if !inShare}
      <label class="muted small">Project <select class="input" value={projectOf(viewing.name)} onchange={(e) => moveChat(viewing!.name, (e.currentTarget as HTMLSelectElement).value)} aria-label="File this chat in a project">
        <option value="">No project</option>{#each openProjects as p (p.id)}<option value={p.id}>{p.name}</option>{/each}</select></label>
    {/if}
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
      {#if !excerpt.link}<p class="muted small">This source is a loose file or a saved page, so there is no page here to open. The words above are what the answer used.</p>{/if}
      <div class="row">
        {#if excerpt.link}<button class="btn primary" title="Go to the page this came from" onclick={goToSource}>Open the source</button>{/if}
        <button class="btn" onclick={() => (excerpt = null)}>Close</button>
      </div>
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
  .padx { padding: 0 10px; }
  .proj { border-radius: 8px; display: flex; }
  .proj:hover, .proj.on { background: var(--panel); }
  .proj.on .chatbtn { color: var(--accent); }
  .caret { background: none; border: 0; color: var(--muted); cursor: pointer; padding: 7px 4px 7px 8px; }
  .indent { margin-left: 14px; }
  .x.show { opacity: 1; font-size: 16px; }
  .inproj { max-width: 760px; margin: 0 0 12px; padding: 10px 14px; border-left: 3px solid var(--cat-ops); }
  .inproj .notes { white-space: pre-wrap; margin: 6px 0 0; }
  .burger { display: none; }
  @media (max-width: 800px) {
    .askpage { grid-template-columns: 1fr; grid-template-rows: auto 1fr; gap: 8px; }
    .burger { display: block; justify-self: start; background: var(--panel); border: 1px solid var(--line); color: inherit; border-radius: 8px; padding: 8px 12px; cursor: pointer; }
    .side { display: none; border-right: 0; max-height: 55vh; padding: 8px; background: var(--panel); border: 1px solid var(--line); border-radius: 10px; }
    .side.open { display: flex; }
    .askpage:has(.side.open) .main { display: none; }
  }
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
