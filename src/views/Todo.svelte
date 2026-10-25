<script lang="ts">
  // The to-do list: one place for everything that needs doing. Tasks can repeat, belong to a project, and link to
  // an item, plan, card or guide. Saved as plain text in user/tasks.txt.
  import { onMount } from 'svelte';
  import { opsLoad, taskSave, taskDelete, taskDone, type TaskRec, type ProjectRec, type Link } from '../lib/api';
  import Connected from '../lib/Connected.svelte';
  import { nav } from '../lib/nav.svelte';

  let tasks: TaskRec[] = $state([]);
  let projects: ProjectRec[] = $state([]);
  let today = $state('');
  let note = $state('');
  let showDone = $state(false);
  let filterProject = $state('');
  let editing: string | null = $state(null);
  let draft: TaskRec = $state(blank());

  function blank(): TaskRec { return { id: '', title: '', due: '', done: '', project: '', every_days: 0, notes: '', links: [] }; }
  async function load() {
    try { const d = await opsLoad(); tasks = d.tasks; projects = d.projects; today = d.today; } catch (e) { note = String(e); }
  }
  onMount(load);
  // opened from a link elsewhere: show that task's editor
  let want: string | null = $state(null);
  $effect(() => { const o = nav.open; if (o?.kind === 'task') want = o.id; });
  $effect(() => { const t = want ? tasks.find((x) => x.id === want) : null; if (t) { editing = t.id; draft = $state.snapshot(t) as TaskRec; want = null; } });

  async function add() {
    if (!draft.title.trim()) { note = 'Type what needs doing first.'; return; }
    try { tasks = await taskSave({ ...draft, id: '' }); draft = blank(); note = ''; } catch (e) { note = String(e); }
  }
  async function tick(t: TaskRec) { try { tasks = await taskDone(t.id, !t.done); } catch (e) { note = String(e); } }
  async function remove(t: TaskRec) {
    if (!confirm('Take "' + t.title + '" off the list?')) return;
    try { tasks = await taskDelete(t.id); if (editing === t.id) editing = null; } catch (e) { note = String(e); }
  }
  function edit(t: TaskRec) { editing = editing === t.id ? null : t.id; if (editing) draft = $state.snapshot(t) as TaskRec; else draft = blank(); }
  async function saveEdit() {
    try { tasks = await taskSave(draft); editing = null; draft = blank(); note = ''; } catch (e) { note = String(e); }
  }
  const projName = (id: string) => projects.find((p) => p.id === id)?.name ?? '';
  const visible = $derived(tasks.filter((t) => !filterProject || t.project === filterProject));
  const open = $derived(visible.filter((t) => !t.done));
  const late = $derived(open.filter((t) => t.due && t.due < today).sort((a, b) => a.due.localeCompare(b.due)));
  const dueToday = $derived(open.filter((t) => t.due === today));
  const soon = $derived(open.filter((t) => t.due > today).sort((a, b) => a.due.localeCompare(b.due)));
  const anytime = $derived(open.filter((t) => !t.due));
  const done = $derived(visible.filter((t) => t.done).sort((a, b) => b.done.localeCompare(a.done)));
  const nice = (d: string) => (d ? new Date(d + 'T12:00:00').toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' }) : '');
  const groups = $derived([
    { name: 'Late', list: late, cls: 'late' }, { name: 'Today', list: dueToday, cls: 'today' },
    { name: 'Coming up', list: soon, cls: '' }, { name: 'Whenever', list: anytime, cls: '' },
  ]);
  const setLinks = (v: Link[]) => (draft = { ...draft, links: v });
</script>

<section>
  <h1>To-do list</h1>
  <p class="sub">Everything that needs doing, in one place. Give a task a day, make it come back on its own (every week, every 6 months), file it under a project, and link it to the item or plan it is about.</p>
  {#if note}<p class="notice" role="status">{note}</p>{/if}

  <div class="panel addbox">
    <div class="row">
      <input class="input grow" placeholder="What needs doing?" bind:value={draft.title} onkeydown={(e) => e.key === 'Enter' && !editing && add()} aria-label="New task" disabled={!!editing} />
      <input class="input" type="date" bind:value={draft.due} aria-label="Day it is due" title="Day it is due" disabled={!!editing} />
      <select class="input" bind:value={draft.every_days} aria-label="Repeat" title="Does it come back?" disabled={!!editing}>
        <option value={0}>Once</option><option value={7}>Every week</option><option value={14}>Every 2 weeks</option><option value={30}>Every month</option>
        <option value={90}>Every 3 months</option><option value={180}>Every 6 months</option><option value={365}>Every year</option>
      </select>
      <select class="input" bind:value={draft.project} aria-label="Project" title="Which project is it part of?" disabled={!!editing}>
        <option value="">No project</option>
        {#each projects.filter((p) => p.status !== 'done') as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
      </select>
      <button class="btn primary" onclick={add} disabled={!!editing}>Add</button>
    </div>
  </div>

  <div class="row filt">
    <label class="muted small">Show <select class="input" bind:value={filterProject} aria-label="Filter by project"><option value="">all tasks</option>{#each projects as p (p.id)}<option value={p.id}>{p.name}</option>{/each}</select></label>
    <span class="muted small">{open.length} open</span>
  </div>

  {#each groups as g (g.name)}
    {#if g.list.length}
      <h2 class={'gh ' + g.cls}>{g.name}</h2>
      <ul class="list">
        {#each g.list as t (t.id)}
          {@render taskRow(t)}
        {/each}
      </ul>
    {/if}
  {/each}
  {#if !open.length}<p class="muted">Nothing is waiting. Add the first thing above.</p>{/if}

  {#if done.length}
    <button class="btn" onclick={() => (showDone = !showDone)}>{showDone ? 'Hide' : 'Show'} finished ({done.length})</button>
    {#if showDone}<ul class="list">{#each done as t (t.id)}{@render taskRow(t)}{/each}</ul>{/if}
  {/if}
</section>

{#snippet taskRow(t: TaskRec)}
  <li class="task" class:isdone={!!t.done}>
    <div class="line">
      <input type="checkbox" checked={!!t.done} onchange={() => tick(t)} aria-label={'Done: ' + t.title} />
      <button class="title" onclick={() => edit(t)} title="Open to change it">{t.title}</button>
      {#if t.due}<span class="chip" class:late={!t.done && t.due < today}>{nice(t.due)}</span>{/if}
      {#if t.every_days}<span class="chip">every {t.every_days === 7 ? 'week' : t.every_days === 30 ? 'month' : t.every_days + ' days'}</span>{/if}
      {#if t.project}<span class="chip proj">{projName(t.project)}</span>{/if}
      <button class="x" aria-label={'Remove ' + t.title} title="Take off the list" onclick={() => remove(t)}>&times;</button>
    </div>
    {#if t.links.length && editing !== t.id}<div class="inl"><Connected links={t.links} /></div>{/if}
    {#if editing === t.id}
      <div class="edit">
        <div class="row">
          <input class="input grow" bind:value={draft.title} aria-label="Task name" />
          <input class="input" type="date" bind:value={draft.due} aria-label="Day it is due" />
          <select class="input" bind:value={draft.every_days} aria-label="Repeat"><option value={0}>Once</option><option value={7}>Every week</option><option value={14}>Every 2 weeks</option><option value={30}>Every month</option><option value={90}>Every 3 months</option><option value={180}>Every 6 months</option><option value={365}>Every year</option></select>
          <select class="input" bind:value={draft.project} aria-label="Project"><option value="">No project</option>{#each projects as p (p.id)}<option value={p.id}>{p.name}</option>{/each}</select>
        </div>
        <textarea class="input" rows="3" bind:value={draft.notes} placeholder="Notes" aria-label="Notes"></textarea>
        <Connected links={draft.links} onchange={setLinks} suggestFrom={{ text: draft.title + ' ' + draft.notes, kind: 'task', id: draft.id }} />
        <div class="row"><button class="btn primary" onclick={saveEdit}>Save</button><button class="btn" onclick={() => { editing = null; draft = blank(); }}>Cancel</button></div>
      </div>
    {/if}
  </li>
{/snippet}

<style>
  .addbox { margin: 12px 0; border-left: 3px solid var(--cat-ops); }
  .grow { flex: 1; min-width: 200px; }
  .filt { margin: 4px 0 8px; justify-content: space-between; }
  .gh { font-size: 14px; margin: 16px 0 6px; color: var(--muted); }
  .gh.late { color: var(--accent); }
  .list { list-style: none; margin: 0 0 8px; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .task { border: 1px solid var(--line); border-left: 3px solid var(--cat-ops); border-radius: 8px; background: var(--panel); padding: 8px 10px; }
  .task.isdone .title { text-decoration: line-through; color: var(--muted); }
  .line { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .title { flex: 1; min-width: 160px; text-align: left; border: 0; background: transparent; font-size: 15px; padding: 2px 0; }
  .title:hover { color: var(--blue); }
  .chip { font-family: var(--mono); font-size: 11px; padding: 2px 8px; border: 1px solid var(--line2); border-radius: 999px; color: var(--muted); }
  .chip.late { border-color: var(--accent-line); color: var(--accent); }
  .chip.proj { border-color: var(--blue-line); color: var(--blue); }
  .x { border: 0; background: transparent; color: var(--muted); font-size: 18px; }
  .x:hover { color: var(--text); }
  .inl { margin: 6px 0 0 28px; }
  .edit { margin: 8px 0 0 28px; display: flex; flex-direction: column; gap: 8px; }
  textarea { width: 100%; resize: vertical; font-family: inherit; }
  .small { font-size: 13px; }
</style>
