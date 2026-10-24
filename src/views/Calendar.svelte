<script lang="ts">
  // The calendar: one month at a time. It shows your own events, the tasks that fall due, things in the Inventory that
  // expire, and plan checks that come due, all on the day they fall. Events can repeat. Saved in user/events.txt.
  import { onMount } from 'svelte';
  import { opsLoad, agendaGet, eventSave, eventDelete, type AgendaItem, type EventRec, type ProjectRec, type Link } from '../lib/api';
  import MonthGrid from '../lib/MonthGrid.svelte';
  import Connected from '../lib/Connected.svelte';
  import { openLink, nav } from '../lib/nav.svelte';

  const pad = (n: number) => String(n).padStart(2, '0');
  const iso = (d: Date) => d.getFullYear() + '-' + pad(d.getMonth() + 1) + '-' + pad(d.getDate());
  let day = $state(iso(new Date()));
  let range = $state({ from: '', to: '' });
  let agenda: AgendaItem[] = $state([]);
  let events: EventRec[] = $state([]);
  let projects: ProjectRec[] = $state([]);
  let overdueTasks: AgendaItem[] = $state([]);
  let note = $state('');
  let editing: EventRec | null = $state(null);

  async function load() {
    if (!range.from) return;
    try {
      const d = await opsLoad();
      events = d.events;
      projects = d.projects;
      // overdue tasks sit on their own due day, not piled on the first day of the grid
      overdueTasks = d.tasks.filter((t) => !t.done && t.due && t.due >= range.from && t.due < d.today).map((t) => ({ date: t.due, kind: 'task', id: t.id, title: t.title, time: '', overdue: true, project: t.project }));
      agenda = (await agendaGet(range.from, range.to)).filter((a) => !a.overdue);
    } catch (e) { note = String(e); }
  }
  function onmonth(from: string, to: string) { range = { from, to }; void load(); }
  onMount(load);
  const all = $derived([...agenda, ...overdueTasks]);
  const marks = $derived.by(() => { const m: Record<string, number> = {}; for (const a of all) m[a.date] = (m[a.date] ?? 0) + 1; return m; });
  const thatDay = $derived(all.filter((a) => a.date === day).sort((a, b) => (a.time || '99').localeCompare(b.time || '99')));
  const niceDay = $derived(new Date(day + 'T12:00:00').toLocaleDateString(undefined, { weekday: 'long', month: 'long', day: 'numeric', year: 'numeric' }));
  const KIND: Record<string, string> = { event: 'Event', task: 'To-do', item: 'Inventory', plan: 'Plan check' };

  function blank(): EventRec { return { id: '', title: '', date: day, time: '', repeat: '', project: '', notes: '', links: [] }; }
  function startNew() { editing = blank(); }
  let want: string | null = null;
  $effect(() => { const o = nav.open; if (o?.kind === 'event') want = o.id; });
  $effect(() => { const e = want ? events.find((x) => x.id === want) : null; if (e) { editing = structuredClone(e); day = e.date; want = null; } });
  function open(a: AgendaItem) {
    if (a.kind === 'event') { const e = events.find((x) => x.id === a.id); if (e) editing = structuredClone({ ...e, date: e.repeat ? e.date : e.date }); }
    else openLink(a.kind, a.id);
  }
  async function save() {
    if (!editing) return;
    try { events = await eventSave(editing); editing = null; note = ''; await load(); } catch (e) { note = String(e); }
  }
  async function remove() {
    if (!editing?.id || !confirm('Take "' + editing.title + '" off the calendar? If it repeats, every day it falls on goes.')) return;
    try { events = await eventDelete(editing.id); editing = null; await load(); } catch (e) { note = String(e); }
  }
  const setLinks = (v: Link[]) => { if (editing) editing = { ...editing, links: v }; };
</script>

<section>
  <h1>Calendar</h1>
  <p class="sub">Your events, tasks that fall due, things that expire and plan checks, all on their days. Pick a day to see what is on it or to add an event.</p>
  {#if note}<p class="notice" role="status">{note}</p>{/if}

  <div class="wrap">
    <div class="panel"><MonthGrid selected={day} {marks} onpick={(d) => (day = d)} {onmonth} /></div>
    <div class="panel side">
      <div class="label">{niceDay}</div>
      {#if !editing}
        {#if thatDay.length}
          <ul class="list">
            {#each thatDay as a (a.kind + a.id + a.date)}
              <li><button onclick={() => open(a)}><span class="k">{KIND[a.kind] ?? a.kind}</span> {#if a.time}<b>{a.time}</b>{/if} {a.title}{#if a.overdue} <em>late</em>{/if}</button></li>
            {/each}
          </ul>
        {:else}<p class="muted">Nothing on this day.</p>{/if}
        <button class="btn primary" onclick={startNew}>+ Add an event</button>
      {:else}
        <div class="form">
          <input class="input" placeholder="What is happening?" bind:value={editing.title} aria-label="Event name" />
          <div class="row">
            <input class="input" type="date" bind:value={editing.date} aria-label="Day" />
            <input class="input" type="time" bind:value={editing.time} aria-label="Time (leave empty for all day)" />
            <select class="input" bind:value={editing.repeat} aria-label="Repeat"><option value="">Once</option><option value="weekly">Every week</option><option value="monthly">Every month</option><option value="yearly">Every year</option></select>
          </div>
          <select class="input" bind:value={editing.project} aria-label="Project"><option value="">No project</option>{#each projects as p (p.id)}<option value={p.id}>{p.name}</option>{/each}</select>
          <textarea class="input" rows="3" placeholder="Notes" bind:value={editing.notes} aria-label="Notes"></textarea>
          <Connected links={editing.links} onchange={setLinks} suggestFrom={{ text: editing.title + ' ' + editing.notes, kind: 'event', id: editing.id }} />
          <div class="row">
            <button class="btn primary" onclick={save}>Save</button>
            <button class="btn" onclick={() => (editing = null)}>Cancel</button>
            {#if editing.id}<button class="btn" onclick={remove}>Delete</button>{/if}
          </div>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .wrap { display: grid; grid-template-columns: 1fr 340px; gap: 14px; margin-top: 12px; align-items: start; }
  .panel { border-left: 3px solid var(--cat-ops); }
  .side { display: flex; flex-direction: column; gap: 10px; }
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .list button { width: 100%; text-align: left; border: 1px solid var(--line2); border-radius: 8px; background: var(--panel2); padding: 7px 10px; }
  .list button:hover { border-color: var(--blue-line); }
  .k { font-family: var(--mono); font-size: 10px; text-transform: uppercase; letter-spacing: .08em; color: var(--muted); margin-right: 6px; }
  em { color: var(--accent); font-style: normal; font-size: 12px; }
  .form { display: flex; flex-direction: column; gap: 8px; }
  textarea { width: 100%; resize: vertical; font-family: inherit; }
  @media (max-width: 860px) { .wrap { grid-template-columns: 1fr; } }
</style>
