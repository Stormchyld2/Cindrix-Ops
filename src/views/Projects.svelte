<script lang="ts">
  // Projects: a named piece of work (Winter prep, The garden, Move to the cabin). It gathers its own tasks, events and
  // saved Ask chats, and links to the plans, items and guides it is about. Notes live in user/projects.txt.
  import { onMount } from 'svelte';
  import { opsLoad, projectSave, projectDelete, taskSave, taskDone, projectFileChat, chatList, type ProjectRec, type TaskRec, type EventRec } from '../lib/api';
  import Connected from '../lib/Connected.svelte';
  import { nav, openLink } from '../lib/nav.svelte';

  let { openChat }: { openChat?: (name: string) => void } = $props();
  let projects: ProjectRec[] = $state([]);
  let tasks: TaskRec[] = $state([]);
  let events: EventRec[] = $state([]);
  let chats: string[] = $state([]);
  let selected: string | null = $state(null);
  let note = $state('');
  let newName = $state('');
  let quick = $state('');
  let draft: ProjectRec | null = $state(null);

  async function load() {
    try {
      const d = await opsLoad();
      projects = d.projects; tasks = d.tasks; events = d.events;
      chats = (await chatList()).map((c) => c.name);
      if (selected && !projects.some((p) => p.id === selected)) selected = null;
    } catch (e) { note = String(e); }
  }
  onMount(load);
  let want: string | null = null;
  $effect(() => { const o = nav.open; if (o?.kind === 'project') want = o.id; });
  $effect(() => { if (want && projects.some((p) => p.id === want)) { choose(want); want = null; } });

  function choose(id: string) { selected = id; const p = projects.find((x) => x.id === id); draft = p ? structuredClone(p) : null; }
  async function create() {
    if (!newName.trim()) { note = 'Give the project a name first.'; return; }
    try { projects = await projectSave({ id: '', name: newName, status: 'open', notes: '', chats: [] }); newName = ''; note = ''; const p = projects[projects.length - 1]; choose(p.id); } catch (e) { note = String(e); }
  }
  async function saveDraft() { if (!draft) return; try { projects = await projectSave(draft); note = 'Saved.'; } catch (e) { note = String(e); } }
  async function remove() {
    if (!draft || !confirm('Delete the project "' + draft.name + '"? Its tasks, events and chats stay; they just stop belonging to it.')) return;
    try { projects = await projectDelete(draft.id); selected = null; draft = null; await load(); } catch (e) { note = String(e); }
  }
  async function addTask() {
    if (!selected || !quick.trim()) return;
    try { tasks = await taskSave({ id: '', title: quick, due: '', done: '', project: selected, every_days: 0, notes: '', links: [] }); quick = ''; } catch (e) { note = String(e); }
  }
  async function tick(t: TaskRec) { try { tasks = await taskDone(t.id, !t.done); } catch (e) { note = String(e); } }
  async function addChat(name: string) {
    if (!selected || !name) return;
    try { projects = await projectFileChat(selected, name); draft = structuredClone(projects.find((p) => p.id === selected) ?? draft); } catch (e) { note = String(e); }
  }
  async function dropChat(name: string) {
    try { projects = await projectFileChat('', name); draft = structuredClone(projects.find((p) => p.id === selected) ?? draft); } catch (e) { note = String(e); }
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
  .end { justify-content: flex-end; }
  @media (max-width: 860px) { .wrap { grid-template-columns: 1fr; } }
</style>
