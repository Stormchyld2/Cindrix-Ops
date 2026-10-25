<script lang="ts">
  // Projects: a named piece of work (Winter prep, The garden, Move to the cabin). It gathers its own tasks, events and
  // saved Ask chats, and links to the plans, items and guides it is about. Notes live in user/projects.txt.
  import { onMount } from 'svelte';
  import { opsLoad, projectSave, projectDelete, taskSave, taskDone, projectFileChat, chatList, projectDocLink, projectDocUpload, projectDocRemove, projectDocOpen, projectDocImage, MAX_PROJECT_DOCS, type ProjectRec, type TaskRec, type EventRec } from '../lib/api';
  import Connected from '../lib/Connected.svelte';
  import { nav, openLink } from '../lib/nav.svelte';

  // `onchange` tells the Ask side panel the project list changed; `request` asks for a project (and its tab) to be opened.
  let { openChat, onchange, request = null }: { openChat?: (name: string) => void; onchange?: () => void; request?: { id: string; tab: 'work' | 'settings'; n: number } | null } = $props();
  let projects: ProjectRec[] = $state([]);
  let tasks: TaskRec[] = $state([]);
  let events: EventRec[] = $state([]);
  let chats: string[] = $state([]);
  let selected: string | null = $state(null);
  let note = $state('');
  let newName = $state('');
  let quick = $state('');
  let draft: ProjectRec | null = $state(null);
  let tab: 'work' | 'settings' = $state('work');
  let linkName = $state('');
  let linkPath = $state('');
  let thumbs: Record<string, string> = $state({});
  let busy = $state(false);

  async function load() {
    try {
      const d = await opsLoad();
      projects = d.projects; tasks = d.tasks; events = d.events;
      chats = (await chatList()).map((c) => c.name);
      if (selected && !projects.some((p) => p.id === selected)) selected = null;
    } catch (e) { note = String(e); }
  }
  onMount(load);
  let want: { id: string; tab: 'work' | 'settings' } | null = $state(null);
  $effect(() => { const o = nav.open; if (o?.kind === 'project') want = { id: o.id, tab: 'work' }; });
  // asked for from the side panel: read the list again first, because the project may have only just been made
  $effect(() => { const r = request; if (r?.id) void load().then(() => { want = { id: r.id, tab: r.tab }; }); });
  $effect(() => { const w = want; if (w && projects.some((p) => p.id === w.id)) { choose(w.id); tab = w.tab; want = null; } });

  function choose(id: string) { selected = id; tab = 'work'; const p = projects.find((x) => x.id === id); draft = p ? (copyOf(p) as ProjectRec) : null; }
  // A reactive object cannot go through structuredClone (the browser refuses it), so take a plain copy first.
  function copyOf<T>(v: T): T { return $state.snapshot(v) as T; }
  async function create() {
    if (!newName.trim()) { note = 'Give the project a name first.'; return; }
    try { projects = await projectSave({ id: '', name: newName, status: 'open', notes: '', chats: [], instructions: '', docs: [] }); newName = ''; note = ''; const p = projects[projects.length - 1]; choose(p.id); tab = 'settings'; onchange?.(); } catch (e) { note = String(e); }
  }
  // Documents are saved by their own buttons; only the text fields go with Save.
  function keepDocs(list: ProjectRec[]) {
    projects = list;
    const p = list.find((x) => x.id === selected);
    if (p && draft) draft.docs = copyOf(p.docs);
    loadThumbs();
  }
  async function loadThumbs() {
    const p = draft; if (!p) return;
    for (let i = 0; i < p.docs.length; i++) {
      const d = p.docs[i]; const key = p.id + ':' + d.kind + ':' + d.target;
      if (d.image && !d.missing && !(key in thumbs)) { try { const u = await projectDocImage(p.id, i); if (u) thumbs[key] = u; } catch { /* no preview */ } }
    }
  }
  const thumb = (d: { kind: string; target: string }) => (draft ? thumbs[draft.id + ':' + d.kind + ':' + d.target] : undefined);
  async function addLink() {
    if (!draft) return;
    try { keepDocs(await projectDocLink(draft.id, linkName, linkPath)); linkName = ''; linkPath = ''; note = 'Linked.'; } catch (e) { note = String(e); }
  }
  async function addFiles(ev: Event) {
    const input = ev.currentTarget as HTMLInputElement;
    const files = Array.from(input.files ?? []);
    input.value = '';
    if (!draft || !files.length) return;
    busy = true;
    for (const f of files) {
      try { keepDocs(await projectDocUpload(draft.id, f)); note = 'Added.'; } catch (e) { note = String(e); break; }
    }
    busy = false;
  }
  async function dropDoc(i: number) {
    if (!draft) return;
    const d = draft.docs[i];
    if (!confirm(d.kind === 'upload' ? 'Take "' + d.name + '" out? The copy kept in this project is deleted.' : 'Take "' + d.name + '" out of this project? The file itself is not touched.')) return;
    try { keepDocs(await projectDocRemove(draft.id, i)); } catch (e) { note = String(e); }
  }
  async function openDoc(i: number) { if (!draft) return; try { await projectDocOpen(draft.id, i); } catch (e) { note = String(e); } }
  $effect(() => { if (tab === 'settings') loadThumbs(); });
  async function saveDraft() { if (!draft) return; try { projects = await projectSave(copyOf(draft)); note = 'Saved.'; onchange?.(); } catch (e) { note = String(e); } }
  async function remove() {
    if (!draft || !confirm('Delete the project "' + draft.name + '"? Its tasks, events and chats stay; they just stop belonging to it.')) return;
    try { projects = await projectDelete(draft.id); selected = null; draft = null; await load(); onchange?.(); } catch (e) { note = String(e); }
  }
  async function addTask() {
    if (!selected || !quick.trim()) return;
    try { tasks = await taskSave({ id: '', title: quick, due: '', done: '', project: selected, every_days: 0, notes: '', links: [] }); quick = ''; } catch (e) { note = String(e); }
  }
  async function tick(t: TaskRec) { try { tasks = await taskDone(t.id, !t.done); } catch (e) { note = String(e); } }
  async function addChat(name: string) {
    if (!selected || !name) return;
    try { projects = await projectFileChat(selected, name); draft = copyOf(projects.find((p) => p.id === selected) ?? draft); onchange?.(); } catch (e) { note = String(e); }
  }
  async function dropChat(name: string) {
    try { projects = await projectFileChat('', name); draft = copyOf(projects.find((p) => p.id === selected) ?? draft); onchange?.(); } catch (e) { note = String(e); }
  }
  const mine = $derived(tasks.filter((t) => t.project === selected));
  const myEvents = $derived(events.filter((e) => e.project === selected).sort((a, b) => a.date.localeCompare(b.date)));
  // what the project depends on: everything its tasks and events link to, once each
  const deps = $derived.by(() => {
    const seen = new Set<string>();
    const out: { kind: string; id: string }[] = [];
    for (const l of [...mine.flatMap((t) => t.links), ...myEvents.flatMap((e) => e.links)]) {
      const k = l.kind + ':' + l.id;
      if (!seen.has(k)) { seen.add(k); out.push(l); }
    }
    return out;
  });
  const freeChats = $derived(chats.filter((c) => !projects.some((p) => p.chats.includes(c))));
</script>

<section>
  <h1>Projects</h1>
  <p class="sub">A project gathers everything about one piece of work: its tasks, its dates, the Ask chats you had about it, and the plans, items and guides it depends on.</p>
  {#if note}<p class="notice" role="status">{note}</p>{/if}
  <div class="wrap">
    <nav class="panel list" aria-label="Projects">
      <div class="row"><input class="input grow" placeholder="New project name" bind:value={newName} onkeydown={(e) => e.key === 'Enter' && create()} aria-label="New project name" /><button class="btn primary" onclick={create}>Add</button></div>
      {#each projects as p (p.id)}
        <button class="pick" class:on={p.id === selected} class:isdone={p.status === 'done'} onclick={() => choose(p.id)}>
          <strong>{p.name}</strong>
          <span class="muted small">{tasks.filter((t) => t.project === p.id && !t.done).length} open · {p.chats.length} chats</span>
        </button>
      {/each}
      {#if !projects.length}<p class="muted small">No projects yet.</p>{/if}
    </nav>

    {#if draft && selected}
      <div class="panel detail">
        <div class="row"><input class="input grow" bind:value={draft.name} aria-label="Project name" />
          <select class="input" bind:value={draft.status} aria-label="Status"><option value="open">Open</option><option value="done">Finished</option></select>
          <button class="btn primary" onclick={saveDraft}>Save</button></div>
        <div class="tabs" role="tablist">
          <button role="tab" aria-selected={tab === 'work'} class:on={tab === 'work'} onclick={() => (tab = 'work')}>Work</button>
          <button role="tab" aria-selected={tab === 'settings'} class:on={tab === 'settings'} title="Instructions for the assistant and the documents that belong to this project" onclick={() => (tab = 'settings')}>Settings ({draft.docs.length} documents)</button>
        </div>
        {#if tab === 'settings'}
          <h2>Instructions</h2>
          <p class="muted small">Standing instructions for the assistant whenever you ask about this project, such as “We are 4 adults and 2 children. Keep answers short. Use metric.” They are also used to suggest links. The assistant still never gives medicine doses.</p>
          <textarea class="input" rows="6" maxlength="4000" placeholder="Instructions for this project" bind:value={draft.instructions} aria-label="Project instructions"></textarea>
          <div class="row"><button class="btn primary" onclick={saveDraft}>Save instructions</button></div>

          <h2>Documents and pictures ({draft.docs.length} of {MAX_PROJECT_DOCS})</h2>
          <p class="muted small">Link a file where it already is, or add a copy to this project (for a scan or photo from a phone). Pictures, PDFs, text and office files. Copies up to 25 MB.</p>
          <ul class="docs">
            {#each draft.docs as d, i (d.kind + ':' + d.target)}
              <li class:gone={d.missing}>
                {#if thumb(d)}<button class="pic" title="Open this picture" onclick={() => openDoc(i)}><img src={thumb(d)} alt={d.name} /></button>{/if}
                <div class="grow">
                  <button class="lnk" title="Open with the program this computer uses for it" onclick={() => openDoc(i)}>{d.name}</button>
                  <div class="muted small">{d.kind === 'upload' ? 'Copy kept in this project' : 'Linked: ' + d.target}{d.missing ? ' · not found. Move it back or link it again.' : ''}</div>
                </div>
                <button class="btn" onclick={() => dropDoc(i)}>Take out</button>
              </li>
            {/each}
          </ul>
          {#if !draft.docs.length}<p class="muted small">Nothing added yet.</p>{/if}
          {#if draft.docs.length < MAX_PROJECT_DOCS}
            <div class="row"><input class="input" placeholder="Name (optional)" bind:value={linkName} aria-label="Name for the linked file" /><input class="input grow" placeholder="Path, such as D:\Scans\deed.pdf" bind:value={linkPath} onkeydown={(e) => e.key === 'Enter' && addLink()} aria-label="Path of the file to link" /><button class="btn" onclick={addLink}>Link it</button></div>
            <label class="btn filebtn" title="Choose pictures or documents to keep a copy of in this project">{busy ? 'Adding…' : 'Add a file…'}<input type="file" multiple accept=".png,.jpg,.jpeg,.gif,.webp,.bmp,.heic,.pdf,.txt,.md,.csv,.doc,.docx,.xls,.xlsx,.odt,.ods,.rtf" onchange={addFiles} /></label>
          {:else}
            <p class="muted small">This project holds {MAX_PROJECT_DOCS} documents. Take one out to add another.</p>
          {/if}
        {:else}
        <textarea class="input" rows="4" placeholder="Notes: what is this for, what must be true when it is finished?" bind:value={draft.notes} aria-label="Project notes"></textarea>

        <h2>Tasks</h2>
        <div class="row"><input class="input grow" placeholder="Add a task to this project" bind:value={quick} onkeydown={(e) => e.key === 'Enter' && addTask()} aria-label="New task for this project" /><button class="btn" onclick={addTask}>Add</button></div>
        <ul class="plain">{#each mine as t (t.id)}<li><label><input type="checkbox" checked={!!t.done} onchange={() => tick(t)} /> <span class:isdone={!!t.done}>{t.title}</span>{#if t.due} <span class="muted small">· {t.due}</span>{/if}</label></li>{/each}</ul>
        {#if !mine.length}<p class="muted small">No tasks yet.</p>{/if}

        {#if myEvents.length}
          <h2>Dates</h2>
          <ul class="plain">{#each myEvents as e (e.id)}<li><button class="lnk" onclick={() => openLink('event', e.id)}>{e.date}{e.time ? ' ' + e.time : ''} · {e.title}</button></li>{/each}</ul>
        {/if}

        <h2>Ask chats</h2>
        <ul class="plain">
          {#each draft.chats as c (c)}<li><button class="lnk" onclick={() => openChat?.(c)}>{c.replace(/\.txt$/, '')}</button> <button class="x" aria-label={'Take ' + c + ' out'} onclick={() => dropChat(c)}>&times;</button></li>{/each}
        </ul>
        {#if freeChats.length}
          <select class="input" aria-label="File a saved chat here" onchange={(e) => { addChat((e.currentTarget as HTMLSelectElement).value); (e.currentTarget as HTMLSelectElement).value = ''; }}>
            <option value="">File a saved chat in this project…</option>
            {#each freeChats as c (c)}<option value={c}>{c.replace(/\.txt$/, '')}</option>{/each}
          </select>
        {/if}

        <h2>Connected to</h2>
        <p class="muted small">Everything its tasks and events link to. Add a link from any task or event with “+ Link”.</p>
        <Connected links={deps} />
        {/if}
        <div class="row end"><button class="btn" onclick={remove}>Delete project</button></div>
      </div>
    {:else}
      <div class="panel detail"><p class="muted">Pick a project, or add one.</p></div>
    {/if}
  </div>
</section>

<style>
  .wrap { display: grid; grid-template-columns: 280px 1fr; gap: 14px; margin-top: 12px; align-items: start; }
  .panel { border-left: 3px solid var(--cat-ops); }
  .list { display: flex; flex-direction: column; gap: 6px; }
  .grow { flex: 1; min-width: 120px; }
  .pick { text-align: left; padding: 8px 10px; border: 1px solid var(--line); border-radius: 8px; background: var(--panel2); display: flex; flex-direction: column; }
  .pick.on { border-color: var(--blue-line); background: var(--blue-soft); }
  .pick.isdone strong { text-decoration: line-through; color: var(--muted); }
  .detail { display: flex; flex-direction: column; gap: 10px; }
  .detail h2 { font-size: 14px; margin-top: 8px; color: var(--muted); }
  textarea { width: 100%; resize: vertical; font-family: inherit; }
  .plain { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
  .isdone { text-decoration: line-through; color: var(--muted); }
  .lnk { border: 0; background: transparent; padding: 0; color: var(--link); text-align: left; }
  .x { border: 0; background: transparent; color: var(--muted); }
  .small { font-size: 13px; }
  .tabs { display: flex; gap: 6px; border-bottom: 1px solid var(--line); }
  .tabs button { border: 0; background: transparent; padding: 6px 12px; border-bottom: 2px solid transparent; }
  .tabs button.on { border-bottom-color: var(--blue); color: var(--blue); }
  .docs { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .docs li { display: flex; gap: 10px; align-items: center; border: 1px solid var(--line); border-radius: 8px; padding: 6px 8px; background: var(--panel2); }
  .docs li.gone { opacity: .7; }
  .pic { border: 0; padding: 0; background: transparent; }
  .pic img { width: 56px; height: 56px; object-fit: cover; border-radius: 6px; display: block; }
  .filebtn { position: relative; align-self: flex-start; cursor: pointer; }
  .filebtn input { position: absolute; inset: 0; opacity: 0; cursor: pointer; width: 100%; }
  .end { justify-content: flex-end; }
  @media (max-width: 860px) { .wrap { grid-template-columns: 1fr; } }
</style>
