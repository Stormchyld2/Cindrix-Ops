<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { drillsList, drillSave, drillCreate, eventSave, opsLoad, type Drill, type EventRec } from '../lib/api';
  import PageHeader from '../lib/PageHeader.svelte';
  import Ring from '../lib/Ring.svelte';
  import Icon from '../lib/Icon.svelte';

  let { go }: { go?: (tab: string) => void } = $props();

  let drills: Drill[] = $state([]);
  let error = $state('');
  let saved = $state('');

  // The drill being walked through
  let active = $state<Drill | null>(null);
  let checked: boolean[] = $state([]);
  let seconds = $state(0);
  let paused = $state(false);
  let notes = $state('');
  let tick: ReturnType<typeof setInterval> | undefined;

  async function load() {
    try {
      drills = await drillsList();
    } catch (e) {
      error = String(e);
    }
  }
  onMount(load);
  onDestroy(() => { if (tick) clearInterval(tick); });

  const mmss = (s: number) => `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
  let doneCount = $derived(checked.filter(Boolean).length);
  // Drills that were started and not yet saved or ended. Kept for this window only (not a result, so it is not in the log).
  const RUN_KEY = 'cindrix.drills.running';
  let running: string[] = $state((() => { try { return JSON.parse(localStorage.getItem(RUN_KEY) || '[]'); } catch { return []; } })());
  function setRunning(ids: string[]) { running = ids; try { localStorage.setItem(RUN_KEY, JSON.stringify(ids)); } catch { /* fine */ } }
  const isRunning = (d: Drill) => running.includes(d.id);
  const inProgressCount = $derived(drills.filter(isRunning).length);
  const notStartedCount = $derived(drills.filter((d) => d.times_done === 0 && !isRunning(d)).length);
  let view = $state<'library' | 'history'>('library');
  const runs = $derived(drills.flatMap((d) => d.history.map((h) => ({ ...h, drill: d.title }))).sort((a, b) => b.date.localeCompare(a.date)));
  const totalSeconds = $derived(runs.reduce((n, r) => n + r.seconds, 0));
  // make a drill
  let mkDlg: HTMLDialogElement;
  let mk = $state({ title: '', minutes: 15, place: 'at home', summary: '', steps: '' });
  let mkNote = $state('');
  async function createDrill() {
    mkNote = '';
    try {
      await drillCreate(mk.title, Number(mk.minutes) || 0, mk.place, mk.summary, mk.steps.split('\n'));
      mkDlg.close();
      mk = { title: '', minutes: 15, place: 'at home', summary: '', steps: '' };
      saved = 'Your drill was saved in your own drills folder.';
      await load();
    } catch (e) { mkNote = String(e); }
  }
  // put a drill on the calendar
  let calDlg: HTMLDialogElement;
  let sched = $state({ drill: '', date: new Date(Date.now() + 7 * 86400000).toISOString().slice(0, 10), repeat: '' });
  let scheduled: EventRec[] = $state([]);
  let calNote = $state('');
  async function openCal() {
    calNote = '';
    sched.drill = sched.drill || drills[0]?.id || '';
    try { scheduled = (await opsLoad()).events.filter((e) => e.title.startsWith('Drill: ')).sort((a, b) => a.date.localeCompare(b.date)); } catch { scheduled = []; }
    calDlg.showModal();
  }
  async function addToCalendar() {
    const d = drills.find((x) => x.id === sched.drill);
    if (!d) { calNote = 'Choose a drill first.'; return; }
    try {
      await eventSave({ id: '', title: 'Drill: ' + d.title, date: sched.date, time: '', repeat: sched.repeat, project: '', notes: d.summary, links: [] });
      calNote = 'Added to the Calendar.';
      scheduled = (await opsLoad()).events.filter((e) => e.title.startsWith('Drill: ')).sort((a, b) => a.date.localeCompare(b.date));
    } catch (e) { calNote = String(e); }
  }
  let filter = $state('all');
  let q = $state('');
  let listView = $state(false);
  const doneDrills = $derived(drills.filter((d) => d.times_done > 0).length);
  const totalRuns = $derived(drills.reduce((n, d) => n + d.times_done, 0));
  const nextUndone = $derived(drills.find((d) => d.times_done === 0));
  const shown = $derived(drills.filter((d) => (filter === 'all' || (filter === 'prog' ? isRunning(d) : filter === 'done' ? d.times_done > 0 : d.times_done === 0 && !isRunning(d))) && (!q.trim() || (d.title + ' ' + d.summary + ' ' + d.place).toLowerCase().includes(q.trim().toLowerCase()))));
  let nextStep = $derived(checked.indexOf(false) === -1 ? checked.length : checked.indexOf(false) + 1);

  function start(d: Drill) {
    setRunning([...new Set([...running, d.id])]);
    active = d;
    checked = d.steps.map(() => false);
    seconds = 0;
    paused = false;
    notes = '';
    saved = '';
    if (tick) clearInterval(tick);
    tick = setInterval(() => { if (!paused) seconds++; }, 1000);
  }

  function stop() {
    if (tick) clearInterval(tick);
    if (active) setRunning(running.filter((id) => id !== active!.id));
    active = null;
  }

  async function finish() {
    if (!active) return;
    try {
      await drillSave(active.id, seconds, doneCount, checked.length, notes);
      saved = `Saved: ${active.title}, ${doneCount} of ${checked.length} steps in ${mmss(seconds)}.`;
      stop();
      await load();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<section class="wrap">
  {#if !active}
    <PageHeader icon="clipboard" title="Drills" tagline="Practice • Prepare • Build Skill • Be Ready" />
    <div class="top3">
      <section class="dbl p1" style="--c: var(--glow-blue)">
        <span class="bt"><Icon name="target" size={46} /></span>
        <div class="p1t"><div class="pnl-h nosl">Drill readiness</div><div class="bigamber">{doneDrills} of {drills.length}</div><div class="sm">Drills Completed</div></div>
        <Ring value={drills.length ? (doneDrills / drills.length) * 100 : 0} size={118} />
      </section>
      <section class="dbl p2" style="--c: var(--glow-blue)">
        <div class="p2h"><div class="pnl-h nosl"><Icon name="target" size={22} /> Drill progress</div><span class="sm">{doneDrills} / {drills.length} Complete</span></div>
        <div class="bar"><span style="width:{drills.length ? (doneDrills / drills.length) * 100 : 0}%"></span></div>
        <div class="mini4">
          <div class="mb"><Icon name="play" size={34} /><span><b>{drills.length}</b><i>Total Drills</i></span></div>
          <div class="mb"><Icon name="hourglass" size={34} /><span><b>{inProgressCount}</b><i>In Progress</i></span></div>
          <div class="mb ok"><Icon name="check" size={34} /><span><b>{doneDrills}</b><i>Completed</i></span></div>
          <div class="mb"><Icon name="calx" size={34} /><span><b>{notStartedCount}</b><i>Not Started</i></span></div>
        </div>
      </section>
      <section class="dbl p3" style="--c: var(--glow-blue)">
        <div class="pnl-h nosl"><Icon name="bolt" size={22} /> Quick actions</div>
        <button class="btn primary ico qa" onclick={() => { mkNote = ''; mkDlg.showModal(); }} title="Write your own drill"><Icon name="plus" size={18} /> Create drill</button>
        <button class="btn ico qa" onclick={openCal} title="Put a drill on the Calendar"><Icon name="cal" size={18} /> Drill calendar</button>
        <button class="btn ico qa" onclick={() => (view = 'history')} title="Every drill you have done"><Icon name="chart" size={18} /> View progress</button>
      </section>
    </div>
    {#if view === 'history'}
    <section class="dbl lib" style="--c: var(--glow-blue)">
      <div class="libh">
        <div class="pnl-h nosl"><Icon name="chart" size={24} /> Your drill history</div>
        <button class="btn ico" onclick={() => (view = 'library')}>← Back to the drills</button>
      </div>
      <p class="sm hsum">{runs.length} time{runs.length === 1 ? '' : 's'} done, {mmss(totalSeconds)} in all.</p>
      {#if runs.length === 0}<p class="sm">Nothing yet. Do a drill and save the result, and it shows here.</p>{/if}
      {#each runs as r}
        <div class="hrow"><span class="hd">{r.date}</span><strong class="ht">{r.drill}</strong><span class="hs">{r.done} of {r.total} steps</span><span class="hm">{mmss(r.seconds)}</span>{#if r.notes}<span class="hn">Missing: {r.notes}</span>{/if}</div>
      {/each}
    </section>
    {:else}
    <section class="dbl lib" style="--c: var(--glow-blue)">
      <div class="libh">
        <div class="pnl-h nosl"><Icon name="target" size={24} /> Drill library ({drills.length})</div>
        <div class="tabs" role="tablist" aria-label="Show">
          {#each [['all', `All Drills`], ['todo', `Not Started (${notStartedCount})`], ['prog', `In Progress (${inProgressCount})`], ['done', `Completed (${doneDrills})`]] as [k, t]}
            <button class="tab" class:on={filter === k} role="tab" aria-selected={filter === k} onclick={() => (filter = k)}>{t}</button>
          {/each}
        </div>
        <label class="srch"><Icon name="search" size={18} /><input type="search" placeholder="Search drills…" aria-label="Search drills" bind:value={q} /></label>
        <div class="vt"><button class:on={!listView} aria-label="Cards" title="Cards" onclick={() => (listView = false)}><Icon name="grid" size={20} /></button><button class:on={listView} aria-label="List" title="List" onclick={() => (listView = true)}><Icon name="list" size={20} /></button></div>
      </div>
    {#if error}<div class="warn">{error}</div>{/if}
    {#if saved}<div class="done">{saved}</div>{/if}
    <div class={listView ? 'list' : 'grid'} style="margin-top: 12px">
      {#each shown as d (d.id)}
        <article class="drill" style="--c: {isRunning(d) ? 'var(--glow-amber)' : d.times_done > 0 ? 'var(--glow-green)' : 'var(--glow-blue)'}">
          <div class="thumb" style="background-image:url(/ui/drill-{d.id}.jpg), url(/ui/banner.jpg)"></div>
          <div class="dbody">
            <div class="meta">
              <span class="st"><i class="dot"></i>{isRunning(d) ? 'In progress' : d.times_done > 0 ? `Done ${d.times_done}×` : 'Not done yet'}</span>
              {#if d.status !== 'reviewed'}<span class="draft">Draft</span>{/if}
            </div>
            <h2>{d.title}</h2>
            <p>{d.summary}</p>
            <div class="foot">
              <span class="fine"><Icon name="hourglass" size={14} /> {d.minutes} min <b>|</b> <Icon name="list" size={14} /> {d.steps.length} steps</span>
              <button class="btn primary sm" onclick={() => start(d)}>{isRunning(d) ? 'Start again' : d.times_done > 0 ? 'Do it again' : 'Start drill'}</button>
            </div>
            {#if d.last}<p class="fine last">Last: {d.last.date}, {d.last.done} of {d.last.total} steps in {mmss(d.last.seconds)}.{d.last.notes ? ' Missing: ' + d.last.notes : ''}</p>{/if}
          </div>
        </article>
      {/each}
    </div>
    </section>
    {/if}
    {#if drills.length === 0 && !error}<p class="muted">No drills found in the content folder.</p>{/if}
  {:else}
    <div class="panel run">
      <div class="head">
        <div>
          <div class="label">{active.title.toUpperCase()} · STEP {nextStep} OF {checked.length}</div>
          <div class="timer" aria-live="off">{mmss(seconds)}</div>
        </div>
        <div class="row">
          <button class="btn" onclick={() => (paused = !paused)}>{paused ? 'Resume' : 'Pause'}</button>
        </div>
      </div>
      {#if active.status !== 'reviewed'}<div class="warn">Draft checklist. Check it against your own situation and your doctor's written plans.</div>{/if}
      <ul class="steps">
        {#each active.steps as s, i}
          <li>
            <label class:ticked={checked[i]}>
              <input type="checkbox" bind:checked={checked[i]} />
              <span>{s}</span>
            </label>
          </li>
        {/each}
      </ul>
      <label class="debrief">Debrief: what was missing?
        <textarea class="input" rows="3" bind:value={notes} placeholder="Batteries for the lantern, a can opener, more water…"></textarea>
      </label>
      {#if error}<div class="warn">{error}</div>{/if}
      <div class="row end">
        <button class="btn" onclick={stop}>End drill without saving</button>
        <button class="btn primary" onclick={finish}>Save result ({doneCount} of {checked.length})</button>
      </div>
    </div>
  {/if}
</section>

<dialog bind:this={mkDlg} aria-label="Create a drill">
  <form class="mkform" onsubmit={(e) => { e.preventDefault(); createDrill(); }}>
    <h2>Create a drill</h2>
    <p class="muted small">A drill is a short checklist you walk through with a timer. It is saved as a plain file in your own drills folder, and it shows in the library with the others.</p>
    <label>Name<input class="input" bind:value={mk.title} required placeholder="Power out at the cabin" /></label>
    <div class="row"><label class="half">Minutes<input class="input" type="number" min="1" max="600" bind:value={mk.minutes} /></label><label class="half">Where<input class="input" bind:value={mk.place} /></label></div>
    <label>What it is for<input class="input" bind:value={mk.summary} placeholder="One sentence" /></label>
    <label>Steps, one on each line<textarea class="input" rows="6" bind:value={mk.steps} required placeholder={'Find the flashlights\nCount the water'}></textarea></label>
    <p class="muted small">Do not put medicine doses in a drill. Follow your doctor's written plan.</p>
    {#if mkNote}<div class="warn" role="alert">{mkNote}</div>{/if}
    <div class="row end"><button class="btn" type="button" onclick={() => mkDlg.close()}>Cancel</button><button class="btn primary" type="submit">Save drill</button></div>
  </form>
</dialog>

<dialog bind:this={calDlg} aria-label="Drill calendar">
  <div class="mkform">
    <h2>Drill calendar</h2>
    <p class="muted small">Pick a drill and a day, and it goes on the Calendar, once or repeating, so you remember to practise.</p>
    <label>Drill<select class="input" bind:value={sched.drill}>{#each drills as d}<option value={d.id}>{d.title}</option>{/each}</select></label>
    <div class="row"><label class="half">Day<input class="input" type="date" bind:value={sched.date} /></label><label class="half">Repeat<select class="input" bind:value={sched.repeat}><option value="">Once</option><option value="weekly">Every week</option><option value="monthly">Every month</option><option value="yearly">Every year</option></select></label></div>
    {#if calNote}<p class="small" role="status">{calNote}</p>{/if}
    <div class="row end"><button class="btn primary" onclick={addToCalendar}>Add to the Calendar</button></div>
    <div class="label">DRILLS ON THE CALENDAR</div>
    {#if scheduled.length === 0}<p class="muted small">None yet.</p>{/if}
    {#each scheduled as e (e.id)}<div class="hrow"><span class="hd">{e.date}</span><strong class="ht">{e.title.replace('Drill: ', '')}</strong><span class="hs">{e.repeat === 'weekly' ? 'every week' : e.repeat === 'monthly' ? 'every month' : e.repeat === 'yearly' ? 'every year' : 'once'}</span></div>{/each}
    <div class="row end"><button class="btn" onclick={() => { calDlg.close(); go?.('Calendar'); }}>Open the Calendar</button><button class="btn" onclick={() => calDlg.close()}>Close</button></div>
  </div>
</dialog>

<style>
  .mkform { display: flex; flex-direction: column; gap: 10px; padding: 18px; min-width: min(520px, 90vw); }
  .mkform label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--muted); }
  .mkform h2 { margin: 0; }
  .half { flex: 1 1 140px; }
  .end { justify-content: flex-end; }
  .hsum { margin: 8px 0; }
  .hrow { display: flex; flex-wrap: wrap; gap: 4px 16px; align-items: baseline; padding: 10px 4px; border-top: 1px solid #1d3550; }
  .hd { font-family: var(--mono); color: var(--glow-blue); min-width: 92px; }
  .ht { flex: 1 1 200px; }
  .hs, .hm { color: #e6edf5; }
  .hn { flex-basis: 100%; color: #9fb4c9; font-size: 13px; }
  .top3 { display: grid; grid-template-columns: 1.15fr 1.7fr 0.9fr; gap: 14px; }
  @media (max-width: 1100px) { .top3 { grid-template-columns: 1fr; } }
  .top3 > section { padding: 14px 18px; }
  .p1 { display: flex; align-items: center; gap: 14px; }
  .bt { color: var(--glow-blue); filter: drop-shadow(0 0 6px rgba(61, 155, 255, .6)); }
  .p1t { flex: 1; }
  .bigamber { font-size: 40px; font-weight: 700; color: var(--glow-amber); line-height: 1; }
  .sm { font-size: 14px; color: #dfe7f0; }
  .p2h { display: flex; justify-content: space-between; align-items: center; }
  .bar { height: 10px; border-radius: 6px; background: #1b2735; overflow: hidden; margin: 4px 0 12px; }
  .bar span { display: block; height: 100%; background: linear-gradient(90deg, #2f86e8, #4db0ff); box-shadow: 0 0 10px #3d9bff; }
  .mini4 { display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px; }
  .mb { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border: 1px solid #2a4a70; border-radius: 10px; background: #0b121b; color: var(--glow-blue); }
  .mb.ok { color: var(--glow-green); border-color: #2c6a40; }
  .mb b { display: block; font-size: 24px; color: #fff; line-height: 1; }
  .mb i { font-style: normal; font-size: 12px; color: #cfd8e3; }
  .p3 { display: flex; flex-direction: column; gap: 8px; }
  .qa { width: 100%; justify-content: center; }
  .lib { padding: 14px 16px 16px; }
  .libh { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; }
  .libh .pnl-h { margin: 0; flex: 1 1 200px; }
  .tabs { display: inline-flex; gap: 8px; }
  .tab { padding: 8px 16px; border-radius: 10px; background: #0d1824; border: 1px solid #2a4a70; color: #dfe7f0; font-size: 14px; }
  .tab.on { background: linear-gradient(180deg, #38a4ff, #1c7fe0); border-color: #4db0ff; color: #fff; font-weight: 600; box-shadow: 0 0 12px rgba(56, 164, 255, .5); }
  .srch { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 40px; border: 1px solid #2a4a70; border-radius: 10px; background: #0a121b; color: #9fb4c9; min-width: 260px; }
  .srch input { flex: 1; background: transparent; border: 0; outline: 0; color: var(--text); font: inherit; }
  .vt { display: inline-flex; border-radius: 10px; overflow: hidden; border: 1px solid #2a4a70; }
  .vt button { width: 42px; height: 40px; display: inline-flex; align-items: center; justify-content: center; background: #0d1824; border: 0; color: #cfd8e3; }
  .vt button.on { background: linear-gradient(180deg, #38a4ff, #1c7fe0); color: #fff; }
  .drill { display: flex; min-height: 124px; border: 1.5px solid color-mix(in srgb, var(--c) 60%, #000); border-radius: 12px; overflow: hidden; background: linear-gradient(135deg, color-mix(in srgb, var(--c) 8%, #0d1620), #0a1018); box-shadow: 0 0 14px color-mix(in srgb, var(--c) 22%, transparent); }
  .thumb { flex: 0 0 34%; max-width: 150px; background-size: cover; background-position: center; border-right: 1px solid #1d3550; }
  .dbody { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; padding: 8px 12px 10px; }
  .meta { display: flex; justify-content: space-between; align-items: center; font-family: var(--mono); font-size: 11px; letter-spacing: .1em; text-transform: uppercase; color: #dfe7f0; }
  .st .dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; margin-right: 7px; background: var(--c); box-shadow: 0 0 6px var(--c); }
  .draft { padding: 1px 10px; border-radius: 999px; border: 1px solid var(--glow-amber); color: var(--glow-amber); font-size: 10px; font-weight: 700; }
  .drill h2 { margin: 2px 0 0; font-size: 17px; letter-spacing: .04em; text-transform: uppercase; }
  .drill p { margin: 0; font-size: 13.5px; color: #e3eaf2; }
  .drill .foot { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-top: auto; }
  .fine { font-size: 12px; color: #cfd8e3; display: inline-flex; align-items: center; gap: 5px; flex-wrap: wrap; }
  .fine b { color: #4a6a8a; margin: 0 4px; font-weight: 400; }
  .drill .last { display: block; color: #9fb4c9; }
  .btn.sm { padding: 6px 14px; }
  .list .drill { min-height: 0; }
  .list .thumb { flex-basis: 90px; }
  .wrap { display: flex; flex-direction: column; gap: 14px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 14px; }
  .list { display: flex; flex-direction: column; gap: 10px; }
  .ready { display: flex; align-items: center; gap: 16px; }
  .ready p { margin: 4px 0 8px; }
  .bar2 { gap: 10px; }
  .grow { flex: 1 1 200px; }
  .small { font-size: 13px; }
  .fine { font-size: 12px; }
  .done { padding: 10px 12px; border: 1px solid var(--line2); border-radius: 8px; background: var(--panel2); }
  .head { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 10px; }
  .timer { font-family: var(--mono); font-size: 34px; color: var(--blue); }
  .steps { list-style: none; padding: 0; margin: 12px 0; display: flex; flex-direction: column; gap: 4px; }
  .steps label { display: flex; gap: 12px; align-items: flex-start; padding: 12px; border: 1px solid var(--line); border-radius: 8px; cursor: pointer; }
  .steps label.ticked { opacity: .6; text-decoration: line-through; }
  .steps input { width: 20px; height: 20px; margin-top: 1px; flex: none; }
  .debrief { display: flex; flex-direction: column; gap: 6px; margin-bottom: 12px; }
  .end { justify-content: flex-end; }
</style>
