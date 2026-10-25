<script lang="ts">
  // The calendar, laid out like the calendars people already know: a big month (or week) with each day's things written
  // in it. It shows your own events, birthdays from the Family page, the tasks that fall due, things in the Inventory
  // that expire, plan checks, planting times from the Seed vault, and the usual frost days, all on the day they fall.
  // Events can repeat. Saved in user/events.txt.
  import { opsLoad, agendaGet, eventSave, eventDelete, type AgendaItem, type EventRec, type ProjectRec, type Link } from '../lib/api';
  import Connected from '../lib/Connected.svelte';
  import { openLink, nav } from '../lib/nav.svelte';

  const pad = (n: number) => String(n).padStart(2, '0');
  const iso = (d: Date) => d.getFullYear() + '-' + pad(d.getMonth() + 1) + '-' + pad(d.getDate());
  const parse = (s: string) => new Date(s + 'T12:00:00');
  const todayIso = iso(new Date());

  let view: 'month' | 'week' = $state('month');
  /** The day that is picked. The grid shows the month, or the week, it is in. */
  let day = $state(todayIso);
  let agenda: AgendaItem[] = $state([]);
  let events: EventRec[] = $state([]);
  let projects: ProjectRec[] = $state([]);
  let overdueTasks: AgendaItem[] = $state([]);
  let note = $state('');
  let editing: EventRec | null = $state(null);
  let loadN = 0;

  const days = $derived.by(() => {
    const sel = parse(day);
    const from = view === 'week'
      ? new Date(sel.getFullYear(), sel.getMonth(), sel.getDate() - sel.getDay(), 12)
      : (() => { const first = new Date(sel.getFullYear(), sel.getMonth(), 1, 12); return new Date(first.getFullYear(), first.getMonth(), 1 - first.getDay(), 12); })();
    return Array.from({ length: view === 'week' ? 7 : 42 }, (_, i) => {
      const d = new Date(from.getFullYear(), from.getMonth(), from.getDate() + i, 12);
      return { iso: iso(d), n: d.getDate(), other: view === 'month' && d.getMonth() !== sel.getMonth() };
    });
  });
  const range = $derived({ from: days[0].iso, to: days[days.length - 1].iso });
  const title = $derived.by(() => {
    const sel = parse(day);
    if (view === 'month') return sel.toLocaleDateString(undefined, { month: 'long', year: 'numeric' });
    const a = parse(days[0].iso), b = parse(days[6].iso);
    return a.toLocaleDateString(undefined, { month: 'short', day: 'numeric' }) + ' – ' + b.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
  });

  async function load(from: string, to: string) {
    const n = ++loadN;
    try {
      const d = await opsLoad();
      // overdue tasks sit on their own due day, not piled on the first day of the grid
      const late = d.tasks.filter((t) => !t.done && t.due && t.due >= from && t.due < d.today).map((t) => ({ date: t.due, kind: 'task', id: t.id, title: t.title, time: '', overdue: true, project: t.project }));
      const list = (await agendaGet(from, to)).filter((a) => !a.overdue);
      if (n !== loadN) return; // a newer month was asked for meanwhile
      events = d.events; projects = d.projects; overdueTasks = late; agenda = list;
      note = '';
    } catch (e) { note = String(e); }
  }
  $effect(() => { const r = range; void load(r.from, r.to); });

  const all = $derived([...agenda, ...overdueTasks]);
  const byDay = $derived.by(() => {
    const m: Record<string, AgendaItem[]> = {};
    for (const a of all) (m[a.date] ??= []).push(a);
    for (const k of Object.keys(m)) m[k].sort((a, b) => (a.time || '00').localeCompare(b.time || '00') || a.title.localeCompare(b.title));
    return m;
  });
  const thatDay = $derived(byDay[day] ?? []);
  const niceDay = $derived(parse(day).toLocaleDateString(undefined, { weekday: 'long', month: 'long', day: 'numeric', year: 'numeric' }));
  const KIND: Record<string, string> = { event: 'Event', task: 'To-do', item: 'Inventory', plan: 'Plan check', seed: 'Planting', season: 'Frost' };
  const isBirthday = (a: AgendaItem) => a.kind === 'event' && /birthday$/i.test(a.title.trim());
  const cls = (a: AgendaItem) => (isBirthday(a) ? 'bday' : a.kind);
  const label = (a: AgendaItem) => (isBirthday(a) ? 'Birthday' : KIND[a.kind] ?? a.kind);
  const LEGEND: [string, string][] = [['event', 'Events'], ['bday', 'Birthdays'], ['task', 'To-do'], ['item', 'Inventory'], ['plan', 'Plan checks'], ['seed', 'Planting'], ['season', 'Frost']];
  const SHOW = $derived(view === 'week' ? 12 : 3);

  function step(k: number) {
    const sel = parse(day);
    if (view === 'week') { day = iso(new Date(sel.getFullYear(), sel.getMonth(), sel.getDate() + 7 * k, 12)); return; }
    const last = new Date(sel.getFullYear(), sel.getMonth() + k + 1, 0, 12).getDate();
    day = iso(new Date(sel.getFullYear(), sel.getMonth() + k, Math.min(sel.getDate(), last), 12));
  }

  function blank(): EventRec { return { id: '', title: '', date: day, time: '', repeat: '', project: '', notes: '', links: [] }; }
  function startNew() { editing = blank(); }
  let want: string | null = $state(null);
  $effect(() => { const o = nav.open; if (o?.kind === 'event') want = o.id; });
  $effect(() => { const e = want ? events.find((x) => x.id === want) : null; if (e) { editing = $state.snapshot(e) as EventRec; day = e.date; want = null; } });
  function open(a: AgendaItem) {
    day = a.date;
    if (a.kind === 'event') { const e = events.find((x) => x.id === a.id); if (e) editing = $state.snapshot(e) as EventRec; }
    else if (a.kind !== 'season') openLink(a.kind, a.id);
  }
  async function save() {
    if (!editing) return;
    try { events = await eventSave($state.snapshot(editing) as EventRec); editing = null; note = ''; await load(range.from, range.to); } catch (e) { note = String(e); }
  }
  async function remove() {
    if (!editing?.id || !confirm('Take "' + editing.title + '" off the calendar? If it repeats, every day it falls on goes.')) return;
    try { events = await eventDelete(editing.id); editing = null; await load(range.from, range.to); } catch (e) { note = String(e); }
  }
  const setLinks = (v: Link[]) => { if (editing) editing = { ...editing, links: v }; };
</script>

<section>
  <h1>Calendar</h1>
  <p class="sub">Your events, birthdays, tasks that fall due, things that expire, plan checks, planting times and the usual frost days, all on their days. Click a day to see it, double-click a day to add an event.</p>
  {#if note}<p class="notice" role="status">{note}</p>{/if}

  <div class="wrap">
    <div class="panel cal">
      <div class="bar">
        <button class="btn" title="Go to today" onclick={() => (day = todayIso)}>Today</button>
        <button class="btn" aria-label={view === 'week' ? 'Last week' : 'Last month'} onclick={() => step(-1)}>&lsaquo;</button>
        <button class="btn" aria-label={view === 'week' ? 'Next week' : 'Next month'} onclick={() => step(1)}>&rsaquo;</button>
        <h2 class="ttl">{title}</h2>
        <div class="seg" role="group" aria-label="Show">
          <button class:on={view === 'month'} aria-pressed={view === 'month'} onclick={() => (view = 'month')}>Month</button>
          <button class:on={view === 'week'} aria-pressed={view === 'week'} onclick={() => (view = 'week')}>Week</button>
        </div>
        <button class="btn primary" title="Add an event on the picked day" onclick={startNew}>+ Add an event</button>
      </div>
      <div class="legend" aria-label="What the colours mean">
        {#each LEGEND as [k, name] (k)}<span class="lg"><i class={'sw ' + k}></i>{name}</span>{/each}
      </div>
      <div class="grid" class:week={view === 'week'} role="grid" aria-label={title}>
        {#each ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'] as w}<div class="wd" role="columnheader">{w}</div>{/each}
        {#each days as d (d.iso)}
          {@const items = byDay[d.iso] ?? []}
          <div class="cell" class:other={d.other} class:today={d.iso === todayIso} class:sel={d.iso === day} role="presentation"
            onclick={() => (day = d.iso)} ondblclick={() => { day = d.iso; startNew(); }}>
            <button class="num" aria-label={'Pick ' + d.iso + (items.length ? ', ' + items.length + ' things' : '')} onclick={(e) => { e.stopPropagation(); day = d.iso; }}>{d.n}</button>
            <div class="chips">
              {#each items.slice(0, SHOW) as a (a.kind + a.id + a.date)}
                <button class={'chip ' + cls(a)} title={label(a) + ': ' + a.title + (a.overdue ? ' (late)' : '')} onclick={(e) => { e.stopPropagation(); open(a); }}>{#if a.time}<b>{a.time}</b> {/if}{a.title}</button>
              {/each}
              {#if items.length > SHOW}<button class="more" onclick={(e) => { e.stopPropagation(); day = d.iso; }}>+{items.length - SHOW} more</button>{/if}
            </div>
          </div>
        {/each}
      </div>
    </div>

    <div class="panel side">
      <div class="label">{niceDay}</div>
      {#if !editing}
        {#if thatDay.length}
          <ul class="list">
            {#each thatDay as a (a.kind + a.id + a.date)}
              <li><button onclick={() => open(a)}><i class={'sw ' + cls(a)}></i><span class="k">{label(a)}</span> {#if a.time}<b>{a.time}</b>{/if} {a.title}{#if a.overdue} <em>late</em>{/if}</button></li>
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
  .wrap { display: grid; grid-template-columns: minmax(0, 1fr) 320px; gap: 14px; margin-top: 12px; align-items: start; }
  .panel { border-left: 3px solid var(--cat-ops); }
  .cal { display: flex; flex-direction: column; gap: 10px; min-width: 0; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .ttl { flex: 1; min-width: 160px; margin: 0 6px; font-family: var(--display); font-size: 22px; letter-spacing: .04em; text-transform: uppercase; }
  .seg { display: inline-flex; border: 1px solid var(--line2); border-radius: 8px; overflow: hidden; }
  .seg button { border: 0; background: var(--panel2); color: inherit; padding: 6px 14px; cursor: pointer; }
  .seg button.on { background: var(--blue-soft); color: var(--blue); }
  .legend { display: flex; flex-wrap: wrap; gap: 4px 14px; font-size: 12px; color: var(--muted); }
  .lg { display: inline-flex; align-items: center; gap: 6px; }
  .sw { display: inline-block; width: 10px; height: 10px; border-radius: 3px; background: var(--blue); flex: none; }
  .sw.task { background: var(--accent); } .sw.item { background: var(--danger); } .sw.plan { background: var(--cat-intel); }
  .sw.seed { background: var(--ok); } .sw.season { background: var(--cat-comms); } .sw.bday { background: var(--cat-morale); }
  .grid { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 4px; }
  .wd { text-align: center; font-family: var(--mono); font-size: 12px; color: var(--muted); padding: 4px 0; text-transform: uppercase; letter-spacing: .08em; }
  .cell { min-height: 118px; padding: 4px; border: 1px solid var(--line); border-radius: 8px; background: var(--panel); display: flex; flex-direction: column; gap: 3px; min-width: 0; cursor: pointer; }
  .grid.week .cell { min-height: 360px; }
  .cell.other { opacity: .5; }
  .cell.today { border-color: var(--accent-line); }
  .cell.sel { border-color: var(--blue-line); background: var(--blue-soft); }
  .cell:hover { border-color: var(--blue-line); }
  .num { align-self: flex-start; border: 0; background: transparent; color: inherit; font-size: 13px; min-width: 26px; height: 26px; border-radius: 13px; padding: 0 6px; cursor: pointer; }
  .cell.today .num { background: var(--accent); color: #111; font-weight: 700; }
  .chips { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .chip { text-align: left; font-size: 12px; line-height: 1.25; padding: 2px 6px; border: 0; border-left: 3px solid var(--blue); border-radius: 4px; background: var(--blue-soft); color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; cursor: pointer; }
  .chip.task { border-left-color: var(--accent); background: var(--accent-soft); }
  .chip.item { border-left-color: var(--danger); background: #e5533d22; }
  .chip.plan { border-left-color: var(--cat-intel); background: #4a90d022; }
  .chip.seed { border-left-color: var(--ok); background: #6fbf7322; }
  .chip.season { border-left-color: var(--cat-comms); background: #9b79c922; cursor: default; }
  .chip.bday { border-left-color: var(--cat-morale); background: #d9779b22; }
  .chip:hover { filter: brightness(1.25); }
  .more { text-align: left; border: 0; background: transparent; color: var(--muted); font-size: 12px; padding: 0 6px; cursor: pointer; }
  .side { display: flex; flex-direction: column; gap: 10px; position: sticky; top: 8px; }
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .list button { width: 100%; text-align: left; border: 1px solid var(--line2); border-radius: 8px; background: var(--panel2); padding: 7px 10px; display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
  .list button:hover { border-color: var(--blue-line); }
  .k { font-family: var(--mono); font-size: 10px; text-transform: uppercase; letter-spacing: .08em; color: var(--muted); }
  em { color: var(--accent); font-style: normal; font-size: 12px; }
  .form { display: flex; flex-direction: column; gap: 8px; }
  textarea { width: 100%; resize: vertical; font-family: inherit; }
  @media (max-width: 1100px) { .wrap { grid-template-columns: 1fr; } .side { position: static; } }
  @media (max-width: 640px) { .cell { min-height: 74px; } .chip { font-size: 11px; } }
</style>
