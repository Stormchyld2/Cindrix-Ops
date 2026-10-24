<script lang="ts">
  import { onMount } from 'svelte';
  import { trainingReports, trainingEnrol, trainingSignoff, type TrainReport, type TrainCourse } from '../lib/api';
  import { learner, loadPeople } from '../lib/learner.svelte';

  let { back, goFamily, openCourse, openLesson }: {
    back: () => void;
    goFamily: () => void;
    openCourse: (person: string, course: string) => void;
    openLesson: (person: string, lesson: string) => void;
  } = $props();

  let reports: TrainReport[] = $state([]);
  let who: string | null = $state(null);
  let error = $state('');
  let entry = $state('');
  let wrong = $state(false);
  let signer = $state('');
  let loading = $state(true);
  // a paper sheet for one course of one person: a certificate when signed off, a checklist when not
  let sheet: { person: string; course: TrainCourse } | null = $state(null);
  const todayText = new Date().toISOString().slice(0, 10);

  async function load() {
    try {
      reports = learner.people.length ? await trainingReports(learner.people) : [];
      error = '';
    } catch (e) {
      error = 'Training could not be read: ' + String(e);
    }
    loading = false;
  }
  onMount(async () => {
    if (!learner.loaded) await loadPeople();
    await load();
  });

  async function unlock() {
    wrong = !(await loadPeople(entry));
    entry = '';
    if (!wrong) await load();
  }

  const report = $derived(who ? reports.find((r) => r.person === who) : undefined);
  const pct = (a: number, b: number) => (b ? Math.round((a / b) * 100) : 0);
  const stateText: Record<string, string> = { passed: 'Learned', tried: 'Tried, not there yet', new: 'Not started', noquiz: 'Read only (no quiz)' };
  const followed = (r: TrainReport) => r.courses.filter((c) => c.chosen);
  const others = (r: TrainReport) => r.courses.filter((c) => !c.chosen);

  async function enrol(c: TrainCourse, on: boolean) {
    if (!who) return;
    try { await trainingEnrol(who, c.id, on); await load(); } catch (e) { error = String(e); }
  }
  async function sign(c: TrainCourse, on: boolean) {
    if (!who) return;
    try { await trainingSignoff(who, c.id, signer, on); await load(); } catch (e) { error = String(e); }
  }
</script>

<section class="wrap">
  {#if sheet}
    <div class="row between no-print">
      <button class="btn" onclick={() => (sheet = null)}>← Back</button>
      <button class="btn primary" onclick={() => window.print()} title="Print, or choose Save as PDF in the print window">Print</button>
    </div>
    <article class="paper" aria-label="Certificate or checklist">
      {#if sheet.course.signed_date}
        <p class="kicker">CERTIFICATE OF TRAINING</p>
        <h1>{sheet.person}</h1>
        <p class="big">completed the course</p>
        <h2>{sheet.course.title}</h2>
        <p>Signed off by <b>{sheet.course.signed_by}</b> on <b>{sheet.course.signed_date}</b>.</p>
        <p class="fine">{sheet.course.passed} of {sheet.course.needed} lesson quizzes passed at 70% or better. This is a record kept by the family, not an official qualification.</p>
      {:else}
        <p class="kicker">TRAINING CHECKLIST · printed {todayText}</p>
        <h1>{sheet.person}</h1>
        <h2>{sheet.course.title}</h2>
        <p class="fine">{sheet.course.passed} of {sheet.course.needed} lessons learned so far.</p>
      {/if}
      <ul class="boxes">
        {#each sheet.course.lessons as l}
          <li><span class="box" class:ticked={l.state === 'passed'}></span> {l.title}<span class="fine"> · {l.module}{l.last_date ? ' · ' + l.last_date : ''}</span></li>
        {/each}
      </ul>
      {#if !sheet.course.signed_date}
        <p class="sigline">Signed off by ______________________ on ____________</p>
      {/if}
    </article>
  {:else if !report}
    <div class="row between">
      <button class="btn" onclick={back}>← All courses</button>
    </div>
    <div>
      <h1>Training sign-off</h1>
      <p class="sub">One card for each person on your Family page. It shows what they have learned from the Academy quizzes, what they still need, and who has signed them off. A lesson counts as learned when the quiz score is 70% or better.</p>
    </div>
    {#if error}<div class="warn" role="alert">{error}</div>{/if}

    {#if learner.locked}
      <div class="panel">
        <p>The Family page is locked, so the names are hidden. Type its passphrase to see everyone here. It is only used to read the names and is not kept.</p>
        <form class="row" onsubmit={(e) => { e.preventDefault(); void unlock(); }}>
          <input class="input grow" type="password" bind:value={entry} placeholder="Family passphrase" aria-label="Family passphrase" />
          <button class="btn primary" type="submit">Unlock</button>
        </form>
        {#if wrong}<p class="warn">That did not open it. Try again.</p>{/if}
      </div>
    {:else if learner.loaded && learner.people.length === 0}
      <div class="panel">
        <p>No one is on the Family page yet. Add the people in your home there, and a card for each will appear here.</p>
        <button class="btn" onclick={goFamily}>Open the Family page</button>
      </div>
    {:else}
      <div class="cards">
        {#each reports as r (r.person)}
          <button class="card" onclick={() => (who = r.person)} aria-label={`Open the training page for ${r.person}`}>
            <span class="ctitle">{r.person}</span>
            <span class="meta">{r.chosen} course{r.chosen === 1 ? '' : 's'} followed · {r.signed} signed off</span>
            <span class="track"><span style="width:{pct(r.passed, r.needed)}%"></span></span>
            <span class="meta">{r.needed ? `${r.passed} of ${r.needed} lessons learned` : 'Nothing started yet'}</span>
            {#if r.suggestions[0]}<span class="next">Next: {r.suggestions[0].title}</span>{/if}
          </button>
        {/each}
      </div>
    {/if}
    {#if loading}<p class="muted">Reading the training…</p>{/if}
  {:else}
    <div class="row between">
      <button class="btn" onclick={() => (who = null)}>← Everyone</button>
      <span class="muted small">{report.signed} signed off</span>
    </div>
    <header>
      <h1 class="ctitle">{report.person}</h1>
      <div class="track big"><span style="width:{pct(report.passed, report.needed)}%"></span></div>
      <p class="muted">{report.needed ? `${report.passed} of ${report.needed} lessons learned in the courses followed (${pct(report.passed, report.needed)}%).` : 'No course followed yet. Choose one below.'}</p>
    </header>
    {#if error}<div class="warn" role="alert">{error}</div>{/if}

    {#if report.suggestions.length}
      <section class="panel">
        <div class="label">NEXT COURSE</div>
        {#each report.suggestions as s}
          <div class="line">
            <div class="grow"><strong>{s.title}</strong><div class="muted small">{s.reason}</div></div>
            <button class="btn" onclick={() => openCourse(report.person, s.course)} aria-label={`Open the course ${s.title}`}>Open course</button>
            {#if !report.courses.find((c) => c.id === s.course)?.chosen}
              <button class="btn primary" onclick={() => enrol(report.courses.find((c) => c.id === s.course)!, true)} aria-label={`Add ${s.title} to ${report.person}'s courses`}>Add to {report.person}</button>
            {/if}
          </div>
        {/each}
      </section>
    {/if}

    {#each followed(report) as c (c.id)}
      <section class="panel">
        <div class="row between">
          <h2>{c.title}</h2>
          <span class="meta">{c.passed} of {c.needed} learned{c.minutes_left ? ` · about ${c.minutes_left} min left` : ''}</span>
        </div>
        <div class="track"><span style="width:{pct(c.passed, c.needed)}%"></span></div>
        <ul class="lessons">
          {#each c.lessons as l (l.id)}
            <li>
              <button onclick={() => openLesson(report.person, l.id)} aria-label={`Open the lesson ${l.title}`}>
                <span class="mark {l.state}">{l.state === 'passed' ? '✓' : l.state === 'tried' ? '!' : l.state === 'noquiz' ? '–' : '○'}</span>
                <span class="grow"><span class="title">{l.title}</span><span class="muted small">{l.module} · {stateText[l.state]}{l.total ? ` · best ${l.best_score}/${l.total}` : ''}{l.last_date ? ` · ${l.last_date}` : ''}</span></span>
              </button>
            </li>
          {/each}
        </ul>
        {#if c.signed_date}
          <div class="signed">Signed off by <strong>{c.signed_by}</strong> on {c.signed_date}. <button class="link" onclick={() => sign(c, false)}>Take the sign-off back</button></div>
        {:else}
          <div class="row sign">
            <input class="input grow" bind:value={signer} placeholder="Who is signing off? (your name)" aria-label="Name of the person signing off" />
            <button class="btn primary" disabled={!signer.trim()} onclick={() => sign(c, true)} title={c.complete ? 'Record that this person has completed the course' : 'You can sign off before every quiz is passed, for example after a hands-on practice'}>Sign off {c.title}</button>
          </div>
          {#if !c.complete}<p class="muted small">{c.needed - c.passed} lesson{c.needed - c.passed === 1 ? '' : 's'} still to pass. You may sign off anyway if you have seen them do it.</p>{/if}
        {/if}
        <button class="btn" onclick={() => (sheet = { person: report.person, course: c })} title="Open a page to print: a certificate once signed off, otherwise a checklist of the lessons">{c.signed_date ? 'Certificate to print' : 'Checklist to print'}</button>
        <button class="link" onclick={() => enrol(c, false)}>Stop following this course (results are kept)</button>
      </section>
    {/each}

    {#if others(report).length}
      <section class="panel">
        <div class="label">COURSES NOT FOLLOWED YET</div>
        {#each others(report) as c (c.id)}
          <div class="line">
            <div class="grow"><strong>{c.title}</strong><div class="muted small">{c.needed} lesson{c.needed === 1 ? '' : 's'} with a quiz</div></div>
            <button class="btn" onclick={() => enrol(c, true)} aria-label={`Add ${c.title} to ${report.person}'s courses`}>Add to {report.person}</button>
          </div>
        {/each}
      </section>
    {/if}
  {/if}
</section>

<style>
  .paper { background: #fff; color: #111; border-radius: 6px; padding: 32px 36px; font-family: Georgia, 'Times New Roman', serif; line-height: 1.5; }
  .paper h1, .paper h2, .paper p, .paper li { color: #111; font-family: inherit; }
  .paper h1 { font-size: 30px; margin: 6px 0; }
  .paper h2 { font-size: 20px; margin: 4px 0 10px; }
  .paper .kicker { letter-spacing: .14em; font-size: 12px; margin: 0; }
  .paper .big { font-size: 16px; margin: 0; }
  .paper .fine { font-size: 12px; color: #444; }
  .paper .boxes { list-style: none; padding: 0; margin: 10px 0; }
  .paper .boxes li { display: flex; gap: 10px; align-items: baseline; margin: 5px 0; }
  .paper .box { width: 14px; height: 14px; border: 2px solid #111; flex: none; }
  .paper .box.ticked { background: #111; }
  .paper .sigline { margin-top: 28px; }
  .wrap { display: flex; flex-direction: column; gap: 14px; max-width: 900px; }
  .between { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 12px; }
  .card { display: flex; flex-direction: column; gap: 8px; text-align: left; padding: 16px; background: var(--panel); border: 1px solid var(--line); border-top: 2px solid var(--line2); border-radius: 12px; }
  .card:hover { border-color: var(--blue-line); }
  .ctitle { font-size: 20px; letter-spacing: .1em; font-family: var(--display); text-transform: uppercase; margin: 0; }
  .meta { font-family: var(--mono); font-size: 11px; letter-spacing: .06em; color: var(--muted); }
  .next { font-size: 13px; color: var(--blue); }
  .track { height: 8px; border-radius: 4px; background: var(--line); overflow: hidden; }
  .track.big { height: 12px; }
  .track span { display: block; height: 100%; background: var(--blue); }
  h2 { margin: 0; font-size: 18px; }
  .line { display: flex; gap: 10px; align-items: center; padding: 10px 0; border-top: 1px solid var(--line); flex-wrap: wrap; }
  .grow { flex: 1 1 200px; display: flex; flex-direction: column; gap: 2px; }
  .small { font-size: 12px; }
  .lessons { list-style: none; padding: 0; margin: 10px 0; display: flex; flex-direction: column; gap: 6px; }
  .lessons button { width: 100%; display: flex; align-items: center; gap: 12px; text-align: left; padding: 10px 12px; border: 1px solid var(--line); border-radius: 8px; background: transparent; color: inherit; }
  .lessons button:hover { border-color: var(--blue); }
  .title { font-weight: 600; }
  .mark { width: 24px; height: 24px; border-radius: 50%; border: 1px solid var(--line2); display: grid; place-items: center; flex: none; font-size: 13px; }
  .mark.passed { background: var(--blue); color: #000; border-color: var(--blue); }
  .mark.tried { border-color: #d9a441; color: #d9a441; }
  .signed { padding: 10px 12px; border: 1px solid var(--blue-line); background: var(--blue-soft); border-radius: 8px; margin: 8px 0; }
  .sign { gap: 8px; margin: 8px 0; }
</style>
