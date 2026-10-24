<script lang="ts">
  import { onMount } from 'svelte';
  import { sourceRequest, viewRequest } from '../lib/chat.svelte';
  import { academyList, academyLesson, academySave, academyProblems, type AcademyModule, type AcademyLesson, type CourseProblem } from '../lib/api';
  import { learner, chooseLearner, loadPeople } from '../lib/learner.svelte';
  import Training from './Training.svelte';

  let { go }: { go?: (tab: string) => void } = $props();
  let training = $state(false);
  // the top bar's Academy > Training sign-off
  $effect(() => { if (viewRequest.training) { training = true; viewRequest.training = false; } });

  let modules: AcademyModule[] = $state([]);
  let problems: CourseProblem[] = $state([]);
  let error = $state('');
  let loaded = $state(false);

  let lesson = $state<AcademyLesson | null>(null);
  let quizOpen = $state(false);
  let picked: number[] = $state([]);
  // the order the answers are shown in, rotated at random so the right one is not always first
  let orders: number[][] = $state([]);
  let checkedAnswers = $state(false);
  let savedNote = $state('');

  async function load() {
    try {
      modules = await academyList(learner.name || undefined);
      problems = await academyProblems();
    } catch (e) {
      error = String(e);
    }
    loaded = true;
  }
  onMount(async () => {
    if (!learner.loaded) await loadPeople();
    await load();
    if (sourceRequest.lesson) { const want = sourceRequest.lesson; sourceRequest.lesson = ''; await open(want); }
  });
  async function pickLearner(n: string) {
    chooseLearner(n);
    await load();
  }
  async function fromTraining(person: string, course: string | null, lessonId: string | null) {
    chooseLearner(person);
    training = false;
    courseId = course;
    await load();
    if (lessonId) await open(lessonId);
  }

  // the modules, grouped under their course
  const courses = $derived.by(() => {
    const out: { id: string; title: string; summary: string; author: string; modules: AcademyModule[] }[] = [];
    for (const m of modules) {
      let c = out.find((x) => x.id === m.course);
      if (!c) { c = { id: m.course, title: m.course_title, summary: m.course_summary, author: m.course_author, modules: [] }; out.push(c); }
      c.modules.push(m);
    }
    return out;
  });
  let courseId: string | null = $state(null);
  const openCourse = $derived(courseId ? courses.find((c) => c.id === courseId) : undefined);
  function stats(c: { modules: AcademyModule[] }) {
    const ls = c.modules.flatMap((m) => m.lessons);
    return { lessons: ls.length, done: ls.filter((l) => l.tries > 0).length, hours: Math.round((ls.reduce((a, l) => a + l.minutes, 0) / 60) * 10) / 10, draft: ls.some((l) => l.status !== 'reviewed') };
  }
  const allLessons = $derived(modules.flatMap((m) => m.lessons));
  const finished = $derived(allLessons.filter((l) => l.tries > 0).length);
  const minutes = $derived(allLessons.reduce((a, l) => a + l.minutes, 0));

  async function open(id: string) {
    error = '';
    try {
      lesson = await academyLesson(id);
      quizOpen = false;
      checkedAnswers = false;
      savedNote = '';
      picked = lesson.quiz.map(() => -1);
      orders = lesson.quiz.map((q) => {
        const n = q.options.length;
        const r = Math.floor(Math.random() * n);
        return q.options.map((_, i) => (i + r) % n);
      });
      window.scrollTo?.(0, 0);
    } catch (e) {
      error = String(e);
    }
  }

  function close() {
    lesson = null;
  }

  const score = $derived(lesson ? lesson.quiz.filter((q, i) => picked[i] === q.correct).length : 0);

  async function save() {
    if (!lesson) return;
    try {
      await academySave(lesson.id, score, lesson.quiz.length, learner.name || undefined);
      savedNote = `Saved: ${score} of ${lesson.quiz.length}.`;
      await load();
    } catch (e) {
      error = String(e);
    }
  }

  // **bold** inside a line, shown without ever inserting raw HTML
  function parts(t: string): { b: boolean; t: string }[] {
    return t.split('**').map((x, i) => ({ b: i % 2 === 1, t: x })).filter((x) => x.t);
  }
  function nextIn(id: string): string | null {
    const i = allLessons.findIndex((l) => l.id === id);
    return i >= 0 && i + 1 < allLessons.length ? allLessons[i + 1].id : null;
  }
</script>

<section class="wrap">
  {#if training}
    <Training back={() => (training = false)} goFamily={() => go?.('Family')} openCourse={(p, c) => fromTraining(p, c, null)} openLesson={(p, l) => fromTraining(p, null, l)} />
  {:else if !lesson}
    {#if !openCourse}
      <div>
        <h1>Academy</h1>
        <p class="sub">Courses you can work through at your own pace, with a short quiz after each lesson. Choose a course to go inside it. Courses are plain folders on this drive, so nothing needs the internet. Your progress stays on this drive.</p>
      </div>
      {#if error}<div class="warn">{error}</div>{/if}
      <div class="panel who row">
        {#if learner.people.length}
          <label class="row grow">Learning as
            <select class="input" value={learner.name} onchange={(e) => pickLearner(e.currentTarget.value)} aria-label="Who is learning">
              <option value="">Nobody chosen (everyone together)</option>
              {#each learner.people as n}<option value={n}>{n}</option>{/each}
            </select>
          </label>
        {:else}
          <span class="grow muted">Add people on the Family page to keep each person's progress apart.</span>
        {/if}
        <button class="btn primary" onclick={() => (training = true)} title="See what each person has learned and still needs to learn">Training sign-off</button>
      </div>
      {#if allLessons.length}
        <div class="panel prog">
          <span class="label">{learner.name ? learner.name.toUpperCase() + '’S PROGRESS' : 'YOUR PROGRESS'}</span>
          <span>{finished} of {allLessons.length} lessons done · about {Math.round(minutes / 60 * 10) / 10} hours in all</span>
          <div class="track"><span style="width:{(finished / allLessons.length) * 100}%"></span></div>
        </div>
      {/if}
      <div class="cards">
        {#each courses as c (c.id)}
          {@const st = stats(c)}
          <button class="card" onclick={() => (courseId = c.id)} aria-label={`Open the course ${c.title}`}>
            <span class="ctitle">{c.title}</span>
            {#if c.summary}<span class="csum">{c.summary}</span>{/if}
            {#if c.author}<span class="muted small">By {c.author}</span>{/if}
            <span class="meta">{c.modules.length} module{c.modules.length === 1 ? '' : 's'} · {st.lessons} lesson{st.lessons === 1 ? '' : 's'} · about {st.hours} hour{st.hours === 1 ? '' : 's'}</span>
            <span class="track"><span style="width:{st.lessons ? (st.done / st.lessons) * 100 : 0}%"></span></span>
            <span class="meta">{st.done} of {st.lessons} done{st.draft ? ' · not yet reviewed' : ''}</span>
          </button>
        {/each}
      </div>
      {#if problems.length}
        <details class="panel probs">
          <summary>{problems.length} course file{problems.length === 1 ? ' was' : 's were'} left out</summary>
          <p class="muted small">The rest of each course still works. Fix the file and it appears here by itself next time.</p>
          <ul>{#each problems as p}<li><code>{p.place}</code>: {p.message}</li>{/each}</ul>
        </details>
      {/if}
      {#if loaded && courses.length === 0 && !error}<p class="muted">No courses found. A course is a folder in the <code>academy</code> folder on the drive.</p>{/if}
    {:else}
      <div class="row between">
        <button class="btn" onclick={() => (courseId = null)}>← All courses</button>
        {#if openCourse.author}<span class="muted small">By {openCourse.author}</span>{/if}
      </div>
      <header class="course">
        <h1 class="ctitle">{openCourse.title}</h1>
        {#if openCourse.summary}<p class="muted">{openCourse.summary}</p>{/if}
      </header>
      {#each openCourse.modules as m (m.id)}
        <article class="panel">
          <h2>{m.title}</h2>
          {#if m.summary}<p class="muted">{m.summary}</p>{/if}
          <ul class="lessons">
            {#each m.lessons as l (l.id)}
              <li>
                <button class="row" onclick={() => open(l.id)} aria-label={l.title}>
                  <span class="mark" class:done={l.tries > 0}>{l.tries > 0 ? '✓' : '○'}</span>
                  <span class="grow">
                    <span class="title">{l.title}</span>
                    <span class="muted small">{l.minutes} min{l.questions ? ` · ${l.questions} question${l.questions === 1 ? '' : 's'}` : ''}{l.status !== 'reviewed' ? ' · draft' : ''}</span>
                  </span>
                  {#if l.tries > 0}<span class="best">Best {l.best_score}/{l.total}</span>{/if}
                </button>
              </li>
            {/each}
          </ul>
        </article>
      {/each}
    {/if}
  {:else}
    <div class="row between no-print">
      <button class="btn" onclick={close}>← Back to the course</button>
      <span class="row">
        <span class="muted small">{lesson.minutes} min</span>
        <button class="btn" title="Print this lesson, or save it as a PDF, to keep a paper copy" onclick={() => window.print()}>Print lesson</button>
      </span>
    </div>
    <article class="panel lesson">
      <h1>{lesson.title}</h1>
      {#if lesson.status !== 'reviewed'}<div class="warn">Draft lesson. It has not been reviewed by a qualified person yet. Check it against your own situation and training.</div>{/if}
      {#each lesson.blocks as b}
        {#if b.kind === 'heading'}
          <h2>{b.text}</h2>
        {:else if b.kind === 'para'}
          <p>{#each parts(b.text) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</p>
        {:else if b.kind === 'note'}
          <p class="note">{#each parts(b.text) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</p>
        {:else if b.kind === 'bullets'}
          <ul>{#each b.items as it}<li>{#each parts(it) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</li>{/each}</ul>
        {:else}
          <ol>{#each b.items as it}<li>{#each parts(it) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</li>{/each}</ol>
        {/if}
      {/each}
      {#if lesson.source}<p class="muted small">Source: {lesson.source}</p>{/if}
    </article>

    {#if lesson.quiz.length}
      <div class="no-print">
      {#if !quizOpen}
        <div><button class="btn primary" onclick={() => (quizOpen = true)}>Take the quiz</button></div>
      {:else}
        <div class="panel quiz">
          <div class="label">CHECK YOURSELF</div>
          {#each lesson.quiz as q, qi}
            <fieldset>
              <legend>{qi + 1}. {q.text}</legend>
              {#each orders[qi] ?? q.options.map((_, i) => i) as oi}
                {@const o = q.options[oi]}
                <label class:right={checkedAnswers && oi === q.correct} class:wrong={checkedAnswers && picked[qi] === oi && oi !== q.correct}>
                  <input type="radio" name={`q${qi}`} checked={picked[qi] === oi} disabled={checkedAnswers} onchange={() => (picked[qi] = oi)} aria-label={o} />
                  <span>{o}</span>
                </label>
              {/each}
              {#if checkedAnswers && q.explain}<p class="why">{q.explain}</p>{/if}
            </fieldset>
          {/each}
          {#if !checkedAnswers}
            <div><button class="btn primary" disabled={picked.some((p) => p < 0)} onclick={() => (checkedAnswers = true)}>Check answers</button></div>
          {:else}
            <p class="result">{score} of {lesson.quiz.length} correct</p>
            {#if savedNote}<div class="done">{savedNote}</div>{/if}
            <div class="row end">
              {#if !savedNote}<button class="btn primary" onclick={save}>Save result</button>{/if}
              <button class="btn" onclick={() => open(lesson!.id)}>Try again</button>
              {#if nextIn(lesson.id)}<button class="btn" onclick={() => open(nextIn(lesson!.id)!)}>Next lesson →</button>{/if}
            </div>
          {/if}
        </div>
      {/if}
      </div>
    {/if}
    {#if error}<div class="warn no-print">{error}</div>{/if}
  {/if}
</section>

<style>
  @media print {
    .lesson { background: #fff !important; color: #111 !important; border: 0 !important; font-family: Georgia, 'Times New Roman', serif; }
    .lesson h1, .lesson h2, .lesson p, .lesson li, .lesson b { color: #111 !important; }
    .lesson .note { border: 1px solid #111; padding: 6px 10px; background: #fff !important; }
    .lesson .warn { background: #fff !important; color: #111 !important; border: 1px solid #111; }
    .lesson .muted { color: #444 !important; }
  }
  .who { align-items: center; gap: 10px; flex-wrap: wrap; justify-content: space-between; }
  .who label { gap: 8px; align-items: center; }
  .course { margin-top: 8px; }
  .ctitle { font-size: 20px; letter-spacing: .1em; font-family: var(--display); text-transform: uppercase; margin: 0; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 12px; }
  .card { display: flex; flex-direction: column; gap: 8px; text-align: left; padding: 16px; background: var(--panel); border: 1px solid var(--line); border-top: 2px solid var(--line2); border-radius: 12px; }
  .card:hover { border-color: var(--blue-line); }
  .csum { color: var(--muted); }
  .meta { font-family: var(--mono); font-size: 11px; letter-spacing: .06em; color: var(--muted); }
  .probs summary { cursor: pointer; }
  .probs ul { margin: 8px 0 0; padding-left: 20px; }
  .wrap { display: flex; flex-direction: column; gap: 14px; max-width: 900px; }
  .prog { display: flex; flex-direction: column; gap: 6px; }
  .track { height: 8px; border-radius: 4px; background: var(--line); overflow: hidden; }
  .track span { display: block; height: 100%; background: var(--blue); }
  h2 { margin: 0 0 4px; font-size: 18px; }
  .lessons { list-style: none; padding: 0; margin: 10px 0 0; display: flex; flex-direction: column; gap: 6px; }
  .lessons button { width: 100%; display: flex; align-items: center; gap: 12px; text-align: left; padding: 12px; border: 1px solid var(--line); border-radius: 8px; background: transparent; color: inherit; cursor: pointer; }
  .lessons button:hover { border-color: var(--blue); }
  .grow { flex: 1; display: flex; flex-direction: column; gap: 2px; }
  .title { font-weight: 600; }
  .small { font-size: 12px; }
  .mark { width: 24px; height: 24px; border-radius: 50%; border: 1px solid var(--line2); display: grid; place-items: center; flex: none; font-size: 13px; }
  .mark.done { background: var(--blue); color: #000; border-color: var(--blue); }
  .best { font-family: var(--mono); font-size: 12px; color: var(--blue); }
  .between { justify-content: space-between; display: flex; align-items: center; }
  .lesson p, .lesson li { line-height: 1.6; }
  .lesson h1 { margin-top: 0; }
  .lesson ul, .lesson ol { padding-left: 22px; }
  .note { padding: 10px 12px; border-left: 3px solid var(--blue); background: var(--panel2); border-radius: 4px; }
  .quiz { display: flex; flex-direction: column; gap: 14px; }
  fieldset { border: 1px solid var(--line); border-radius: 8px; padding: 10px 12px; display: flex; flex-direction: column; gap: 6px; }
  legend { font-weight: 600; padding: 0 6px; }
  fieldset label { display: flex; gap: 10px; align-items: flex-start; padding: 8px; border-radius: 6px; cursor: pointer; }
  fieldset label.right { background: rgba(60, 180, 90, .18); }
  fieldset label.wrong { background: rgba(220, 70, 60, .18); }
  .why { margin: 0; font-size: 14px; color: var(--muted, inherit); }
  .result { font-size: 20px; font-weight: 700; margin: 0; }
  .done { padding: 10px 12px; border: 1px solid var(--line2); border-radius: 8px; background: var(--panel2); }
  .end { justify-content: flex-end; display: flex; gap: 8px; flex-wrap: wrap; }
</style>
