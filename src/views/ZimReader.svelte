<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { zimBooks, zimFrame, zimOrigin, zimSuggest, zimBookmarksLoad, zimBookmarksSave, type ZimBook, type ZimBookmark, type ZimSuggestion } from '../lib/api';

  let { ready }: { ready: boolean } = $props();

  let books: ZimBook[] = $state([]);
  let book = $state('');
  let origin = '';
  let frame: HTMLIFrameElement | undefined = $state();
  let src: string | undefined = $state();
  let srcdoc: string | undefined = $state();

  // history of library paths, like a browser's
  let history: string[] = $state([]);
  let idx = $state(-1);
  let titles: Record<string, string> = $state({});
  let pending: string | null = null;
  let notice = $state('');
  let zoom = $state(100);
  let bookmarks: ZimBookmark[] = $state([]);
  let showMarks = $state(false);
  let term = $state('');
  let suggestions: ZimSuggestion[] = $state([]);
  let debounce: ReturnType<typeof setTimeout>;
  let loadedFor = '';

  const here = $derived(idx >= 0 ? history[idx] : '');
  const marked = $derived(bookmarks.some((b) => b.book === book && b.path === here));
  const myMarks = $derived(bookmarks.filter((b) => b.book === book));

  onMount(async () => {
    origin = await zimOrigin();
    window.addEventListener('message', onMessage);
    try {
      const s = JSON.parse(await zimBookmarksLoad()) as { items?: ZimBookmark[]; zoom?: number };
      if (Array.isArray(s.items)) bookmarks = s.items.filter((b) => b && typeof b.path === 'string' && typeof b.book === 'string').slice(0, 2000);
      if (typeof s.zoom === 'number') zoom = Math.min(200, Math.max(70, s.zoom));
    } catch { /* start with none */ }
  });
  onDestroy(() => { window.removeEventListener('message', onMessage); clearTimeout(debounce); });

  // pick up the list of books once the library server is ready
  $effect(() => {
    if (ready && loadedFor !== 'done') {
      loadedFor = 'done';
      void zimBooks().then((b) => {
        books = b;
        if (b.length && !b.some((x) => x.name === book)) { book = b[0].name; void go(`/content/${book}`, true); }
      });
    }
  });

  function saveMarks() { void zimBookmarksSave(JSON.stringify({ version: 1, zoom, items: bookmarks })).catch(() => {}); }

  async function load(path: string) {
    const f = await zimFrame(path);
    src = f.src; srcdoc = f.srcdoc;
    if (f.title) titles[path] = f.title;
  }
  async function go(path: string, push: boolean) {
    notice = '';
    if (push) { history = [...history.slice(0, idx + 1), path]; idx = history.length - 1; }
    pending = path;
    await load(path);
  }
  async function back() { if (idx > 0) { idx--; pending = history[idx]; await load(history[idx]); } }
  async function forward() { if (idx < history.length - 1) { idx++; pending = history[idx]; await load(history[idx]); } }
  function home() { if (book) void go(`/content/${book}`, true); }
  function switchBook(name: string) { book = name; history = []; idx = -1; home(); }

  function toPath(url: string): string {
    return origin && url.startsWith(origin) ? url.slice(origin.length) : url;
  }

  function onMessage(e: MessageEvent) {
    if (!frame || e.source !== frame.contentWindow) return;
    const d = e.data as { cx?: string; url?: string; path?: string; title?: string } | null;
    if (!d || typeof d !== 'object') return;
    if (d.cx === 'external') { notice = 'That link goes to the internet, so it is not opened. This library works offline.'; return; }
    if (d.cx === 'nav' && typeof d.path === 'string') { void go(d.path, true); return; }
    if (d.cx === 'page' && typeof d.url === 'string') {
      const p = toPath(d.url);
      if (!p.startsWith('/')) return;
      if (pending !== null) { history[idx] = p; pending = null; }
      else if (p !== history[idx]) { history = [...history.slice(0, idx + 1), p]; idx = history.length - 1; }
      if (d.title) titles[p] = d.title;
      notice = '';
      sendZoom();
    }
  }
  function sendZoom() { frame?.contentWindow?.postMessage({ cx: 'zoom', v: zoom / 100 }, '*'); }
  function setZoom(d: number) { zoom = Math.min(200, Math.max(70, zoom + d)); sendZoom(); saveMarks(); }

  // search
  function onType() {
    clearTimeout(debounce);
    if (term.trim().length < 2) { suggestions = []; return; }
    debounce = setTimeout(async () => { try { suggestions = await zimSuggest(book, term.trim()); } catch { suggestions = []; } }, 180);
  }
  function openSuggestion(s: ZimSuggestion) { suggestions = []; term = ''; void go(s.path, true); }
  function fullSearch() {
    const t = term.trim();
    if (!t) return;
    suggestions = [];
    void go(`/search?content=${encodeURIComponent(book)}&pattern=${encodeURIComponent(t)}`, true);
  }

  function toggleMark() {
    if (!here) return;
    if (marked) bookmarks = bookmarks.filter((b) => !(b.book === book && b.path === here));
    else bookmarks = [...bookmarks, { book, path: here, title: titles[here] || here.split('/').pop() || here }];
    saveMarks();
  }
  function removeMark(b: ZimBookmark) { bookmarks = bookmarks.filter((x) => x !== b); saveMarks(); }
  const shown = (b: ZimBookmark) => decodeURIComponent(b.title || b.path);
</script>

<div class="reader">
  <div class="bar">
    <button class="btn" onclick={back} disabled={idx <= 0} aria-label="Back">←</button>
    <button class="btn" onclick={forward} disabled={idx >= history.length - 1} aria-label="Forward">→</button>
    <button class="btn" onclick={home} disabled={!book} aria-label="Home page of this archive">⌂</button>
    {#if books.length > 1}
      <select class="input" aria-label="Archive" value={book} onchange={(e) => switchBook(e.currentTarget.value)}>
        {#each books as b}<option value={b.name}>{b.title}</option>{/each}
      </select>
    {/if}
    <div class="find">
      <input class="input" type="search" placeholder="Search this archive" aria-label="Search this archive" bind:value={term} oninput={onType}
        onkeydown={(e) => { if (e.key === 'Enter') fullSearch(); if (e.key === 'Escape') suggestions = []; }} />
      {#if suggestions.length}
        <ul class="sug" role="listbox" aria-label="Suggestions">
          {#each suggestions as s}<li><button role="option" aria-selected="false" onclick={() => openSuggestion(s)}>{s.title}</button></li>{/each}
        </ul>
      {/if}
    </div>
    <button class="btn" onclick={fullSearch} disabled={!term.trim()}>Search text</button>
    <button class="btn" onclick={() => setZoom(-10)} aria-label="Smaller text">A−</button>
    <button class="btn" onclick={() => setZoom(10)} aria-label="Larger text">A＋</button>
    <button class="btn" class:on={marked} onclick={toggleMark} disabled={!here} aria-pressed={marked} aria-label={marked ? 'Remove bookmark' : 'Bookmark this page'}>{marked ? '★' : '☆'}</button>
    <button class="btn" onclick={() => (showMarks = !showMarks)} aria-expanded={showMarks}>Bookmarks{myMarks.length ? ` (${myMarks.length})` : ''}</button>
  </div>
  {#if notice}<div class="warn" role="status">{notice}</div>{/if}
  {#if showMarks}
    <div class="marks">
      {#if !myMarks.length}<span class="muted">No bookmarks in this archive yet. Press ☆ on a page to keep it here.</span>{/if}
      <ul>
        {#each myMarks as b}
          <li><button class="open" onclick={() => { showMarks = false; void go(b.path, true); }}>{shown(b)}</button><button class="x" aria-label={`Remove ${shown(b)}`} onclick={() => removeMark(b)}>✕</button></li>
        {/each}
      </ul>
    </div>
  {/if}
  <div class="frame">
    {#if src || srcdoc}
      <iframe bind:this={frame} title="Archive page" {src} {srcdoc} sandbox="allow-scripts allow-same-origin allow-forms"></iframe>
    {:else}
      <p class="muted pad">{books.length ? 'Opening…' : 'Looking for archives…'}</p>
    {/if}
  </div>
</div>

<style>
  .reader { display: flex; flex-direction: column; gap: 8px; height: 100%; min-height: 0; }
  .bar { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; padding: 8px 10px 0; }
  .bar .btn { padding: 8px 12px; }
  .bar .btn.on { color: var(--accent); border-color: var(--accent-line); }
  .find { position: relative; flex: 1 1 200px; min-width: 160px; }
  .find .input { width: 100%; box-sizing: border-box; }
  .sug { position: absolute; z-index: 4; left: 0; right: 0; top: 100%; margin: 2px 0 0; padding: 4px; list-style: none; background: var(--panel); border: 1px solid var(--line2); border-radius: 8px; max-height: 280px; overflow: auto; }
  .sug button { width: 100%; text-align: left; padding: 8px 10px; background: none; border: 0; color: var(--text); font: inherit; cursor: pointer; border-radius: 6px; }
  .sug button:hover { background: var(--blue-soft); }
  .warn { margin: 0 10px; }
  .marks { margin: 0 10px; padding: 8px 10px; border: 1px solid var(--line2); border-radius: 8px; background: var(--panel2); }
  .marks ul { list-style: none; margin: 0; padding: 0; }
  .marks li { display: flex; gap: 4px; }
  .marks .open { flex: 1; text-align: left; padding: 7px 4px; background: none; border: 0; color: var(--link); font: inherit; cursor: pointer; }
  .marks .x { background: none; border: 0; color: var(--muted); cursor: pointer; min-width: 36px; }
  .frame { flex: 1; min-height: 0; }
  iframe { width: 100%; height: 100%; border: 0; background: #fff; }
  .pad { padding: 16px; }
</style>
