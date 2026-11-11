<script lang="ts">
  import { onMount } from 'svelte';
  import { sourceRequest, viewRequest } from '../lib/chat.svelte';
  import { academyList, academyLesson, academySave, academyCheckSet, academyProblems, academyPackInstall, academyPackExport, inShare, type AcademyModule, type AcademyLesson, type AcademyLessonMeta, type CourseProblem } from '../lib/api';
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
  let ticked: number[] = $state([]);
  let tab: 'skills' | 'readiness' | 'checklists' = $state('skills');
  let search = $state('');
  let tierFilter = $state(0);
  let quizOpen = $state(false);
  let picked: number[] = $state([]);
  // the order the answers are shown in, rotated at random so the right one is not always first
  let orders: number[][] = $state([]);
  let checkedAnswers = $state(false);
  let savedNote = $state('');

  let packNotes: string[] = $state([]);
  async function installPacks() {
    packNotes = [];
    try { packNotes = await academyPackInstall(); await load(); } catch (e) { packNotes = [String(e)]; }
  }
  async function sharePack() {
    packNotes = [];
    try { packNotes = ['Saved: ' + (await academyPackExport()) + '. Give this file to someone: they put it in their user\\academy-packs folder and press Install course packs.']; } catch (e) { packNotes = [String(e)]; }
  }
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
  type CourseT = { id: string; title: string; summary: string; author: string; tier: number; modules: AcademyModule[] };
  const courses = $derived.by(() => {
    const out: CourseT[] = [];
    for (const m of modules) {
      let c = out.find((x) => x.id === m.course);
      if (!c) { c = { id: m.course, title: m.course_title, summary: m.course_summary, author: m.course_author, tier: m.course_tier, modules: [] }; out.push(c); }
      c.modules.push(m);
    }
    return out;
  });
  const tierNames: Record<number, [string, string]> = {
    3: ['Foundation', 'The baseline every household should have. Finish all of Tier 3 to reach your first full readiness score.'],
    2: ['Practitioner', 'Deeper skills for the people who lead each area.'],
    1: ['Operator', 'Expert level: run a whole area for the group under stress.']
  };
  const q = $derived(search.trim().toLowerCase());
  function matches(c: CourseT): boolean {
    if (tierFilter && (c.tier || 0) !== tierFilter) return false;
    if (!q) return true;
    return c.title.toLowerCase().includes(q) || c.summary.toLowerCase().includes(q) || c.modules.some((m) => m.title.toLowerCase().includes(q) || m.lessons.some((l) => l.title.toLowerCase().includes(q)));
  }
  const tierGroups = $derived.by(() =>
    [3, 2, 1, 0]
      .map((t) => ({ tier: t, title: tierNames[t]?.[0] ?? 'Other courses', note: tierNames[t]?.[1] ?? '', list: courses.filter((c) => (tierNames[c.tier] ? c.tier : 0) === t && matches(c)) }))
      .filter((g) => g.list.length > 0)
  );
  let courseId: string | null = $state(null);
  const openCourse = $derived(courseId ? courses.find((c) => c.id === courseId) : undefined);

  /** A lesson counts when it has a quiz or a checklist. A plain reading page is shown but not counted. */
  const countable = (l: AcademyLessonMeta) => l.questions > 0 || l.checklist > 0;
  const isDone = (l: AcademyLessonMeta) => (l.questions > 0 ? l.tries > 0 : l.checklist > 0 ? l.checks_done >= l.checklist : false);
  function lessonsOf(c: { modules: AcademyModule[] }): AcademyLessonMeta[] { return c.modules.flatMap((m) => m.lessons); }
  function stats(c: { modules: AcademyModule[] }) {
    const all = lessonsOf(c);
    const cnt = all.filter(countable);
    return {
      lessons: all.length,
      counted: cnt.length,
      done: cnt.filter(isDone).length,
      checklists: all.filter((l) => l.checklist > 0).length,
      checklistsDone: all.filter((l) => l.checklist > 0 && l.checks_done >= l.checklist).length,
      quizzes: all.filter((l) => l.questions > 0).length,
      quizzesDone: all.filter((l) => l.questions > 0 && l.tries > 0).length,
      sections: c.modules.length
    };
  }
  const pct = (a: number, b: number) => (b ? Math.round((a / b) * 100) : 0);
  const allLessons = $derived(modules.flatMap((m) => m.lessons));
  /** The guided path: Tier 3, then Tier 2, then Tier 1, in the order the courses are listed. */
  const pathLessons = $derived([3, 2, 1].flatMap((t) => courses.filter((c) => c.tier === t).flatMap(lessonsOf)).filter(countable));
  const pathNext = $derived(pathLessons.find((l) => !isDone(l)));
  const pathDone = $derived(pathLessons.filter(isDone).length);
  function courseOfLesson(id: string): CourseT | undefined { return courses.find((c) => lessonsOf(c).some((l) => l.id === id)); }
  function moduleOfLesson(id: string): AcademyModule | undefined { return modules.find((m) => m.lessons.some((l) => l.id === id)); }
  function startCourse(c: CourseT) {
    const next = lessonsOf(c).filter(countable).find((l) => !isDone(l)) ?? lessonsOf(c)[0];
    if (next) void open(next.id);
  }
  function startModule(m: AcademyModule) { const next = m.lessons.find((l) => countable(l) && !isDone(l)) ?? m.lessons[0]; if (next) void open(next.id); }
  function pathTierOf(id: string): number { return courseOfLesson(id)?.tier ?? 0; }

  // the readiness score: points from the checklists ticked, shared out over each lesson's items
  const earned = $derived(allLessons.reduce((a, l) => a + (l.checklist ? l.readiness * (l.checks_done / l.checklist) : 0), 0));
  const possible = $derived(allLessons.reduce((a, l) => a + l.readiness, 0));
  const readinessPct = $derived(possible ? Math.round((earned / possible) * 100) : 0);
  const level = $derived(readinessPct >= 90 ? 'Prepared' : readinessPct >= 60 ? 'Ready' : readinessPct >= 25 ? 'Building' : 'Beginning');
  const areas = $derived(courses.filter((c) => c.tier === 3 || c.tier === 2).slice(0, 8).map((c) => { const s = stats(c); return { id: c.id, title: c.title, p: pct(s.done, s.counted) }; }));
  const checklistLessons = $derived(allLessons.filter((l) => l.checklist > 0));

  async function open(id: string) {
    error = '';
    try {
      lesson = await academyLesson(id, learner.name || undefined);
      ticked = [...lesson.ticked];
      const c = courseOfLesson(id);
      if (c) courseId = c.id;
      quizOpen = false;
      checkedAnswers = false;
      savedNote = '';
      picked = lesson.quiz.map(() => -1);
      orders = lesson.quiz.map((qq) => {
        const n = qq.options.length;
        const r = Math.floor(Math.random() * n);
        return qq.options.map((_, i) => (i + r) % n);
      });
      window.scrollTo?.(0, 0);
    } catch (e) {
      error = String(e);
    }
  }
  function close() { lesson = null; }
  function leaveCourse() { courseId = null; lesson = null; }

  const score = $derived(lesson ? lesson.quiz.filter((qq, i) => picked[i] === qq.correct).length : 0);

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
  async function tick(n: number) {
    if (!lesson) return;
    const on = !ticked.includes(n);
    ticked = on ? [...ticked, n] : ticked.filter((x) => x !== n);
    try { await academyCheckSet(lesson.id, n, on, learner.name || undefined); await load(); } catch (e) { error = String(e); }
  }

  // **bold** inside a line, shown without ever inserting raw HTML
  function parts(t: string): { b: boolean; t: string }[] {
    return t.split('**').map((x, i) => ({ b: i % 2 === 1, t: x })).filter((x) => x.t);
  }
  /** A how-to step written as "**Title.** the rest" becomes a card with a title. */
  function stepParts(s: string): { title: string; body: string } {
    const m = s.match(/^\*\*(.+?)\*\*\s*(.*)$/);
    return m ? { title: m[1].replace(/[.:]$/, ''), body: m[2] } : { title: '', body: s };
  }
  function nextIn(id: string): string | null {
    const c = courseOfLesson(id);
    if (!c) return null;
    const ls = lessonsOf(c);
    const i = ls.findIndex((l) => l.id === id);
    return i >= 0 && i + 1 < ls.length ? ls[i + 1].id : null;
  }
  const curMeta = $derived(lesson ? allLessons.find((l) => l.id === lesson!.id) : undefined);
  const curModule = $derived(lesson ? moduleOfLesson(lesson.id) : undefined);
  const curCourse = $derived(lesson ? courseOfLesson(lesson.id) : undefined);
  const modDone = $derived(curModule ? curModule.lessons.filter(countable).filter(isDone).length : 0);
  const modTotal = $derived(curModule ? curModule.lessons.filter(countable).length : 0);
</script>

<section class="ac" class:player={!!lesson && !training}>
  {#if training}
    <Training back={() => (training = false)} goFamily={() => go?.('Family')} openCourse={(p, c) => fromTraining(p, c, null)} openLesson={(p, l) => fromTraining(p, null, l)} />
  {:else if lesson}
    <!-- the lesson player -->
    <div class="crumb no-print">
      <button class="link" onclick={close}>← {curCourse?.title ?? 'Course'}</button>
      <span class="bar-sep"></span>
      <b>{curModule?.title ?? lesson.title}</b>
      <span class="grow"></span>
      <span class="muted small">{lesson.minutes} min</span>
      <button class="btn dark" title="Print this lesson, or save it as a PDF, to keep a paper copy" onclick={() => window.print()}>Print</button>
    </div>
    <div class="player">
      <article class="lbody">
        {#if lesson.blocks[0]?.kind !== 'video'}<h1 class="ltitle">{lesson.title}</h1>{/if}
        {#if lesson.status === 'draft'}<div class="notice">This lesson has not been reviewed by a qualified person yet. Check it against your own situation and training.</div>{/if}
        {#each lesson.blocks as b, bi}
          {#if b.kind === 'video'}
            <div class="video" role="img" aria-label="Video placeholder"><div class="play">▶</div></div>
            <h1 class="ltitle">{b.text || lesson.title}</h1>
          {:else if b.kind === 'heading'}
            <h2>{b.text}</h2>
          {:else if b.kind === 'para'}
            <p>{#each parts(b.text) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</p>
          {:else if b.kind === 'image'}
            <figure class="pic">
              {#if b.items[0]}<img src={b.items[0]} alt={b.text} />{:else}<div class="nopic muted">Picture not found: {b.text}</div>{/if}
              {#if b.text}<figcaption class="muted small">{b.text}</figcaption>{/if}
            </figure>
          {:else if b.kind === 'note'}
            <p class="note">{#each parts(b.text) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</p>
          {:else if b.kind === 'bullets'}
            <ul>{#each b.items as it}<li>{#each parts(it) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</li>{/each}</ul>
          {:else if b.kind === 'steps'}
            {#if b.items.every((s) => stepParts(s).title)}
              <div class="label">HOW IT WORKS · {b.items.length} STEPS</div>
              <div class="steps">
                {#each b.items as s, si}
                  {@const sp = stepParts(s)}
                  <div class="step"><div class="n">{si + 1}</div><b>{sp.title}</b><span class="muted">{sp.body}</span></div>
                {/each}
              </div>
            {:else}
              <ol>{#each b.items as it}<li>{#each parts(it) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</li>{/each}</ol>
            {/if}
          {:else if b.kind === 'checklist'}
            <div class="check">
              <div class="chead">
                <b>Checklist</b>
                {#if lesson.readiness > 0}<span class="rd">+{lesson.readiness}% READINESS</span>{/if}
                <span class="grow"></span>
                <span class="muted small">{(b.checks ?? []).filter((c) => ticked.includes(c.n)).length}/{(b.checks ?? []).length} done</span>
              </div>
              {#each b.checks ?? [] as c (c.n)}
                <label class="ci" class:on={ticked.includes(c.n)}>
                  <input type="checkbox" checked={ticked.includes(c.n)} onchange={() => tick(c.n)} aria-label={c.text} />
                  <span class="grow"><span class="ct">{c.text}</span>{#if c.detail}<span class="cd">{c.detail}</span>{/if}</span>
                  {#if c.gear && go}<button class="shop" onclick={(e) => { e.preventDefault(); go?.('Inventory'); }} title="Open Inventory to add or check this item">Find in Inventory</button>{/if}
                </label>
              {/each}
            </div>
          {/if}
        {/each}
        {#if lesson.source}<p class="muted small">Source: {lesson.source}</p>{/if}

        {#if lesson.quiz.length}
          <div class="no-print">
            {#if !quizOpen}
              <button class="btn" onclick={() => (quizOpen = true)}>Take the quiz</button>
            {:else}
              <div class="quiz">
                <div class="label">CHECK YOURSELF</div>
                {#each lesson.quiz as qq, qi}
                  <fieldset>
                    <legend>{qi + 1}. {qq.text}</legend>
                    {#each orders[qi] ?? qq.options.map((_, i) => i) as oi}
                      {@const o = qq.options[oi]}
                      <label class:right={checkedAnswers && oi === qq.correct} class:wrong={checkedAnswers && picked[qi] === oi && oi !== qq.correct}>
                        <input type="radio" name={`q${qi}`} checked={picked[qi] === oi} disabled={checkedAnswers} onchange={() => (picked[qi] = oi)} aria-label={o} />
                        <span>{o}</span>
                      </label>
                    {/each}
                    {#if checkedAnswers && qq.explain}<p class="why">{qq.explain}</p>{/if}
                  </fieldset>
                {/each}
                {#if !checkedAnswers}
                  <div><button class="btn" disabled={picked.some((p) => p < 0)} onclick={() => (checkedAnswers = true)}>Check answers</button></div>
                {:else}
                  <p class="result">{score} of {lesson.quiz.length} correct</p>
                  {#if savedNote}<div class="notice">{savedNote}</div>{/if}
                  <div class="row end">
                    {#if !savedNote}<button class="btn" onclick={save}>Save result</button>{/if}
                    <button class="btn dark" onclick={() => open(lesson!.id)}>Try again</button>
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/if}
        <div class="row no-print" style="margin-top:18px">
          {#if nextIn(lesson.id)}
            <button class="btn" onclick={() => open(nextIn(lesson!.id)!)}>Next →</button>
          {:else}
            <button class="btn" onclick={close}>Back to the course</button>
          {/if}
        </div>
        {#if error}<div class="notice">{error}</div>{/if}
      </article>
      <aside class="side no-print">
        <div class="label">PROGRESS</div>
        <div class="bigpct">{pct(modDone, modTotal)}%</div>
        <div class="bar"><i style="width:{pct(modDone, modTotal)}%"></i></div>
        <div class="muted small" style="margin:6px 0 18px">{modDone}/{modTotal} complete</div>
        <div class="label">{curModule?.title ?? ''}</div>
        {#each curModule?.lessons ?? [] as l (l.id)}
          <button class="ls" class:on={l.id === lesson.id} onclick={() => open(l.id)}>
            <span class="lt">{l.title}{#if isDone(l)} <span class="tickmark">✓</span>{/if}</span>
            <span class="muted small">{l.video ? 'Video' : 'Reading'}{l.checklist ? ' + checklist' : ''}{l.questions ? ' + quiz' : ''} · {l.minutes} min</span>
          </button>
        {/each}
      </aside>
    </div>
  {:else if openCourse}
    <!-- a course page -->
    {@const st = stats(openCourse)}
    <div class="page">
      <div class="hero">
        <button class="link back" onclick={leaveCourse}>← Skills</button>
        <div class="kick">{tierNames[openCourse.tier] ? `TIER ${openCourse.tier} · ${tierNames[openCourse.tier][0].toUpperCase()}` : 'COURSE'}{openCourse.author ? ` · BY ${openCourse.author.toUpperCase()}` : ''}</div>
        <h1>{openCourse.title}</h1>
        {#if openCourse.summary}<p class="muted">{openCourse.summary}</p>{/if}
        <div class="tags">{#each openCourse.modules.slice(0, 5) as m}<span>{m.title}</span>{/each}</div>
        <div><button class="btn" onclick={() => startCourse(openCourse)}>▶ {st.done > 0 ? 'Resume' : 'Start'}</button></div>
      </div>
      <div class="label">YOUR PROGRESS</div>
      <div class="stats">
        <div class="row"><div class="ring">{pct(st.done, st.counted)}%</div><div><b>Course progress</b><div class="muted small">{st.sections} section{st.sections === 1 ? '' : 's'} · about {Math.round(lessonsOf(openCourse).reduce((a, l) => a + l.minutes, 0) / 6) / 10} hours</div></div></div>
        <div><div class="row small"><b>Lessons</b><span class="grow"></span><span class="muted">{st.done} of {st.counted}</span></div><div class="bar"><i style="width:{pct(st.done, st.counted)}%"></i></div></div>
        <div><div class="row small"><b>Checklists</b><span class="grow"></span><span class="muted">{st.checklistsDone} of {st.checklists}</span></div><div class="bar"><i style="width:{pct(st.checklistsDone, st.checklists)}%"></i></div></div>
        <div><div class="row small"><b>Quizzes</b><span class="grow"></span><span class="muted">{st.quizzesDone} of {st.quizzes}</span></div><div class="bar"><i style="width:{pct(st.quizzesDone, st.quizzes)}%"></i></div></div>
      </div>
      <div class="label">CONTENTS · {openCourse.modules.length}</div>
      <div class="list">
        {#each openCourse.modules as m, mi (m.id)}
          {@const cl = m.lessons.filter((l) => l.checklist > 0).length}
          {@const qz = m.lessons.filter((l) => l.questions > 0).length}
          <button class="it" onclick={() => startModule(m)} aria-label={m.title}>
            <span class="num">{mi + 1}</span>
            <span class="grow"><b>{m.title}</b>{#if m.summary}<span class="muted small block">{m.summary}</span>{/if}<span class="muted small block">{m.lessons.length} lesson{m.lessons.length === 1 ? '' : 's'}{cl ? ` · ${cl} checklist${cl === 1 ? '' : 's'}` : ''}{qz ? ` · ${qz} quiz${qz === 1 ? '' : 'zes'}` : ''}</span></span>
            <span class="btn dark">{m.lessons.some((l) => isDone(l)) ? 'Resume' : 'Start'}</span>
          </button>
        {/each}
      </div>
    </div>
  {:else}
    <!-- the shelf, readiness and checklists -->
    <div class="page">
      <div class="row">
        <h1>Academy</h1>
        <span class="grow"></span>
        <button class="btn dark" onclick={() => (training = true)} title="See what each person has learned and still needs to learn">Training sign-off</button>
      </div>
      {#if error}<div class="notice">{error}</div>{/if}
      <div class="row who">
        {#if learner.people.length}
          <label class="row">Learning as
            <select class="sel" value={learner.name} onchange={(e) => pickLearner(e.currentTarget.value)} aria-label="Who is learning">
              <option value="">Nobody chosen (everyone together)</option>
              {#each learner.people as n}<option value={n}>{n}</option>{/each}
            </select>
          </label>
        {:else}
          <span class="muted small">Add people on the Family page to keep each person's progress apart.</span>
        {/if}
        <span class="grow"></span>
        {#if !inShare}
          <button class="btn dark" onclick={installPacks} title="Add the course packs (.ccourse files) waiting in the user\academy-packs folder">Install course packs</button>
          <button class="btn dark" onclick={sharePack} title="Put the courses you added into one file to give to another household">Share my courses</button>
        {/if}
      </div>
      {#each packNotes as n}<p class="muted small">{n}</p>{/each}

      <div class="chips">
        <button class="chip" class:on={tab === 'skills'} onclick={() => (tab = 'skills')}>Skills</button>
        <button class="chip" class:on={tab === 'readiness'} onclick={() => (tab = 'readiness')}>Readiness</button>
        <button class="chip" class:on={tab === 'checklists'} onclick={() => (tab = 'checklists')}>Checklists</button>
      </div>

      {#if tab === 'skills'}
        {#if pathLessons.length}
          <div class="path">
            <div class="label">YOUR PATH</div>
            <div class="row">
              {#each [3, 2, 1] as t, i}
                {@const ls = courses.filter((c) => c.tier === t).flatMap(lessonsOf).filter(countable)}
                {#if ls.length}
                  <span class="pill" class:on={pathNext && pathTierOf(pathNext.id) === t}>TIER {t} · {ls.filter(isDone).length}/{ls.length}</span>
                  {#if i < 2}<span class="muted">›</span>{/if}
                {/if}
              {/each}
              <span class="grow"></span>
              {#if pathNext}
                <span class="muted small">Next: {pathNext.title}</span>
                <button class="btn" onclick={() => open(pathNext!.id)}>{pathDone ? 'Continue' : 'Start'}</button>
              {:else}
                <span class="muted small">You have finished the whole path.</span>
              {/if}
            </div>
            <div class="muted small">Pick any course below, or follow the path in order. Nothing is locked.</div>
          </div>
        {/if}
        <input class="search" type="search" placeholder="Search courses and lessons…" bind:value={search} aria-label="Search courses and lessons" />
        <div class="chips">
          <button class="chip" class:on={tierFilter === 0} onclick={() => (tierFilter = 0)}>All</button>
          {#each [3, 2, 1] as t}<button class="chip" class:on={tierFilter === t} onclick={() => (tierFilter = t)}>Tier {t}</button>{/each}
        </div>
        {#each tierGroups as g (g.tier)}
          <h2 class="tierhead">{#if g.tier}<span class="tier">TIER {g.tier}</span>{/if}{g.title}</h2>
          {#if g.note}<div class="muted small">{g.note}</div>{/if}
          <div class="cards">
            {#each g.list as c (c.id)}
              {@const st = stats(c)}
              <div class="card">
                <button class="thumb" onclick={() => (courseId = c.id)} aria-label={`Open the course ${c.title}`}></button>
                <div class="cbody">
                  <button class="ctitle" onclick={() => (courseId = c.id)}>{c.title}</button>
                  <div class="tags">{#each c.modules.slice(0, 3) as m}<span>{m.title}</span>{/each}</div>
                  <div class="muted small">{st.sections} section{st.sections === 1 ? '' : 's'} · {st.lessons} lesson{st.lessons === 1 ? '' : 's'}</div>
                  <div class="row">
                    {#if st.done > 0}<div class="bar grow"><i style="width:{pct(st.done, st.counted)}%"></i></div>{:else}<span class="muted small grow">Not started</span>{/if}
                    <button class="btn" onclick={() => startCourse(c)}>{st.done > 0 ? 'Resume' : 'Start'}</button>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/each}
        {#if loaded && courses.length === 0 && !error}<p class="muted">No courses found. A course is a folder in the <code>academy</code> folder on the drive.</p>{/if}
        {#if loaded && courses.length > 0 && tierGroups.length === 0}<p class="muted">Nothing matches that search.</p>{/if}
        {#if problems.length}
          <details class="probs">
            <summary>{problems.length} course file{problems.length === 1 ? ' was' : 's were'} left out</summary>
            <p class="muted small">The rest of each course still works. Fix the file and it appears here by itself next time.</p>
            <ul>{#each problems as p}<li><code>{p.place}</code>: {p.message}</li>{/each}</ul>
          </details>
        {/if}
      {:else if tab === 'readiness'}
        <div class="score">
          <div class="ring big">{Math.round(earned)}</div>
          <div><b class="lg">Your readiness score</b><div class="muted small">{level} · {readinessPct}% of the points in the courses</div></div>
          <div class="areas">
            {#each areas as a (a.id)}
              <div><div class="row small"><span class="grow">{a.title}</span>{a.p}%</div><div class="bar"><i style="width:{a.p}%"></i></div></div>
            {/each}
          </div>
        </div>
        {#if pathNext}
          {@const nc = courseOfLesson(pathNext.id)}
          <h2 class="tierhead">Pick up where you left off</h2>
          <div class="resume">
            <div class="kick">{nc ? nc.title.toUpperCase() : ''}{nc && tierNames[nc.tier] ? ` · TIER ${nc.tier}` : ''}</div>
            <div class="rtitle">{pathNext.title}</div>
            <div class="muted">{moduleOfLesson(pathNext.id)?.title ?? ''}</div>
            <div class="row"><button class="btn" onclick={() => open(pathNext!.id)}>{pathDone ? 'Resume' : 'Start'}</button><span class="muted small">{pathDone} of {pathLessons.length} lessons on the path</span></div>
          </div>
        {/if}
      {:else}
        <div class="muted small">Every lesson with a checklist. Tick items inside the lesson; each tick moves your readiness score.</div>
        <div class="list">
          {#each checklistLessons as l (l.id)}
            <button class="it" onclick={() => open(l.id)} aria-label={l.title}>
              <span class="num">{l.checks_done >= l.checklist ? '✓' : ''}</span>
              <span class="grow"><b>{l.title}</b><span class="muted small block">{moduleOfLesson(l.id)?.title ?? ''}</span></span>
              <span class="muted small">{l.checks_done} of {l.checklist}</span>
              <span class="btn dark">Open</span>
            </button>
          {/each}
          {#if checklistLessons.length === 0}<p class="muted" style="padding:16px">No checklists yet.</p>{/if}
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .ac { --a-bg: #141517; --a-side: #101113; --a-panel: #1c1e21; --a-panel2: #232629; --a-line: #303338; --a-text: #eceae6; --a-muted: #9a9ea5; --a-orange: #67b4ff; --a-orange2: #3f8fe0; --a-ok: #6fbf73;
    color: var(--a-text); background: var(--a-bg); margin: 0; padding: 26px 34px 60px; }
  .ac .muted { color: var(--a-muted); }
  .ac .small { font-size: 12px; }
  .ac .block { display: block; }
  .ac .grow { flex: 1; }
  .ac .row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .ac .end { justify-content: flex-end; }
  .ac h1 { font-family: inherit; text-transform: none; letter-spacing: 0; font-size: 28px; margin: 0; color: var(--a-text); }
  .ac h2 { font-family: inherit; text-transform: none; letter-spacing: 0; font-size: 19px; margin: 22px 0 6px; color: var(--a-text); }
  .ac .page { display: flex; flex-direction: column; gap: 14px; max-width: 1180px; }
  .ac .label { font-size: 11px; letter-spacing: .1em; font-weight: 700; color: var(--a-muted); text-transform: uppercase; font-family: inherit; }
  .ac .btn { background: var(--a-orange); color: #07131d; font-weight: 700; border: 0; border-radius: 8px; padding: 8px 18px; font-family: inherit; text-transform: none; letter-spacing: 0; font-size: 14px; }
  .ac .btn:hover:not(:disabled) { background: #8cc3ff; color: #07131d; border: 0; }
  .ac .btn.dark { background: var(--a-panel2); color: var(--a-text); border: 1px solid var(--a-line); font-weight: 600; }
  .ac .btn.dark:hover:not(:disabled) { background: #2b2f33; color: var(--a-text); border: 1px solid var(--a-line); }
  .ac .link { background: none; border: 0; color: var(--a-muted); padding: 0; font-size: 14px; }
  .ac .link:hover { color: var(--a-text); }
  .ac .sel { background: var(--a-panel); color: var(--a-text); border: 1px solid var(--a-line); border-radius: 8px; padding: 7px 10px; }
  .ac .search { background: var(--a-panel); color: var(--a-text); border: 1px solid var(--a-line); border-radius: 10px; padding: 12px 14px; font: inherit; width: 100%; }
  .ac .chips { display: flex; gap: 8px; flex-wrap: wrap; }
  .ac .chip { border: 1px solid var(--a-line); border-radius: 99px; padding: 5px 16px; color: var(--a-muted); background: var(--a-panel); }
  .ac .chip.on { background: var(--a-orange); color: #07131d; border-color: var(--a-orange); font-weight: 700; }
  .ac .tier { display: inline-block; background: var(--a-orange); color: #07131d; font-size: 11px; font-weight: 800; border-radius: 5px; padding: 2px 7px; margin-right: 8px; letter-spacing: .04em; vertical-align: middle; }
  .ac .tags { display: flex; gap: 5px; flex-wrap: wrap; }
  .ac .tags span { font-size: 10px; letter-spacing: .05em; background: var(--a-panel2); color: var(--a-muted); padding: 3px 8px; border-radius: 5px; text-transform: uppercase; }
  .ac .bar { height: 6px; background: var(--a-line); border-radius: 3px; overflow: hidden; }
  .ac .bar i { display: block; height: 100%; background: linear-gradient(90deg, var(--a-orange2), var(--a-orange)); }
  .ac .notice { padding: 10px 12px; border: 1px solid var(--a-line); border-radius: 8px; background: var(--a-panel2); font-size: 13px; }
  .ac .path { background: var(--a-panel); border: 1px solid var(--a-line); border-radius: 14px; padding: 14px 18px; display: flex; flex-direction: column; gap: 8px; }
  .ac .pill { font-size: 12px; padding: 3px 11px; border-radius: 99px; background: var(--a-panel2); color: var(--a-muted); }
  .ac .pill.on { background: var(--a-orange); color: #07131d; font-weight: 800; }
  .ac .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 16px; }
  .ac .card { background: var(--a-panel); border: 1px solid var(--a-line); border-radius: 14px; overflow: hidden; display: flex; flex-direction: column; }
  .ac .thumb { height: 130px; border: 0; padding: 0; background: #0b0b0c url('/cindrix-banner.png') center / cover; }
  .ac .cbody { padding: 14px; display: flex; flex-direction: column; gap: 8px; flex: 1; }
  .ac .ctitle { background: none; border: 0; padding: 0; text-align: left; font-size: 15px; font-weight: 700; color: var(--a-text); }
  .ac .probs { background: var(--a-panel); border: 1px solid var(--a-line); border-radius: 10px; padding: 12px 14px; }
  .ac .who { justify-content: space-between; }
  /* course page */
  .ac .hero { min-height: 280px; border-radius: 16px; background: linear-gradient(90deg, #141517 38%, #14151700 78%), url('/cindrix-banner.png') 100% 40% / auto 150% no-repeat #0b0b0c; padding: 24px 28px; display: flex; flex-direction: column; justify-content: flex-end; gap: 8px; border: 1px solid var(--a-line); }
  .ac .hero .back { align-self: flex-start; margin-bottom: auto; }
  .ac .hero p { max-width: 520px; margin: 0; }
  .ac .kick { color: var(--a-orange); font-weight: 700; font-size: 12px; letter-spacing: .1em; }
  .ac .stats { display: grid; grid-template-columns: 1.2fr 1fr 1fr 1fr; background: var(--a-panel); border: 1px solid var(--a-line); border-radius: 14px; }
  .ac .stats > div { padding: 16px 20px; border-right: 1px solid var(--a-line); display: flex; flex-direction: column; gap: 8px; justify-content: center; }
  .ac .stats > div:last-child { border: 0; }
  .ac .stats > div.row { flex-direction: row; }
  .ac .ring { width: 54px; height: 54px; border-radius: 50%; border: 5px solid var(--a-line); display: grid; place-items: center; font-weight: 700; color: var(--a-ok); font-size: 13px; flex: none; }
  .ac .ring.big { width: 84px; height: 84px; font-size: 26px; color: var(--a-orange); border-color: #67b4ff55; }
  .ac .list { background: var(--a-panel); border: 1px solid var(--a-line); border-radius: 14px; overflow: hidden; }
  .ac .it { display: flex; align-items: center; gap: 14px; padding: 16px 20px; border: 0; border-bottom: 1px solid var(--a-line); background: transparent; color: var(--a-text); text-align: left; width: 100%; }
  .ac .it:last-child { border-bottom: 0; }
  .ac .it:hover { background: #ffffff06; }
  .ac .num { width: 26px; height: 26px; border: 2px solid var(--a-line); border-radius: 50%; display: grid; place-items: center; color: var(--a-muted); font-size: 12px; flex: none; }
  /* readiness */
  .ac .score { display: flex; gap: 26px; align-items: center; background: var(--a-panel); border: 1px solid var(--a-line); border-radius: 14px; padding: 18px 24px; flex-wrap: wrap; }
  .ac .lg { font-size: 18px; }
  .ac .areas { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px 22px; flex: 1; min-width: 280px; }
  .ac .resume { border-radius: 16px; background: linear-gradient(90deg, #141517 38%, #14151700 80%), url('/cindrix-banner.png') 100% 40% / auto 150% no-repeat #0b0b0c; padding: 30px 34px; display: flex; flex-direction: column; gap: 10px; border: 1px solid var(--a-line); min-height: 200px; justify-content: center; }
  .ac .rtitle { font-size: 28px; font-weight: 800; }
  /* the lesson player */
  .ac.player { padding: 0; }
  .ac .crumb { display: flex; align-items: center; gap: 12px; padding: 12px 24px; border-bottom: 1px solid var(--a-line); background: var(--a-side); }
  .ac .bar-sep { width: 1px; height: 16px; background: var(--a-line); }
  .ac .player { display: grid; grid-template-columns: minmax(0, 1fr) 290px; }
  .ac .lbody { padding: 26px 40px 60px; max-width: 900px; }
  .ac .lbody p, .ac .lbody li { line-height: 1.65; }
  .ac .ltitle { font-size: 24px; margin: 20px 0 8px; }
  .ac .video { aspect-ratio: 16 / 9; border-radius: 12px; background: #000 url('/cindrix-banner.png') center / cover; position: relative; border: 1px solid var(--a-line); }
  .ac .play { position: absolute; inset: 0; margin: auto; width: 64px; height: 64px; border-radius: 50%; background: #000a; border: 2px solid #fff6; display: grid; place-items: center; font-size: 24px; }
  .ac .side { border-left: 1px solid var(--a-line); background: var(--a-side); padding: 22px 18px; }
  .ac .bigpct { font-size: 28px; font-weight: 800; margin: 4px 0; }
  .ac .ls { display: flex; flex-direction: column; gap: 2px; text-align: left; width: 100%; border: 1px solid transparent; border-radius: 10px; padding: 12px; margin-top: 8px; background: transparent; color: var(--a-muted); }
  .ac .ls.on { border-color: var(--a-orange); background: #67b4ff1f; color: var(--a-text); }
  .ac .lt { font-weight: 600; }
  .ac .tickmark { color: var(--a-ok); }
  .ac .steps { display: grid; grid-template-columns: repeat(auto-fit, minmax(210px, 1fr)); gap: 12px; margin: 10px 0 16px; }
  .ac .step { background: var(--a-panel); border: 1px solid var(--a-line); border-radius: 12px; padding: 16px; display: flex; flex-direction: column; gap: 6px; font-size: 13px; }
  .ac .step b { font-size: 15px; }
  .ac .step .n { width: 30px; height: 30px; border-radius: 50%; background: var(--a-orange); color: #07131d; font-weight: 800; display: grid; place-items: center; }
  .ac .check { background: var(--a-panel); border: 1px solid var(--a-line); border-radius: 12px; margin: 16px 0; overflow: hidden; }
  .ac .chead { display: flex; gap: 10px; align-items: center; padding: 12px 16px; border-bottom: 1px solid var(--a-line); }
  .ac .rd { background: #6fbf731f; color: var(--a-ok); font-size: 10px; font-weight: 800; letter-spacing: .06em; padding: 3px 8px; border-radius: 5px; }
  .ac .ci { display: flex; gap: 14px; padding: 12px 16px; border-bottom: 1px solid var(--a-line); cursor: pointer; align-items: flex-start; }
  .ac .ci:last-child { border-bottom: 0; }
  .ac .ci input { width: 18px; height: 18px; margin-top: 2px; accent-color: var(--a-orange); }
  .ac .ci .ct { display: block; }
  .ac .ci.on .ct { color: var(--a-muted); text-decoration: line-through; }
  .ac .ci .cd { display: block; font-size: 12px; color: var(--a-muted); }
  .ac .shop { background: var(--a-panel2); border: 1px solid var(--a-line); border-radius: 7px; padding: 4px 10px; font-size: 12px; font-weight: 600; align-self: center; color: var(--a-text); }
  .ac .note { padding: 10px 14px; border-left: 3px solid var(--a-orange); background: var(--a-panel); border-radius: 4px; }
  .ac .pic { margin: 12px 0; text-align: center; }
  .ac .pic img { max-width: 100%; max-height: 420px; border-radius: 8px; background: #fff; }
  .ac .nopic { padding: 14px; border: 1px dashed var(--a-line); border-radius: 6px; }
  .ac .quiz { display: flex; flex-direction: column; gap: 14px; background: var(--a-panel); border: 1px solid var(--a-line); border-radius: 12px; padding: 18px; margin-top: 18px; }
  .ac fieldset { border: 1px solid var(--a-line); border-radius: 8px; padding: 10px 12px; display: flex; flex-direction: column; gap: 6px; margin: 0; }
  .ac legend { font-weight: 600; padding: 0 6px; }
  .ac fieldset label { display: flex; gap: 10px; align-items: flex-start; padding: 8px; border-radius: 6px; cursor: pointer; }
  .ac fieldset label.right { background: rgba(60, 180, 90, .2); }
  .ac fieldset label.wrong { background: rgba(220, 70, 60, .2); }
  .ac .why { margin: 0; font-size: 14px; color: var(--a-muted); }
  .ac .result { font-size: 20px; font-weight: 700; margin: 0; }
  @media (max-width: 900px) {
    .ac { padding: 18px 14px 50px; }
    .ac .player { grid-template-columns: 1fr; }
    .ac .side { border-left: 0; border-top: 1px solid var(--a-line); }
    .ac .lbody { padding: 18px 16px 40px; }
    .ac .stats { grid-template-columns: 1fr; }
    .ac .stats > div { border-right: 0; border-bottom: 1px solid var(--a-line); }
  }
  @media print {
    .ac, .ac .lbody { background: #fff !important; color: #111 !important; margin: 0 !important; }
    .ac .player { display: block; }
    .ac .lbody p, .ac .lbody li, .ac .lbody h1, .ac .lbody h2, .ac .lbody b { color: #111 !important; }
    .ac .video, .ac .play { display: none; }
    .ac .muted { color: #444 !important; }
  }
</style>
