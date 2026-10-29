<script lang="ts">
  // Bible study: reads Canonleaf's Bible, commentary, dictionary and cross-reference files from library\bible.
  // Read-only. Guests on the share page get the same screen. Nothing is shown that is not in those files.
  import {
    bibleStatus, bibleBooks, bibleChapter, bibleNotes, bibleCrossrefs, bibleSearch, bibleParse, bibleDefine, bibleStrongs, bibleMentions,
    type BibleStatus, type BibleBook, type BibleVerse, type BibleNote, type BibleCross, type BibleHit, type BibleEntry, type BibleStrongs, type BibleRef, type BibleMention,
  } from '../lib/api';
  import { bibleRequest } from '../lib/chat.svelte';

  let status = $state<BibleStatus | null>(null);
  let error = $state('');
  let bible = $state('');
  let books: BibleBook[] = $state([]);
  let book = $state('JHN');
  let chapter = $state(1);
  let verses: BibleVerse[] = $state([]);
  let selected: number | null = $state(null);
  let goto = $state('');
  let gotoNote = $state('');

  type Tab = 'Commentary' | 'Cross-references' | 'Topics' | 'Look up' | 'Search';
  let tab: Tab = $state('Commentary');
  let commentary = $state('');
  let notes: BibleNote[] = $state([]);
  let notesNote = $state('');
  let cross = $state<BibleCross | null>(null);
  let mentions: BibleMention[] = $state([]);
  let term = $state('');
  let entries: BibleEntry[] = $state([]);
  let strongs: BibleStrongs[] = $state([]);
  let lookedUp = $state(false);
  let query = $state('');
  let hits: BibleHit[] = $state([]);
  let searched = $state(false);
  let busy = $state(false);

  const bibles = $derived((status?.modules ?? []).filter((m) => m.kind === 'bible'));
  const commentaries = $derived((status?.modules ?? []).filter((m) => m.kind === 'commentary'));
  const bookInfo = $derived(books.find((b) => b.code === book));
  const verseNo = $derived(selected !== null ? Number(selected) : null);

  async function start() {
    try {
      status = await bibleStatus();
      if (!status.found) return;
      bible = status.default_bible ?? status.modules.find((m) => m.kind === 'bible')?.id ?? '';
      commentary = commentaries[0]?.id ?? '';
      try {
        const saved = JSON.parse(sessionStorage.getItem('cindrix.bible') ?? 'null');
        if (saved?.book) { book = saved.book; chapter = saved.chapter; if (saved.bible && bibles.some((b) => b.id === saved.bible)) bible = saved.bible; }
      } catch { /* start at John 1 */ }
      await loadBooks();
      await load();
      applyRequest();
    } catch (e) { error = String(e); }
  }
  $effect(() => { start(); });

  async function loadBooks() {
    books = await bibleBooks(bible);
    if (books.length && !books.some((b) => b.code === book)) { book = books[0].code; chapter = 1; }
  }
  async function load() {
    error = '';
    selected = null;
    notes = []; cross = null; notesNote = ''; mentions = [];
    try {
      verses = await bibleChapter(bible, book, chapter);
      try { sessionStorage.setItem('cindrix.bible', JSON.stringify({ bible, book, chapter })); } catch { /* not remembered */ }
    } catch (e) { verses = []; error = String(e); }
  }
  async function changeBible() { await loadBooks(); await load(); }
  async function changeBook() { chapter = 1; await load(); }

  function step(d: number) {
    const i = books.findIndex((b) => b.code === book);
    const n = bookInfo?.chapters ?? 1;
    if (d > 0 && chapter < n) chapter++;
    else if (d > 0 && i >= 0 && i < books.length - 1) { book = books[i + 1].code; chapter = 1; }
    else if (d < 0 && chapter > 1) chapter--;
    else if (d < 0 && i > 0) { book = books[i - 1].code; chapter = books[i - 1].chapters; }
    else return;
    load();
  }

  async function open(code: string, ch: number, v: number | null) {
    if (!books.some((b) => b.code === code)) { gotoNote = 'That book is not in this Bible.'; return; }
    gotoNote = '';
    book = code; chapter = ch;
    await load();
    if (v !== null) await pick(v);
  }
  async function go() {
    if (!goto.trim()) return;
    const p = await bibleParse(goto);
    if (!p) { gotoNote = 'That is not a place in the Bible. Try “John 3:16” or “Psalm 23”.'; return; }
    await open(p.book, p.chapter, p.verse);
    goto = '';
  }
  function applyRequest() {
    if (bibleRequest.term) { const t = bibleRequest.term; bibleRequest.term = ''; tab = 'Look up'; term = t; lookup(); }
    if (bibleRequest.ref) {
      const [b, c, v] = bibleRequest.ref.split('.');
      bibleRequest.ref = '';
      if (b && Number(c) > 0) open(b, Number(c), v ? Number(v) : null);
    }
  }
  $effect(() => { if (status?.found && (bibleRequest.ref || bibleRequest.term)) applyRequest(); });

  async function pick(v: number) {
    selected = v;
    await side();
  }
  async function side() {
    if (verseNo === null) return;
    try {
      if (commentary) {
        notes = await bibleNotes(commentary, book, chapter, verseNo);
        notesNote = notes.length ? '' : 'This commentary has nothing on this verse.';
      } else { notes = []; notesNote = 'No commentary is on the drive.'; }
      cross = await bibleCrossrefs(bible, book, chapter, verseNo);
      mentions = await bibleMentions([`${book}.${chapter}.${verseNo}`]);
    } catch (e) { notesNote = String(e); }
  }
  async function lookup() {
    const t = term.trim();
    if (!t) return;
    busy = true; lookedUp = false;
    try {
      const n = t.replace(/^[gh]?0*(\d+)$/i, (m) => m);
      entries = await bibleDefine(t);
      strongs = /^[gh]\d{1,4}$/i.test(n) ? await bibleStrongs(n) : [];
    } catch (e) { error = String(e); entries = []; strongs = []; }
    lookedUp = true; busy = false;
  }
  async function find() {
    const q = query.trim();
    if (!q) return;
    busy = true; searched = false;
    try { hits = await bibleSearch(bible, q); } catch (e) { error = String(e); hits = []; }
    searched = true; busy = false;
  }
  function goRef(r: BibleRef) {
    const [b, c, v] = r.code.split('.');
    if (b) open(b, Number(c), v ? Number(v) : null);
  }
  const bibleTitle = $derived(bibles.find((b) => b.id === bible));
</script>

<section>
  <h1>Bible study</h1>
  {#if error}<p class="warn" role="alert">{error}</p>{/if}
  {#if status && !status.found}
    <div class="panel empty">
      <strong>No Bible is on this drive yet.</strong>
      <p class="muted">The Bible, commentary and dictionary files from Canonleaf are read from {status.where_to_put}. Once they are there, press the button below. The program only reads them.</p>
      <div class="row"><button class="btn" onclick={async () => { status = await bibleStatus(true); if (status.found) start(); }}>Look again</button></div>
    </div>
  {:else if status}
    <div class="panel bar">
      <select class="input" aria-label="Bible" bind:value={bible} onchange={changeBible}>
        {#each bibles as b (b.id)}<option value={b.id}>{b.title}</option>{/each}
      </select>
      <select class="input" aria-label="Book" bind:value={book} onchange={changeBook}>
        {#each books as b (b.code)}<option value={b.code}>{b.name}</option>{/each}
      </select>
      <select class="input narrow" aria-label="Chapter" bind:value={chapter} onchange={load}>
        {#each Array.from({ length: bookInfo?.chapters ?? 1 }, (_, i) => i + 1) as n (n)}<option value={n}>{n}</option>{/each}
      </select>
      <button class="btn" onclick={() => step(-1)} aria-label="Previous chapter" title="The chapter before">‹</button>
      <button class="btn" onclick={() => step(1)} aria-label="Next chapter" title="The chapter after">›</button>
      <form class="goto" onsubmit={(e) => { e.preventDefault(); go(); }}>
        <input class="input" type="search" placeholder="Go to… John 3:16" aria-label="Go to a place" bind:value={goto} />
        <button class="btn" type="submit">Go</button>
      </form>
    </div>
    {#if gotoNote}<p class="muted">{gotoNote}</p>{/if}

    <div class="cols">
      <div class="panel reading">
        <h2>{bookInfo?.name ?? book} {chapter}</h2>
        {#if !verses.length && !error}<p class="muted">This chapter is not in this Bible.</p>{/if}
        {#each verses as v (v.number)}
          <p class="v" class:on={selected !== null && String(selected) === v.number}>
            <button class="num" onclick={() => pick(Number(v.number))} title="Show notes for this verse">{v.number}</button>
            <span>{v.text}</span>
          </p>
        {/each}
        {#if bibleTitle?.attribution}<p class="muted fine">{bibleTitle.title}: {bibleTitle.attribution}</p>{/if}
      </div>

      <div class="panel side">
        <div class="tabs" role="tablist">
          {#each ['Commentary', 'Cross-references', 'Topics', 'Look up', 'Search'] as t (t)}
            <button class="btn" class:active={tab === t} role="tab" aria-selected={tab === t} onclick={() => (tab = t as Tab)}>{t}</button>
          {/each}
        </div>

        {#if tab === 'Commentary'}
          {#if commentaries.length}
            <select class="input" aria-label="Commentary" bind:value={commentary} onchange={side}>
              {#each commentaries as c (c.id)}<option value={c.id}>{c.title}</option>{/each}
            </select>
          {/if}
          {#if verseNo === null}
            <p class="muted">Tap a verse number to read notes on it.</p>
          {:else}
            <h3>{bookInfo?.name} {chapter}:{verseNo}</h3>
            {#each notes as n, i (i)}
              <div class="note"><div class="label">{n.label}</div><p>{n.text}</p></div>
            {/each}
            {#if notesNote}<p class="muted">{notesNote}</p>{/if}
          {/if}
        {:else if tab === 'Cross-references'}
          {#if verseNo === null}
            <p class="muted">Tap a verse number to see related verses.</p>
          {:else if !cross?.source}
            <p class="muted">No cross-reference list is on the drive for this Bible.</p>
          {:else}
            <h3>{bookInfo?.name} {chapter}:{verseNo}</h3>
            {#each cross.items as x, i (i)}
              <div class="note">
                {#if x.anchor}<div class="label">{x.anchor}</div>{/if}
                <p>{#each x.refs as r, j (j)}{#if r.code}<button class="link" onclick={() => goRef(r)}>{r.text}</button>{:else}<span>{r.text}</span>{/if}{' '}{/each}</p>
              </div>
            {/each}
            {#if !cross.items.length}<p class="muted">Nothing is listed for this verse.</p>{/if}
            <p class="muted fine">From {cross.source}.</p>
          {/if}
        {:else if tab === 'Topics'}
          {#if verseNo === null}
            <p class="muted">Tap a verse number to see the topics and dictionary entries that mention it.</p>
          {:else}
            <h3>{bookInfo?.name} {chapter}:{verseNo}</h3>
            {#each mentions as m, i (i)}
              <div class="note">
                <div class="label">{m.title} <span class="fine">· {m.source}</span></div>
                {#each m.points as p, j (j)}<p>{p}</p>{/each}
                {#if m.excerpt}<p>{m.excerpt}</p>{/if}
              </div>
            {/each}
            {#if !mentions.length}<p class="muted">None of the topic books or dictionaries on the drive mention this verse.</p>{/if}
          {/if}
        {:else if tab === 'Look up'}
          <form class="goto" onsubmit={(e) => { e.preventDefault(); lookup(); }}>
            <input class="input" type="search" placeholder="A word, name, or number like G25" aria-label="Look up" bind:value={term} />
            <button class="btn" type="submit" disabled={busy}>Look up</button>
          </form>
          {#each strongs as s, i (i)}
            <div class="note">
              <div class="label">{s.number} · {s.source}</div>
              <p><b>{s.lemma}</b> {s.transliteration}</p>
              {#if s.gloss}<p>{s.gloss}</p>{/if}
              {#if s.morphology}<p class="muted fine">{s.morphology}</p>{/if}
            </div>
          {/each}
          {#each entries as e, i (i)}
            <div class="note">
              <div class="label">{e.title} · {e.source}</div>
              <p>{e.body}</p>
              {#if e.refs.length}<p class="refs">{#each e.refs as r, j (j)}{#if r.code}<button class="link" onclick={() => goRef(r)}>{r.text}</button>{:else}<span>{r.text}</span>{/if}{' '}{/each}</p>{/if}
            </div>
          {/each}
          {#if lookedUp && !entries.length && !strongs.length}<p class="muted">Nothing was found for that in the books on this drive.</p>{/if}
        {:else}
          <form class="goto" onsubmit={(e) => { e.preventDefault(); find(); }}>
            <input class="input" type="search" placeholder="Words in a verse" aria-label="Search the Bible" bind:value={query} />
            <button class="btn" type="submit" disabled={busy}>Search</button>
          </form>
          {#if busy}<p class="muted">Searching…</p>{/if}
          {#each hits as h, i (i)}
            <button class="hit" onclick={() => open(h.book, h.chapter, Number(h.verse) || null)}>
              <b>{h.name} {h.chapter}:{h.verse}</b> {h.text}
            </button>
          {/each}
          {#if searched && !hits.length}<p class="muted">No verse has all of those words.</p>{/if}
          {#if hits.length >= 60}<p class="muted fine">Showing the first 60. Add a word to narrow it.</p>{/if}
        {/if}
      </div>
    </div>
  {:else}
    <p class="muted">Looking…</p>
  {/if}
</section>

<style>
  .bar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .narrow { width: 5.5em; }
  .goto { display: flex; gap: 6px; margin-left: auto; }
  .cols { display: grid; grid-template-columns: minmax(0, 3fr) minmax(0, 2fr); gap: 12px; align-items: start; }
  @media (max-width: 800px) { .cols { grid-template-columns: 1fr; } .goto { margin-left: 0; width: 100%; } }
  .reading { line-height: 1.6; }
  .v { margin: 0 0 6px; }
  .v.on { background: color-mix(in srgb, var(--accent) 18%, transparent); border-radius: 4px; }
  .num { background: none; border: 0; color: var(--blue); font-size: 0.75em; vertical-align: super; cursor: pointer; padding: 0 4px 0 0; }
  .tabs { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 10px; }
  .tabs .active { border-color: var(--accent); }
  .note { border-top: 1px solid var(--line); padding: 8px 0; }
  .note p { margin: 4px 0; white-space: pre-wrap; }
  .link { background: none; border: 0; color: var(--blue); cursor: pointer; padding: 0; text-decoration: underline; font: inherit; }
  .hit { display: block; width: 100%; text-align: left; background: none; border: 0; border-top: 1px solid var(--line); color: var(--text); padding: 8px 0; cursor: pointer; font: inherit; }
  .fine { font-size: 0.85em; }
</style>
