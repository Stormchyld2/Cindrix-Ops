<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    zimBooks, zimFrame, zimOrigin, zimSearchAll, zimBookmarksLoad, zimBookmarksSave, libraryStatus,
    type ZimBook, type ZimBookmark, type ZimHit, type LibraryStatus,
  } from '../lib/api';
  import { settings } from '../lib/settings.svelte';

  // Intel is the offline internet: a small browser for the archives (.zim files) on the drive.
  // Pages come from the archives through a private address, so nothing here touches the network.
  let { openArchives }: { openArchives: () => void } = $props();

  type Loc = { t: 'start' } | { t: 'results'; term: string } | { t: 'page'; path: string };
  type Tab = { id: number; history: Loc[]; idx: number };

  const MAX_TABS = 12;
  let books: ZimBook[] = $state([]);
  let st: LibraryStatus | null = $state(null);
  let tabs: Tab[] = $state([]);
  let active = $state(0);
  let nextId = 1;
  let origin = '';
  let frame: HTMLIFrameElement | undefined = $state();
  let src: string | undefined = $state();
  let srcdoc: string | undefined = $state();
  let pending: string | null = null;
  let notice = $state('');
  let zoom = $state(100);
  let bookmarks: ZimBookmark[] = $state([]);
  let showMarks = $state(false);
  let term = $state('');
  let suggestions: ZimHit[] = $state([]);
  let hits: ZimHit[] = $state([]);
  let searching = $state(false);
  let plain = $state(false);
  // set when one archive is opened in the library server (kiwix-serve) instead of the built-in reader
  let plainUrl: string | null = $state(null);
  let titles: Record<string, string> = $state({});
  let debounce: ReturnType<typeof setTimeout> | undefined;
  let poll: ReturnType<typeof setInterval> | undefined;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  const tab = $derived(tabs.find((t) => t.id === active));
  const loc = $derived(tab ? tab.history[tab.idx] : undefined);
  // Pages come from the library server (a proper web server, so every kind of archive works) when it is
  // running and the person has not asked for the built-in reader.
  const serverOn = $derived.by(() => settings.values.intelReader === 'server' && !!st?.url && st.url !== 'preview');
  const here = $derived(loc && loc.t === 'page' ? loc.path : '');
  const bookHere = $derived(bookOf(here));
  const marked = $derived(here !== '' && bookmarks.some((b) => b.path === here));
  const groups = $derived.by(() => {
    const m = new Map<string, { title: string; items: ZimHit[] }>();
    for (const h of hits) {
      const g = m.get(h.book) ?? { title: h.book_title, items: [] };
      g.items.push(h);
      m.set(h.book, g);
    }
    return Array.from(m.values());
  });

  function bookOf(path: string): string {
    const m = /^\/content\/([^/?]+)/.exec(path);
    if (!m) return '';
    try { return decodeURIComponent(m[1]); } catch { return m[1]; }
  }
  function bookTitle(name: string): string {
    return books.find((b) => b.name === name)?.title ?? name;
  }
  function titleOf(t: Tab): string {
    const l = t.history[t.idx];
    if (!l) return 'Intel';
    if (l.t === 'start') return 'Intel';
    if (l.t === 'results') return 'Search: ' + l.term;
    return titles[l.path] || bookTitle(bookOf(l.path)) || 'Page';
  }
  const size = (mb: number) => (mb >= 1024 ? (mb / 1024).toFixed(1) + ' GB' : mb + ' MB');
  const shown = (b: ZimBookmark) => { try { return decodeURIComponent(b.title || b.path); } catch { return b.title || b.path; } };
  const cur = (): Tab | undefined => tabs.find((t) => t.id === active);
  const homeOf = (name: string): Loc => ({ t: 'page', path: `/content/${encodeURIComponent(name)}` });

  function cleanLoc(x: unknown): Loc | null {
    if (!x || typeof x !== 'object') return null;
    const o = x as Record<string, unknown>;
    if (o.t === 'start') return { t: 'start' };
    if (o.t === 'results' && typeof o.term === 'string') return { t: 'results', term: o.term.slice(0, 200) };
    if (o.t === 'page' && typeof o.path === 'string' && o.path.startsWith('/')) return { t: 'page', path: o.path.slice(0, 2000) };
    return null;
  }

  onMount(async () => {
    origin = await zimOrigin();
    try { st = await libraryStatus(); } catch { /* the built-in reader is used */ }
    window.addEventListener('message', onMessage);
    let restored = false;
    try {
      const s = JSON.parse(await zimBookmarksLoad()) as { items?: ZimBookmark[]; zoom?: number; tabs?: { history?: unknown; idx?: unknown }[]; current?: number };
      if (Array.isArray(s.items)) bookmarks = s.items.filter((b) => b && typeof b.path === 'string' && typeof b.book === 'string').slice(0, 2000);
      if (typeof s.zoom === 'number') zoom = Math.min(200, Math.max(70, s.zoom));
      if (settings.values.intelRestoreTabs && Array.isArray(s.tabs)) {
        for (const raw of s.tabs.slice(0, MAX_TABS)) {
          const h = Array.isArray(raw?.history) ? (raw.history.map(cleanLoc).filter((l): l is Loc => l !== null) as Loc[]) : [];
          if (!h.length) continue;
          const idx = typeof raw.idx === 'number' ? Math.min(h.length - 1, Math.max(0, Math.round(raw.idx))) : h.length - 1;
          tabs.push({ id: nextId++, history: h, idx });
        }
        if (tabs.length) {
          const c = typeof s.current === 'number' ? Math.min(tabs.length - 1, Math.max(0, Math.round(s.current))) : 0;
          restored = true;
          await activate(tabs[c].id);
        }
      }
    } catch { /* start with none */ }
    if (!tabs.length) newTab();
    try { st = await libraryStatus(); } catch { /* the plain Kiwix link just stays hidden */ }
    await refreshBooks();
    if (!restored && settings.values.intelStart === 'first' && books.length) {
      const t = cur();
      if (t && t.history.length === 1 && t.history[0].t === 'start') await go(homeOf(books[0].name));
    }
    poll = setInterval(refreshBooks, 3000);
  });
  onDestroy(() => {
    window.removeEventListener('message', onMessage);
    clearTimeout(debounce);
    clearInterval(poll);
    if (saveTimer) { clearTimeout(saveTimer); save(); }
  });

  // new archives copied onto the drive show up by themselves
  async function refreshBooks() {
    try { books = await zimBooks(); } catch { /* keep what we have */ }
    // the library server may have started since this screen opened
    if (!st?.url) { try { st = await libraryStatus(); } catch { /* the link just stays hidden */ } }
  }
  /** Some archives (saved websites that need a web browser's own features) show blank in the built-in
   *  reader but work in the library server, which is a proper web server. Open this one there. */
  function openInServer() {
    if (!st?.url || !bookHere) return;
    plainUrl = `${st.url}content/${encodeURIComponent(bookHere)}`;
    plain = true;
  }

  function save() {
    const out: Record<string, unknown> = { version: 1, zoom, items: bookmarks };
    if (settings.values.intelRestoreTabs) {
      out.tabs = tabs.map((t) => {
        const h = t.history.slice(-30);
        return { history: h, idx: Math.max(0, t.idx - (t.history.length - h.length)) };
      });
      out.current = Math.max(0, tabs.findIndex((t) => t.id === active));
    }
    void zimBookmarksSave(JSON.stringify(out)).catch(() => {});
  }
  function scheduleSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => { saveTimer = undefined; save(); }, 800);
  }

  // ---------- tabs and navigation ----------
  function newTab() {
    if (tabs.length >= MAX_TABS) { notice = 'That is the most tabs at once. Close one first.'; return; }
    const t: Tab = { id: nextId++, history: [{ t: 'start' }], idx: 0 };
    tabs = [...tabs, t];
    void activate(t.id);
  }
  async function activate(id: number) {
    active = id;
    notice = '';
    suggestions = [];
    showMarks = false;
    plain = false;
    const t = cur();
    if (t) await show(t.history[t.idx]);
    scheduleSave();
  }
  function closeTab(id: number) {
    const i = tabs.findIndex((t) => t.id === id);
    if (i < 0) return;
    const wasActive = id === active;
    tabs = tabs.filter((t) => t.id !== id);
    if (!tabs.length) { newTab(); return; }
    if (wasActive) void activate(tabs[Math.min(i, tabs.length - 1)].id);
    else scheduleSave();
  }

  /** Put what a location holds on screen. */
  async function show(l: Loc) {
    if (l.t === 'page' && serverOn && st?.url) {
      // the page is in another window's hands from here: it cannot report back where it goes
      pending = null;
      src = st.url + l.path.replace(/^\//, '');
      srcdoc = undefined;
    } else if (l.t === 'page') {
      pending = l.path;
      const f = await zimFrame(l.path);
      // the frame is already at this address: no page-loaded message will come
      if (f.src === src && f.srcdoc === srcdoc) pending = null;
      src = f.src;
      srcdoc = f.srcdoc;
      if (f.title) titles[l.path] = f.title;
    } else {
      pending = null;
      src = undefined;
      srcdoc = undefined;
      if (l.t === 'results') await runSearch(l.term);
    }
  }
  async function go(l: Loc, push = true) {
    const t = cur();
    if (!t) return;
    notice = '';
    plain = false;
    if (push) {
      t.history = [...t.history.slice(0, t.idx + 1), l];
      t.idx = t.history.length - 1;
    }
    await show(l);
    scheduleSave();
  }
  async function back() {
    const t = cur();
    if (t && t.idx > 0) { t.idx--; plain = false; await show(t.history[t.idx]); scheduleSave(); }
  }
  async function forward() {
    const t = cur();
    if (t && t.idx < t.history.length - 1) { t.idx++; plain = false; await show(t.history[t.idx]); scheduleSave(); }
  }

  function toPath(url: string): string {
    return origin && url.startsWith(origin) ? url.slice(origin.length) : url;
  }
  function onMessage(e: MessageEvent) {
    if (!frame || e.source !== frame.contentWindow) return;
    const d = e.data as { cx?: string; url?: string; path?: string; title?: string } | null;
    if (!d || typeof d !== 'object') return;
    if (d.cx === 'external') { notice = 'That link goes to the internet, so it is not opened. Intel works offline.'; return; }
    if (d.cx === 'nav' && typeof d.path === 'string') { void go({ t: 'page', path: d.path }); return; }
    if (d.cx === 'page' && typeof d.url === 'string') {
      const p = toPath(d.url);
      if (!p.startsWith('/')) return;
      const t = cur();
      if (!t) return;
      const now = t.history[t.idx];
      if (pending !== null) {
        // the page this tab asked for has loaded (maybe after a redirect): remember where it really is
        t.history[t.idx] = { t: 'page', path: p };
        pending = null;
      } else if (!(now && now.t === 'page' && now.path === p)) {
        // a link inside the page was followed
        t.history = [...t.history.slice(0, t.idx + 1), { t: 'page', path: p }];
        t.idx = t.history.length - 1;
      }
      if (d.title) titles[p] = d.title;
      notice = '';
      sendZoom();
      scheduleSave();
    }
  }
  function sendZoom() { frame?.contentWindow?.postMessage({ cx: 'zoom', v: zoom / 100 }, '*'); }
  function setZoom(d: number) { zoom = Math.min(200, Math.max(70, zoom + d)); sendZoom(); scheduleSave(); }

  // ---------- search ----------
  function onType() {
    clearTimeout(debounce);
    if (term.trim().length < 2) { suggestions = []; return; }
    debounce = setTimeout(async () => {
      try { suggestions = (await zimSearchAll(term.trim())).slice(0, 10); } catch { suggestions = []; }
    }, 200);
  }
  async function runSearch(q: string) {
    searching = true;
    hits = [];
    try { hits = await zimSearchAll(q); } catch { hits = []; }
    searching = false;
  }
  function submit() {
    const q = term.trim();
    if (!q) return;
    suggestions = [];
    term = '';
    void go({ t: 'results', term: q });
  }
  function openHit(h: ZimHit) {
    suggestions = [];
    term = '';
    void go({ t: 'page', path: h.path });
  }
  /** Search the words inside the archive that is open (the whole text, when the library server is running). */
  function searchInside() {
    const q = term.trim();
    if (!q || !bookHere) return;
    suggestions = [];
    term = '';
    void go({ t: 'page', path: `/search?content=${encodeURIComponent(bookHere)}&pattern=${encodeURIComponent(q)}` });
  }

  // ---------- bookmarks ----------
  function toggleMark() {
    if (!here) return;
    if (marked) bookmarks = bookmarks.filter((b) => b.path !== here);
    else bookmarks = [...bookmarks, { book: bookHere, path: here, title: titles[here] || here.split('/').pop() || here }];
    scheduleSave();
  }
  function removeMark(b: ZimBookmark) { bookmarks = bookmarks.filter((x) => x !== b); scheduleSave(); }
</script>

<section class="net">
  <div class="tabstrip" role="tablist" aria-label="Open pages">
    {#each tabs as t (t.id)}
      <div class="tab" class:on={t.id === active}>
        <button role="tab" aria-selected={t.id === active} class="tabbtn" onclick={() => activate(t.id)}>{titleOf(t)}</button>
        <button class="x" aria-label={`Close ${titleOf(t)}`} onclick={() => closeTab(t.id)}>✕</button>
      </div>
    {/each}
    <button class="newtab" aria-label="New tab" onclick={newTab}>＋</button>
  </div>

  <div class="bar">
    <button class="btn" onclick={back} disabled={!tab || tab.idx <= 0} aria-label="Back">←</button>
    <button class="btn" onclick={forward} disabled={!tab || tab.idx >= tab.history.length - 1} aria-label="Forward">→</button>
    <button class="btn" onclick={() => go({ t: 'start' })} aria-label="Intel home">⌂</button>
    <div class="find">
      <input class="input" type="search" placeholder="Search every archive" aria-label="Search every archive" bind:value={term} oninput={onType}
        onkeydown={(e) => { if (e.key === 'Enter') submit(); if (e.key === 'Escape') suggestions = []; }} />
      {#if suggestions.length}
        <ul class="sug" role="listbox" aria-label="Suggestions">
          {#each suggestions as s}
            <li><button role="option" aria-selected="false" onclick={() => openHit(s)}>{s.title} <span class="from">{s.book_title}</span></button></li>
          {/each}
        </ul>
      {/if}
    </div>
    <button class="btn" onclick={submit} disabled={!term.trim()}>Search</button>
    {#if bookHere}
      <button class="btn" onclick={searchInside} disabled={!term.trim()} title="Search the words inside the archive that is open">Search inside {bookTitle(bookHere)}</button>
    {/if}
    <button class="btn" onclick={() => setZoom(-10)} aria-label="Smaller text">A−</button>
    <button class="btn" onclick={() => setZoom(10)} aria-label="Larger text">A＋</button>
    <button class="btn" class:on={marked} onclick={toggleMark} disabled={!here} aria-pressed={marked} aria-label={marked ? 'Remove bookmark' : 'Bookmark this page'}>{marked ? '★' : '☆'}</button>
    <button class="btn" onclick={() => (showMarks = !showMarks)} aria-expanded={showMarks}>Bookmarks{bookmarks.length ? ` (${bookmarks.length})` : ''}</button>
  </div>

  {#if notice}<div class="warn" role="status">{notice}</div>{/if}
  {#if showMarks}
    <div class="marks">
      {#if !bookmarks.length}<span class="muted">No bookmarks yet. Press ☆ on a page to keep it here.</span>{/if}
      <ul>
        {#each bookmarks as b}
          <li>
            <button class="open" onclick={() => { showMarks = false; void go({ t: 'page', path: b.path }); }}>{shown(b)} <span class="from">{bookTitle(b.book)}</span></button>
            <button class="x" aria-label={`Remove ${shown(b)}`} onclick={() => removeMark(b)}>✕</button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  <div class="view panel">
    {#if plain && st?.url}
      <iframe title="Plain Kiwix screen" src={plainUrl ?? st.url} sandbox="allow-scripts allow-same-origin allow-forms allow-popups"></iframe>
    {:else if loc?.t === 'page'}
      <iframe bind:this={frame} title="Archive page" {src} {srcdoc} style:zoom={serverOn ? zoom / 100 : undefined} sandbox="allow-scripts allow-same-origin allow-forms"></iframe>
    {:else if loc?.t === 'results'}
      <div class="scroll">
        <h2>Results for “{loc.term}”</h2>
        {#if searching}
          <p class="muted">Searching every archive…</p>
        {:else if !groups.length}
          <p class="muted">No titles match. Try fewer or shorter words. This searches page titles; open an archive and use “Search inside” to search the words in the pages themselves.</p>
        {:else}
          {#each groups as g}
            <div class="group">
              <div class="label">{g.title.toUpperCase()}</div>
              <ul class="hits">
                {#each g.items as h}<li><button class="open" onclick={() => openHit(h)}>{h.title}</button></li>{/each}
              </ul>
            </div>
          {/each}
        {/if}
      </div>
    {:else}
      <div class="scroll">
        <h1>Intel</h1>
        <p class="sub">The offline internet. Everything here is stored on this drive, so it works with no signal. Open an archive below, or search all of them at once.</p>
        {#if books.length === 0}
          <div class="empty">
            <strong>No archives on the drive yet.</strong>
            <p class="muted">Archives are .zim files (Wikipedia, medical guides, repair manuals, websites you saved). Copy them into the <code>library</code> folder on the drive and they appear here by themselves, or add one from the Archives page.</p>
            <button class="btn primary" onclick={openArchives}>Open Archives</button>
          </div>
        {:else}
          <div class="grid">
            {#each books as b (b.name)}
              <button class="card" onclick={() => go(homeOf(b.name))}>
                <strong>{b.title}</strong>
                {#if b.description}<span class="desc">{b.description}</span>{/if}
                <span class="meta">{b.articles.toLocaleString()} pages · {size(b.mb)}</span>
              </button>
            {/each}
          </div>
        {/if}
        {#if bookmarks.length}
          <h2 class="gap">Bookmarks</h2>
          <ul class="hits">
            {#each bookmarks.slice(0, 12) as b}
              <li><button class="open" onclick={() => go({ t: 'page', path: b.path })}>{shown(b)} <span class="from">{bookTitle(b.book)}</span></button></li>
            {/each}
          </ul>
        {/if}
        <div class="row foot">
          <button class="linkbtn" onclick={openArchives}>Add or remove archives, save a website, share the library</button>
        </div>
      </div>
    {/if}
  </div>
  <div class="row fine muted">
    {#if st?.url && st.url !== 'preview'}
      {#if !plain && bookHere && !serverOn}
        <span>Page blank or broken?</span>
        <button class="linkbtn" onclick={openInServer}>Open {bookTitle(bookHere)} in the library server</button>
        <span>·</span>
      {/if}
      <button class="linkbtn" onclick={() => { plainUrl = null; plain = !plain; }}>{plain ? 'Back to Intel' : 'Use the plain Kiwix screen instead'}</button>
    {/if}
  </div>
</section>

<style>
  .net { display: flex; flex-direction: column; gap: 8px; height: calc(100vh - 175px); min-height: 480px; }
  .tabstrip { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; }
  .tab { display: flex; align-items: center; border: 1px solid var(--line2); border-radius: 8px; background: var(--panel2); max-width: 220px; }
  .tab.on { border-color: var(--blue-line); background: var(--blue-soft); }
  .tabbtn { flex: 1; min-width: 0; padding: 6px 8px 6px 12px; background: none; border: 0; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
  .tab.on .tabbtn { color: var(--blue); font-weight: 600; }
  .x { background: none; border: 0; color: var(--muted); cursor: pointer; min-width: 32px; padding: 6px 8px; }
  .newtab { padding: 6px 12px; border-radius: 8px; border: 1px dashed var(--line2); background: transparent; color: var(--muted); }
  .bar { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .bar .btn { padding: 8px 12px; }
  .bar .btn.on { color: var(--accent); border-color: var(--accent-line); }
  .find { position: relative; flex: 1 1 220px; min-width: 160px; }
  .find .input { width: 100%; }
  .sug { position: absolute; z-index: 4; left: 0; right: 0; top: 100%; margin: 2px 0 0; padding: 4px; list-style: none; background: var(--panel); border: 1px solid var(--line2); border-radius: 8px; max-height: 320px; overflow: auto; }
  .sug button { width: 100%; text-align: left; padding: 8px 10px; background: none; border: 0; color: var(--text); font: inherit; cursor: pointer; border-radius: 6px; }
  .sug button:hover { background: var(--blue-soft); }
  .from { margin-left: 8px; font-size: 12px; color: var(--muted); }
  .marks { padding: 8px 10px; border: 1px solid var(--line2); border-radius: 8px; background: var(--panel2); }
  .marks ul { list-style: none; margin: 0; padding: 0; }
  .marks li { display: flex; gap: 4px; }
  .open { flex: 1; text-align: left; padding: 7px 4px; background: none; border: 0; color: var(--link); font: inherit; cursor: pointer; }
  .view { flex: 1; min-height: 0; padding: 0; display: flex; flex-direction: column; }
  iframe { width: 100%; height: 100%; border: 0; background: #fff; flex: 1; }
  .scroll { padding: 18px 20px; overflow: auto; flex: 1; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; margin-top: 16px; }
  .card { display: flex; flex-direction: column; gap: 6px; text-align: left; padding: 14px; border-radius: 10px; border: 1px solid var(--line2); background: var(--panel2); cursor: pointer; }
  .card:hover { border-color: var(--blue-line); }
  .card strong { color: var(--blue); }
  .desc { font-size: 13px; color: var(--text); }
  .meta { font-family: var(--mono); font-size: 11px; letter-spacing: .08em; color: var(--muted); text-transform: uppercase; }
  .empty { margin-top: 16px; display: flex; flex-direction: column; gap: 8px; align-items: flex-start; }
  .hits { list-style: none; margin: 6px 0 0; padding: 0; }
  .group { margin-top: 14px; }
  .gap { margin-top: 24px; }
  .foot { margin-top: 20px; }
  .fine { font-size: 12px; }
  .linkbtn { background: none; border: 0; color: var(--link); text-decoration: underline; padding: 0; font: inherit; cursor: pointer; }
  code { font-family: var(--mono); }
</style>
