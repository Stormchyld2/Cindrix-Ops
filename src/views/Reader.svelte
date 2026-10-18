<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import * as pdfjs from 'pdfjs-dist';
  import type { PDFDocumentProxy, RenderTask } from 'pdfjs-dist';
  import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url';
  import { shelfRead, shelfSaveText, marksList, marksAdd, marksRemove, marksSetLast, type Book, type Mark } from '../lib/api';

  pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

  let { book, onBack }: { book: Book; onBack: () => void } = $props();

  const isPdf = book.kind === 'pdf';
  let doc: PDFDocumentProxy | null = $state.raw(null);
  let pages = $state(1);
  let page = $state(Math.max(1, book.last_page || 1));
  let zoom = $state(1);
  let night = $state(readPref('night') === '1');
  let textSize = $state(Number(readPref('textsize')) || 18);
  let error = $state('');
  let loading = $state(true);
  let wrap: HTMLDivElement | undefined = $state();
  let canvas: HTMLCanvasElement | undefined = $state();
  let task: RenderTask | null = null;
  let renderId = 0;

  let text = $state(''); // for md/txt books
  let pageText: string[] = $state.raw([]);
  let indexed = $state(0);

  let marks: Mark[] = $state([]);
  let panel: '' | 'notes' | 'search' = $state('');
  let noteText = $state('');
  let query = $state('');
  let results: { page: number; snippet: string }[] = $state([]);
  let searched = $state(false);
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  function readPref(k: string): string {
    try { return localStorage.getItem('cindrix-' + k) ?? ''; } catch { return ''; }
  }
  function writePref(k: string, v: string) {
    try { localStorage.setItem('cindrix-' + k, v); } catch { /* storage may be unavailable */ }
  }

  onMount(async () => {
    try {
      const bytes = await shelfRead(book.id);
      if (isPdf) {
        doc = await pdfjs.getDocument({ data: bytes }).promise;
        pages = doc.numPages;
        page = Math.min(Math.max(1, page), pages);
        loading = false;
        extractText(doc);
      } else {
        text = new TextDecoder().decode(bytes);
        loading = false;
      }
      marks = await marksList(book.id);
    } catch (e) {
      error = 'This book could not be opened: ' + String(e);
      loading = false;
    }
  });

  onDestroy(() => {
    task?.cancel();
    clearTimeout(saveTimer);
    if (isPdf) marksSetLast(book.id, page).catch(() => {});
    doc?.destroy();
  });

  // Read the text of every page in the background: this makes in-book search work,
  // and lets the assistant search the book (saved once for books the person added).
  async function extractText(d: PDFDocumentProxy) {
    const out: string[] = [];
    for (let i = 1; i <= d.numPages; i++) {
      try {
        const p = await d.getPage(i);
        const c = await p.getTextContent();
        out.push(c.items.map((it) => ('str' in it ? it.str : '')).join(' ').replace(/\s+/g, ' ').trim());
      } catch { out.push(''); }
      indexed = i;
      if (i % 10 === 0) await new Promise((r) => setTimeout(r, 0));
    }
    pageText = out;
    if (book.mine && !book.searchable && out.some((t) => t.length > 20)) {
      try { await shelfSaveText(book.id, out); } catch { /* search just stays off for this book */ }
    }
  }

  async function render() {
    if (!doc || !canvas || !wrap) return;
    const mine = ++renderId;
    task?.cancel();
    try {
      const pg = await doc.getPage(page);
      if (mine !== renderId) return;
      const base = pg.getViewport({ scale: 1 });
      const avail = Math.max(240, wrap.clientWidth - 28);
      const scale = (avail / base.width) * zoom;
      const dpr = window.devicePixelRatio || 1;
      const vp = pg.getViewport({ scale: scale * dpr });
      canvas.width = Math.floor(vp.width);
      canvas.height = Math.floor(vp.height);
      canvas.style.width = `${Math.floor(vp.width / dpr)}px`;
      canvas.style.height = `${Math.floor(vp.height / dpr)}px`;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      task = pg.render({ canvasContext: ctx, viewport: vp });
      await task.promise;
    } catch (e) {
      if (String((e as { name?: string })?.name) !== 'RenderingCancelledException') error = 'Could not draw this page: ' + String(e);
    }
  }

  $effect(() => {
    // re-draw when the page, zoom or the loaded document changes
    void page; void zoom; void doc; void canvas;
    render();
  });

  $effect(() => {
    void page;
    clearTimeout(saveTimer);
    if (isPdf && doc) saveTimer = setTimeout(() => marksSetLast(book.id, page).catch(() => {}), 700);
  });

  $effect(() => { writePref('night', night ? '1' : '0'); });
  $effect(() => { writePref('textsize', String(textSize)); });

  function go(n: number) { page = Math.min(Math.max(1, Math.round(n) || 1), pages); }
  function onKey(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    if (t && ['INPUT', 'TEXTAREA', 'SELECT'].includes(t.tagName)) return;
    if (!isPdf) return;
    if (e.key === 'ArrowRight' || e.key === 'PageDown') go(page + 1);
    if (e.key === 'ArrowLeft' || e.key === 'PageUp') go(page - 1);
  }

  let bookmarked = $derived(marks.some((m) => m.kind === 'bookmark' && m.page === page));
  async function toggleBookmark() {
    const m: Mark = { kind: 'bookmark', page, text: '' };
    if (bookmarked) await marksRemove(book.id, m); else await marksAdd(book.id, m);
    marks = await marksList(book.id);
  }
  async function addNote() {
    const t = noteText.trim();
    if (!t) return;
    await marksAdd(book.id, { kind: 'note', page, text: t });
    noteText = '';
    marks = await marksList(book.id);
  }
  async function removeMark(m: Mark) {
    await marksRemove(book.id, m);
    marks = await marksList(book.id);
  }

  function runSearch() {
    const q = query.trim().toLowerCase();
    searched = true;
    results = [];
    if (!q) return;
    if (isPdf) {
      pageText.forEach((t, i) => {
        const at = t.toLowerCase().indexOf(q);
        if (at >= 0) results.push({ page: i + 1, snippet: '…' + t.slice(Math.max(0, at - 50), at + q.length + 70) + '…' });
      });
    } else {
      const at = text.toLowerCase().indexOf(q);
      if (at >= 0) results.push({ page: 1, snippet: '…' + text.slice(Math.max(0, at - 50), at + q.length + 70) + '…' });
    }
    results = results.slice(0, 60);
  }

  function blocks(md: string): { h: boolean; t: string }[] {
    return md.split(/\n{2,}/).map((b) => b.trim()).filter(Boolean).map((b) => (b.startsWith('#') ? { h: true, t: b.replace(/^#+\s*/, '') } : { h: false, t: b }));
  }
  let pageMarks = $derived(marks.filter((m) => m.page === page && m.kind === 'note'));
</script>

<svelte:window onkeydown={onKey} />

<section class="reader">
  <div class="bar">
    <button class="btn" onclick={onBack}>‹ Shelf</button>
    <div class="title"><strong>{book.title}</strong><span class="muted">{book.license}</span></div>
    <div class="spacer"></div>
    {#if isPdf}
      <button class="btn" class:on={panel === 'search'} onclick={() => (panel = panel === 'search' ? '' : 'search')}>Search</button>
      <button class="btn" class:on={bookmarked} onclick={toggleBookmark} aria-pressed={bookmarked}>{bookmarked ? 'Bookmarked' : 'Bookmark'}</button>
      <button class="btn" class:on={panel === 'notes'} onclick={() => (panel = panel === 'notes' ? '' : 'notes')}>Notes ({marks.length})</button>
      <button class="btn" onclick={() => (zoom = Math.max(0.5, +(zoom - 0.25).toFixed(2)))} aria-label="Zoom out">−</button>
      <button class="btn" onclick={() => (zoom = Math.min(4, +(zoom + 0.25).toFixed(2)))} aria-label="Zoom in">+</button>
      <button class="btn" class:on={night} onclick={() => (night = !night)} aria-pressed={night}>Night</button>
    {:else}
      <button class="btn" onclick={() => (textSize = Math.max(12, textSize - 2))} aria-label="Smaller text">A−</button>
      <button class="btn" onclick={() => (textSize = Math.min(32, textSize + 2))} aria-label="Larger text">A+</button>
    {/if}
  </div>

  {#if error}<p class="notice" role="alert">{error}</p>{/if}
  {#if loading}<p class="muted">Opening…</p>{/if}

  <div class="body">
    <div class="page" bind:this={wrap}>
      {#if isPdf}
        <canvas bind:this={canvas} class:night aria-label="Page {page} of {book.title}"></canvas>
      {:else}
        <article class="doc" style="font-size: {textSize}px">
          {#each blocks(text) as b}
            {#if b.h}<h2>{b.t}</h2>{:else}<p>{b.t}</p>{/if}
          {/each}
        </article>
      {/if}
    </div>

    {#if panel}
      <aside class="side panel">
        {#if panel === 'search'}
          <form class="row" onsubmit={(e) => { e.preventDefault(); runSearch(); }}>
            <label class="sr" for="bsearch">Search in this book</label>
            <input id="bsearch" class="input grow" bind:value={query} placeholder="Search in this book" />
            <button class="btn primary" type="submit">Find</button>
          </form>
          {#if isPdf && indexed < pages}<p class="muted fine">Still reading the book for search: {indexed} of {pages} pages.</p>{/if}
          {#if searched && results.length === 0}<p class="muted">Nothing found.</p>{/if}
          {#each results as r}
            <button class="hit" onclick={() => go(r.page)}><span class="label">PAGE {r.page}</span><span>{r.snippet}</span></button>
          {/each}
        {:else}
          <div class="label">BOOKMARKS</div>
          {#each marks.filter((m) => m.kind === 'bookmark').sort((a, b) => a.page - b.page) as m}
            <div class="mark"><button class="link" onclick={() => go(m.page)}>Page {m.page}</button><button class="mini-btn" onclick={() => removeMark(m)} aria-label="Remove bookmark on page {m.page}">Remove</button></div>
          {:else}<p class="muted fine">None yet.</p>{/each}
          <div class="label top">NOTES</div>
          {#each marks.filter((m) => m.kind === 'note').sort((a, b) => a.page - b.page) as m}
            <div class="note"><button class="link" onclick={() => go(m.page)}>Page {m.page}</button><div>{m.text}</div><button class="mini-btn" onclick={() => removeMark(m)} aria-label="Remove note on page {m.page}">Remove</button></div>
          {:else}<p class="muted fine">None yet.</p>{/each}
          <form class="col" onsubmit={(e) => { e.preventDefault(); addNote(); }}>
            <label class="sr" for="note">Note for page {page}</label>
            <textarea id="note" class="input" bind:value={noteText} placeholder="Note for page {page}"></textarea>
            <button class="btn primary" type="submit">Add note</button>
          </form>
        {/if}
      </aside>
    {/if}
  </div>

  {#if isPdf}
    <div class="nav">
      <button class="btn" onclick={() => go(page - 1)} disabled={page <= 1}>‹ Prev</button>
      <input class="range" type="range" min="1" max={pages} value={page} oninput={(e) => go(Number(e.currentTarget.value))} aria-label="Page slider" />
      <label class="pg">Page <input class="input num" type="number" min="1" max={pages} value={page} onchange={(e) => go(Number(e.currentTarget.value))} aria-label="Go to page" /> of {pages}</label>
      <button class="btn" onclick={() => go(page + 1)} disabled={page >= pages}>Next ›</button>
    </div>
    {#if pageMarks.length}<p class="muted fine">Your note on this page: {pageMarks.map((m) => m.text).join(' · ')}</p>{/if}
  {/if}
</section>

<style>
  .reader { display: flex; flex-direction: column; gap: 12px; flex: 1; }
  .bar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .title { display: flex; flex-direction: column; min-width: 0; }
  .title span { font-size: 12px; }
  .spacer { flex: 1; }
  .btn.on { border-color: var(--blue); background: var(--blue-soft); color: var(--blue); }
  .notice { margin: 0; padding: 10px 12px; border-radius: 8px; background: #241512; border: 1px solid #6b2a20; }
  .body { display: flex; flex-wrap: wrap; gap: 14px; align-items: flex-start; flex: 1; }
  .page { flex: 999 1 420px; min-width: 0; background: var(--panel2); border: 1px solid var(--line); border-radius: 12px; padding: 10px; overflow: auto; text-align: center; }
  canvas { max-width: none; background: #fff; border-radius: 4px; box-shadow: 0 2px 12px #0008; }
  canvas.night { filter: invert(1) hue-rotate(180deg); }
  .doc { text-align: left; max-width: 680px; margin: 0 auto; padding: 16px 12px; line-height: 1.7; }
  .doc h2 { margin: 18px 0 8px; }
  .doc p { margin: 0 0 12px; }
  .side { flex: 1 1 260px; max-width: 360px; display: flex; flex-direction: column; gap: 8px; }
  .hit { text-align: left; display: flex; flex-direction: column; gap: 2px; padding: 8px; border-radius: 8px; border: 1px solid var(--line); background: var(--panel2); font-size: 13px; }
  .hit:hover { border-color: var(--blue-line); }
  .mark, .note { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; justify-content: space-between; font-size: 13px; }
  .note { flex-direction: column; align-items: flex-start; padding: 8px; border: 1px solid var(--line); border-radius: 8px; background: var(--panel2); }
  .link { background: none; border: 0; color: var(--blue); padding: 0; text-decoration: underline; }
  .mini-btn { padding: 3px 10px; border-radius: 6px; border: 1px solid var(--line2); background: transparent; font-size: 12px; color: var(--muted); }
  .top { margin-top: 10px; }
  .col { display: flex; flex-direction: column; gap: 8px; margin-top: 8px; }
  .nav { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; }
  .range { flex: 1 1 160px; accent-color: var(--blue); }
  .pg { display: flex; align-items: center; gap: 6px; font-family: var(--mono); font-size: 12px; color: var(--muted); }
  .num { width: 72px; padding: 6px 8px; }
  .fine { font-size: 12px; margin: 0; }
  .grow { flex: 1 1 140px; }
</style>
