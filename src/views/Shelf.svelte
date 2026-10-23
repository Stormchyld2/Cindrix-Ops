<script lang="ts">
  import { onMount } from 'svelte';
  import { shelfList, shelfAdd, shelfUpdate, shelfRemove, shelfScanDrive, type Book } from '../lib/api';
  import Reader from './Reader.svelte';
  import { sourceRequest } from '../lib/chat.svelte';
  import { settings } from '../lib/settings.svelte';

  // The Reading Corner (Morale) is the same shelf for books to enjoy; the Shelf (Intel) keeps the manuals. A book is in one or the other.
  let { corner = false }: { corner?: boolean } = $props();
  const LEISURE = 'Reading Corner';
  // "Reading Corner" or "Reading Corner / Mystery": the part after the slash is the genre, taken from the folder name
  const isLeisure = (c: string) => { const t = c.trim().toLowerCase(), l = LEISURE.toLowerCase(); return t === l || t.startsWith(l + ' / '); };
  const genreOf = (c: string) => { const i = c.indexOf('/'); return i >= 0 ? c.slice(i + 1).trim() : ''; };
  /** The word on a filter button: the genre in the Reading Corner, the category on the Shelf. */
  const labelOf = (b: Book) => (corner ? genreOf(b.category) || 'No genre' : b.category);

  let books: Book[] = $state([]);
  let everyBook: Book[] = $state([]);
  let filter = $state('All');
  let reading: Book | null = $state(null);
  let message = $state('');
  let busy = $state(false);
  let dragging = $state(false);
  let editing: Book | null = $state(null);
  let editTitle = $state('');
  let editAuthor = $state('');
  let editCategory = $state('');
  let removing: Book | null = $state(null);
  let fileInput: HTMLInputElement | undefined = $state();

  const covers = ['#3a4a33', '#5a2a24', '#5a4a1f', '#1f4a5a', '#3a3a44', '#41335a', '#44402a', '#24465a'];
  function color(title: string): string {
    let h = 0;
    for (const c of title) h = (h * 31 + c.charCodeAt(0)) >>> 0;
    return covers[h % covers.length];
  }

  async function refresh() {
    try { everyBook = await shelfList(); books = everyBook.filter((b) => isLeisure(b.category) === corner); } catch (e) { message = String(e); }
  }
  let scanning = $state(false);
  /** Put the books found on the drive onto the shelf. `quiet`: say something only when books were added. */
  async function scan(quiet: boolean) {
    if (scanning) return;
    scanning = true;
    try {
      const r = await shelfScanDrive();
      await refresh();
      const parts: string[] = [];
      if (r.added.length) parts.push(`Added ${r.added.length} book${r.added.length === 1 ? '' : 's'} from the drive: ${r.added.join(', ')}.`);
      else if (!quiet) parts.push(r.known > 0 ? `No new books on the drive (${r.known} already on the shelf).` : `No books found on the drive. Put PDF, Markdown or text files in the library, books or manuals folder (looked in ${r.folders.join(', ')}).`);
      if (r.problems.length) parts.push(...r.problems.slice(0, 3));
      if (parts.length) message = parts.join(' ');
    } catch (e) {
      if (!quiet) message = String(e);
    }
    scanning = false;
  }
  onMount(async () => {
    await refresh();
    const want = sourceRequest.book;
    if (want) {
      sourceRequest.book = null;
      // a source link opens the book even when it sits on the other shelf
      const b = everyBook.find((x) => x.id === want.id);
      if (b) reading = { ...b, last_page: want.page };
      else message = 'That book is no longer on the shelf.';
    }
    if (settings.values.autoScanShelf) await scan(true);
  });

  let query = $state('');
  let sortBy = $state<'title' | 'author' | 'newest'>('title');
  let categories = $derived(['All', ...Array.from(new Set(books.map(labelOf))).sort()]);
  let shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const found = books.filter((b) => (filter === 'All' || labelOf(b) === filter) && (!q || `${b.title} ${b.author} ${b.category}`.toLowerCase().includes(q)));
    const by = (a: Book, b: Book) =>
      sortBy === 'author' ? (a.author || '~').localeCompare(b.author || '~') || a.title.localeCompare(b.title)
      : sortBy === 'newest' ? b.added.localeCompare(a.added) || a.title.localeCompare(b.title)
      : a.title.localeCompare(b.title);
    return [...found].sort(by);
  });
  let resume = $derived(books.filter((b) => b.last_page > 1).sort((a, b) => b.last_page - a.last_page)[0] ?? null);

  async function addFiles(files: FileList | File[] | null) {
    if (!files || busy) return;
    busy = true;
    message = '';
    const done: string[] = [];
    const failed: string[] = [];
    for (const f of Array.from(files)) {
      try { const b = await shelfAdd(f, corner ? LEISURE : ''); done.push(b.title); }
      catch (e) { failed.push(`${f.name}: ${String(e)}`); }
    }
    await refresh();
    message = [done.length ? `Added: ${done.join(', ')}.` : '', ...failed].filter(Boolean).join(' ');
    busy = false;
    if (fileInput) fileInput.value = '';
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    dragging = false;
    addFiles(e.dataTransfer?.files ?? null);
  }

  function startEdit(b: Book) { editing = b; editTitle = b.title; editAuthor = b.author; editCategory = b.category; }
  async function saveEdit() {
    if (!editing) return;
    try { await shelfUpdate(editing.id, editTitle, editAuthor, editCategory); } catch (e) { message = String(e); }
    editing = null;
    await refresh();
  }
  async function confirmRemove() {
    if (!removing) return;
    try { await shelfRemove(removing.id); } catch (e) { message = String(e); }
    removing = null;
    await refresh();
  }
  async function moveBook(b: Book) {
    try { await shelfUpdate(b.id, b.title, b.author, corner ? 'Moved from the Reading Corner' : LEISURE); message = corner ? `“${b.title}” is back on the Shelf.` : `“${b.title}” is now in the Reading Corner.`; } catch (e) { message = String(e); }
    await refresh();
  }
  async function closeReader() { reading = null; await refresh(); }
</script>

{#if reading}
  <Reader book={reading} onBack={closeReader} />
{:else}
  <section
    class="shelf"
    class:drag={dragging}
    role="region"
    aria-label="Bookshelf"
    ondragover={(e) => { e.preventDefault(); dragging = true; }}
    ondragleave={() => (dragging = false)}
    ondrop={onDrop}
  >
    <div class="top">
      <div>
        <h1>{corner ? 'Reading Corner' : 'Shelf'}</h1>
        <p class="sub">{corner ? 'Books to enjoy, kept on this drive: stories, adventures and something to read when you need a break. Open one to read it.' : 'Real books and manuals, kept on this drive. Open one to read it, search inside it, or ask the assistant about it.'}</p>
      </div>
      <div class="row">
        <input bind:this={fileInput} id="add-book" class="sr" type="file" accept=".pdf,.md,.txt,application/pdf" multiple onchange={(e) => addFiles(e.currentTarget.files)} />
        <button class="btn" onclick={() => scan(false)} disabled={scanning} aria-label="Look for books on the drive">{scanning ? 'Looking…' : 'Look for books on the drive'}</button>
        <label class="btn primary add" for="add-book" aria-label="Add a book from a file">+ Add a book</label>
      </div>
    </div>

    {#if message}<p class="notice" role="status">{message}</p>{/if}

    {#if resume}
      <div class="resume panel">
        <div class="swatch" style="background: linear-gradient(160deg, {color(resume.title)}, #161918)"></div>
        <div class="grow">
          <div class="label">CONTINUE READING</div>
          <strong>{resume.title}</strong>
          <div class="muted">Page {resume.last_page}</div>
        </div>
        <button class="btn primary" onclick={() => (reading = resume)}>Resume</button>
      </div>
    {/if}

    <div class="pills">
      {#each categories as c}
        <button class="pill" class:on={filter === c} onclick={() => (filter = c)}>{c}</button>
      {/each}
    </div>
    <div class="row finder">
      <input class="input" type="search" placeholder="Search by title or author" aria-label="Search the books" bind:value={query} />
      <label class="sortpick">Sort by
        <select class="input" bind:value={sortBy} aria-label="Sort the books">
          <option value="title">Title, A to Z</option>
          <option value="author">Author, A to Z</option>
          <option value="newest">Newest first</option>
        </select>
      </label>
    </div>

    {#if shown.length === 0}
      <div class="empty panel">
        <strong>{corner ? 'Your Reading Corner is empty.' : 'Your shelf is empty.'}</strong>
        {#if corner}<p class="muted">Drag a book here, or use “Add a book”. PDF, Markdown and text files in a folder called <b>leisure</b> on the drive are put here automatically. A folder inside it, like <b>leisure\Mystery</b>, becomes a genre button; press “Look for books on the drive” after adding some. You can also move a book here from the Shelf with “Move to Reading Corner”.</p>
        {:else}<p class="muted">Drag a PDF here, or use “Add a book”. PDF, Markdown and text files in the drive's library, books or manuals folder are put on the shelf automatically; press “Look for books on the drive” after adding some. Books you add stay on this drive and never leave it. Books for enjoying go in a folder called <b>leisure</b> and show in the Reading Corner instead.</p>{/if}
      </div>
    {/if}

    <div class="grid">
      {#each shown as b (b.id)}
        <div class="book">
          <button class="cover" style="background: linear-gradient(160deg, {color(b.title)}, #161918)" onclick={() => (reading = b)} aria-label="Open {b.title}">
            <span class="spine"></span>
            <span class="t">{b.title}</span>
            <span class="k">{b.kind.toUpperCase()}{b.author ? ' · ' + b.author : ''}</span>
          </button>
          <div class="meta">{b.license}</div>
          {#if b.mine}
            <div class="row small">
              <button class="mini-btn" onclick={() => moveBook(b)}>{corner ? 'Move to the Shelf' : 'Move to Reading Corner'}</button>
              <button class="mini-btn" onclick={() => startEdit(b)}>Edit</button>
              <button class="mini-btn" onclick={() => (removing = b)}>Remove</button>
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <p class="muted fine">Add only books you have the right to use. A downloaded copy of a book stays a private copy on this drive. Added books are searchable by the assistant after you open them once.</p>
  </section>

  {#if editing}
    <dialog open aria-label="Edit book">
      <form class="dlg" onsubmit={(e) => { e.preventDefault(); saveEdit(); }}>
        <h2>Edit book</h2>
        <label class="f">Title<input class="input" bind:value={editTitle} /></label>
        <label class="f">Author<input class="input" bind:value={editAuthor} /></label>
        <label class="f">Shelf category<input class="input" bind:value={editCategory} placeholder="Mine, Medical, Repair…" /></label>
        <div class="row end">
          <button type="button" class="btn" onclick={() => (editing = null)}>Cancel</button>
          <button type="submit" class="btn primary">Save</button>
        </div>
      </form>
    </dialog>
  {/if}

  {#if removing}
    <dialog open aria-label="Remove book">
      <div class="dlg">
        <h2>Remove this book?</h2>
        <p>“{removing.title}” will be taken off your shelf and its file deleted from this drive, along with your bookmarks and notes for it.</p>
        <div class="row end">
          <button class="btn" onclick={() => (removing = null)}>Cancel</button>
          <button class="btn danger" onclick={confirmRemove}>Remove</button>
        </div>
      </div>
    </dialog>
  {/if}
{/if}

<style>
  .shelf { display: flex; flex-direction: column; gap: 16px; border-radius: 12px; }
  .shelf.drag { outline: 2px dashed var(--blue); outline-offset: 6px; }
  .top { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: flex-end; gap: 12px; }
  .add { cursor: pointer; }
  .notice { margin: 0; padding: 10px 12px; border-radius: 8px; background: var(--blue-soft); border: 1px solid var(--blue-line); }
  .resume { display: flex; flex-wrap: wrap; align-items: center; gap: 14px; }
  .swatch { width: 54px; height: 72px; border-radius: 6px; border: 1px solid var(--line2); }
  .grow { flex: 1 1 200px; }
  .finder { gap: 12px; align-items: center; flex-wrap: wrap; margin: 10px 0 4px; }
  .finder .input { min-width: 240px; }
  .sortpick { display: inline-flex; gap: 8px; align-items: center; color: var(--muted); font-size: 13px; }
  .sortpick .input { min-width: 160px; }
  .pills { display: flex; flex-wrap: wrap; gap: 8px; }
  .pill { padding: 7px 14px; border-radius: 999px; border: 1px solid var(--line2); background: transparent; color: #b5ada0; font-family: var(--mono); font-size: 12px; text-transform: uppercase; letter-spacing: .1em; }
  .pill.on { border-color: var(--blue); background: var(--blue-soft); color: var(--blue); }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 18px; }
  .book { display: flex; flex-direction: column; gap: 8px; }
  .cover { position: relative; aspect-ratio: 3 / 4; border-radius: 10px; border: 1px solid var(--line2); padding: 14px 12px; display: flex; flex-direction: column; justify-content: space-between; text-align: left; color: var(--text); overflow: hidden; }
  .cover:hover { border-color: var(--blue); }
  .spine { position: absolute; left: 0; top: 0; bottom: 0; width: 8px; background: #00000055; }
  .t { font-family: var(--display); font-weight: 700; font-size: 18px; line-height: 1.05; letter-spacing: .04em; text-transform: uppercase; padding-left: 6px; overflow-wrap: break-word; hyphens: auto; min-width: 0; }
  .k { font-family: var(--mono); font-size: 10px; letter-spacing: .1em; color: #ece7dea0; padding-left: 6px; text-transform: uppercase; }
  .meta { font-size: 12px; color: var(--muted); }
  .small { gap: 6px; }
  .mini-btn { padding: 3px 10px; border-radius: 6px; border: 1px solid var(--line2); background: transparent; font-size: 12px; color: var(--muted); }
  .mini-btn:hover { color: var(--blue); border-color: var(--blue-line); }
  .empty { text-align: center; }
  .fine { font-size: 12px; margin: 0; }
  .dlg { padding: 18px; display: flex; flex-direction: column; gap: 12px; }
  .f { display: flex; flex-direction: column; gap: 4px; font-size: 13px; color: var(--muted); }
  .end { justify-content: flex-end; }
</style>
