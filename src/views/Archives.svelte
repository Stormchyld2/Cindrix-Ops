<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    libraryStatus, libraryShare, driveRescan, crawlStart, crawlStatus, crawlCancel, archiveAdd, archiveCopyStatus, archiveRemove,
    type LibraryStatus, type CrawlStatus, type CopyStatus,
  } from '../lib/api';

  let { openIntel }: { openIntel: () => void } = $props();

  let st: LibraryStatus = $state({ archives: [], ready: false, url: null, shared: false, lan_url: null, drive: null, folder: '', online: false });
  const manyLibraries = $derived(new Set(st.archives.map((a) => a.library)).size > 1);
  let crawl: CrawlStatus = $state({ running: false, phase: '', pages: 0, files: 0, mb: 0, queued: 0, current: '', failed: 0, finished: false, ok: false, message: '', file: '', blocked: 0, hit_limit: false });
  let copy: CopyStatus = $state({ running: false, finished: false, ok: false, name: '', copied_mb: 0, total_mb: 0, message: '' });
  let timer: ReturnType<typeof setInterval> | undefined;
  let working = $state(false);
  let confirmShare = $state(false);
  let looking = $state(false);
  let removing: string | null = $state(null);
  let problem = $state('');

  // Save a website
  let siteUrl = $state('');
  let fastSave = $state(false);
  let siteName = $state('');
  let scope = $state('site300');
  let sizeLimit = $state(2000);
  const scopes: Record<string, { label: string; section: boolean; pages: number }> = {
    page: { label: 'Just this page and the pages it links to (about 25 pages)', section: true, pages: 25 },
    site300: { label: 'This site, up to 300 pages', section: false, pages: 300 },
    section: { label: 'Only the folder this address is in, up to 1,000 pages', section: true, pages: 1000 },
    site3000: { label: 'The whole site, up to 3,000 pages', section: false, pages: 3000 },
    big: { label: 'A very large site, up to 20,000 pages', section: false, pages: 20000 },
  };

  async function refresh() {
    st = await libraryStatus();
    crawl = await crawlStatus();
    copy = await archiveCopyStatus();
  }
  onMount(() => { refresh(); timer = setInterval(refresh, 2000); });
  onDestroy(() => { if (timer) clearInterval(timer); });

  async function setShare(on: boolean) {
    working = true;
    confirmShare = false;
    await libraryShare(on);
    // the library server restarts in the background; wait for it to come back
    for (let i = 0; i < 40; i++) {
      await new Promise((r) => setTimeout(r, 750));
      await refresh();
      if (st.ready && st.shared === on) break;
    }
    working = false;
  }

  async function lookAgain() {
    looking = true;
    await driveRescan();
    for (let i = 0; i < 6; i++) {
      await new Promise((r) => setTimeout(r, 1000));
      await refresh();
    }
    looking = false;
  }

  async function addArchive() {
    problem = '';
    try {
      await archiveAdd();
      await refresh();
    } catch (e) {
      problem = String(e);
    }
  }

  async function removeArchive(name: string) {
    problem = '';
    removing = null;
    try {
      await archiveRemove(name);
      await refresh();
    } catch (e) {
      problem = String(e);
    }
  }

  async function startSite() {
    problem = '';
    const sc = scopes[scope];
    try {
      await crawlStart(siteUrl.trim(), siteName.trim(), sc.section, sc.pages, sizeLimit, fastSave);
      await refresh();
    } catch (e) {
      problem = String(e);
    }
  }

  const size = (mb: number) => (mb >= 1024 ? (mb / 1024).toFixed(1) + ' GB' : mb + ' MB');
</script>

<section class="wrap">
  <div class="top">
    <div>
      <h1>Archives</h1>
      <p class="sub">The big offline library: Wikipedia, medical guides, books and websites you saved, stored on this drive as .zim files. Read them in Intel, no internet needed. New archives you copy into the library folder are picked up by themselves.</p>
    </div>
    <button class="btn primary" onclick={openIntel}>Read them in Intel →</button>
  </div>

  <div class="panel drive">
    <div class="head">
      <div>
        <div class="label">THE DRIVE</div>
        {#if st.drive}
          <strong>{st.drive.name}{st.drive.edition ? ' · ' + st.drive.edition[0].toUpperCase() + st.drive.edition.slice(1) + ' edition' : ''}</strong>
          <p class="muted fine path">{st.drive.path}</p>
        {:else}
          <strong>No Cindrix drive found</strong>
          <p class="muted fine path">Using the folder next to the program: {st.folder}</p>
        {/if}
      </div>
      <button class="btn" onclick={lookAgain} disabled={looking}>{looking ? 'Looking…' : 'Look for the drive'}</button>
    </div>
    <p class="muted fine">
      {#if st.drive}
        Your library, books and lists are saved on this drive. The program finds it by its marker file, whatever drive letter this computer gives it.
      {:else}
        Plug the drive in at any time. The program looks every few seconds and finds it whatever drive letter it gets, or press the button.
      {/if}
    </p>
  </div>

  {#if problem}<div class="warn">{problem}</div>{/if}

  <div class="panel">
    <div class="head">
      <div>
        <div class="label">YOUR ARCHIVES</div>
        <strong>{st.archives.length} on this drive</strong>
      </div>
      <button class="btn primary" onclick={addArchive} disabled={copy.running}>+ Add archive</button>
    </div>
    {#if copy.running}
      <p>Copying <b>{copy.name}</b>: {size(copy.copied_mb)} of {size(copy.total_mb)}. Large archives take a while. Keep the drive plugged in.</p>
      <progress max={copy.total_mb || 1} value={copy.copied_mb}></progress>
    {:else if copy.finished}
      <p class={copy.ok ? 'muted' : 'bad'}>{copy.ok ? `Added ${copy.name}.` : copy.message}</p>
    {/if}
    {#if st.archives.length === 0}
      <p class="muted">None found in <code>{st.folder}\library</code> (subfolders are searched too). Use <b>+ Add archive</b> to copy a .zim file from your computer (you can download them from the Kiwix library while you have internet), or save a website below. Only add archives whose license lets you keep a copy.</p>
    {:else}
      <ul class="list">
        {#each st.archives as a (a.library + '/' + a.file)}
          <li>
            <div class="grow">
              <strong>{a.file}</strong>
              <span class="muted"> · {size(a.mb)}{a.articles ? ' · ' + a.articles.toLocaleString() + ' items' : ''}</span>
              {#if manyLibraries && a.library}<span class="tagline">On {a.library}</span>{/if}
              {#if a.saved_site}<span class="tagline">Saved website · the assistant can read it</span>{/if}
              {#if a.note}<div class="warn small">{a.note}</div>{/if}
            </div>
            {#if removing === a.file}
              <span class="row">
                <button class="btn" onclick={() => (removing = null)}>Keep</button>
                <button class="btn danger" onclick={() => removeArchive(a.file)}>Remove from drive</button>
              </span>
            {:else if a.removable}
              <button class="btn" onclick={() => (removing = a.file)} disabled={copy.running || crawl.running}>Remove</button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <div class="panel">
    <div class="label">SAVE A WEBSITE</div>
    <strong>Turn a website into an archive on this drive</strong>
    <p class="muted">This is the one thing in the app that uses the internet, and only while it is running. It copies the pages and pictures, follows the site's links, and writes a .zim file you can read offline. Pages that need JavaScript to show anything (many modern web apps) will come out empty; wikis, blogs, manuals, news and documentation sites work well. The site's own rules apply, and the program obeys its robots.txt and goes slowly on purpose. Save only what you may keep a copy of.</p>
    {#if crawl.running}
      <div class="run">
        <p><b>{crawl.phase === 'writing' ? 'Writing the archive…' : 'Saving…'}</b> {crawl.pages} pages · {crawl.files} pictures and files · {crawl.mb.toFixed(1)} MB{crawl.phase === 'crawling' ? ' · ' + crawl.queued + ' waiting' : ''}{crawl.failed ? ' · ' + crawl.failed + ' could not be fetched' : ''}</p>
        <p class="muted fine path">{crawl.current}</p>
        <button class="btn" onclick={crawlCancel}>Stop and discard</button>
      </div>
    {:else}
      <div class="form">
        <label>Web address
          <input class="input" type="url" placeholder="https://www.ready.gov/" bind:value={siteUrl} />
        </label>
        <label><span>Name <span class="muted">(optional)</span></span>
          <input class="input" type="text" placeholder="Ready.gov" bind:value={siteName} />
        </label>
        <label>How much
          <select class="input" bind:value={scope}>
            {#each Object.entries(scopes) as [k, v]}<option value={k}>{v.label}</option>{/each}
          </select>
        </label>
        <label>Stop at
          <select class="input" bind:value={sizeLimit}>
            <option value={500}>500 MB</option>
            <option value={2000}>2 GB</option>
            <option value={10000}>10 GB</option>
            <option value={50000}>50 GB</option>
          </select>
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={fastSave} />
          <span><b>Fast</b>: fetch several pages at once. Several times quicker, but it puts more load on the website. Use it for sites you own or may copy freely. Sites that ask for a slower pace in their robots.txt still get one page at a time.</span>
        </label>
        <div><button class="btn primary" onclick={startSite} disabled={!siteUrl.trim() || copy.running}>Save this website</button></div>
      </div>
      {#if crawl.finished}
        {#if crawl.ok}
          <div class="done">
            <strong>Saved {crawl.file}</strong>: {crawl.pages} pages, {crawl.files} pictures and files, {crawl.mb.toFixed(1)} MB. The archive is in the list above, and the assistant can answer from it.
            {#if crawl.hit_limit}<div class="muted">It stopped at your limit. Run it again with a bigger limit to get more.</div>{/if}
            {#if crawl.blocked}<div class="muted">{crawl.blocked} pages were skipped because the site asks programs not to copy them.</div>{/if}
            {#if crawl.failed}<div class="muted">{crawl.failed} files could not be fetched and are missing from the copy.</div>{/if}
          </div>
        {:else}
          <div class="warn">{crawl.message}</div>
        {/if}
      {/if}
    {/if}
  </div>

  {#if st.ready}
    <div class="panel share">
      <div class="head">
        <div>
          <div class="label">SHARE WITH OTHER DEVICES</div>
          <strong>{st.shared ? 'Sharing is on' : 'Sharing is off'}</strong>
        </div>
        {#if st.shared}
          <button class="btn" onclick={() => setShare(false)} disabled={working}>Turn off</button>
        {:else}
          <button class="btn primary" onclick={() => (confirmShare = true)} disabled={working}>Turn on</button>
        {/if}
      </div>
      {#if st.shared}
        <p>Phones, tablets and laptops on the same Wi-Fi or hotspot can read this library. Open this address on the other device:</p>
        <p class="addr">{st.lan_url ?? 'Your computer’s address could not be found. Look in the Wi-Fi settings for its IP address and use it with port 8787.'}</p>
        <p class="muted fine">They can only read. They cannot change anything, and cannot see your inventory, family page or books. Anyone on this network can open the library while sharing is on.</p>
      {:else}
        <p class="muted">Off, the library can only be opened on this computer. Turn it on to use this drive as a small off-grid library server for the family: put it on a hotspot or router with no internet, and phones can browse it.</p>
      {/if}
      {#if working}<p class="muted fine">Restarting the library…</p>{/if}
    </div>
  {/if}

  {#if confirmShare}
    <dialog open aria-label="Share the library">
      <div class="dlg">
        <h2>Share the library on this network?</h2>
        <p>Other devices on the same Wi-Fi will be able to read the library while this is on. Windows may ask to allow the library program through the firewall: choose private networks only. Do not turn this on in public places.</p>
        <p class="muted">Everything else on this drive stays private and the assistant does not move onto the network.</p>
        <div class="row end">
          <button class="btn" onclick={() => (confirmShare = false)}>Cancel</button>
          <button class="btn primary" onclick={() => setShare(true)}>Share</button>
        </div>
      </div>
    </dialog>
  {/if}
</section>

<style>
  .wrap { display: flex; flex-direction: column; gap: 14px; flex: 1; }
  .top { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: flex-start; gap: 10px 16px; }
  .head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 10px; }
  .share p { margin: 8px 0 0; }
  .addr { font-family: var(--mono); font-size: 16px; color: var(--blue); word-break: break-all; }
  .fine { font-size: 12px; }
  code { font-family: var(--mono); }
  .dlg { padding: 18px; display: flex; flex-direction: column; gap: 10px; }
  .end { justify-content: flex-end; }
  .drive .path { word-break: break-all; font-family: var(--mono); margin: 4px 0 0; }
  .list { list-style: none; padding: 0; margin: 10px 0 0; display: flex; flex-direction: column; gap: 10px; }
  .list li { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 14px; padding: 10px 0; border-top: 1px solid var(--line); }
  .grow { flex: 1; min-width: 220px; }
  .tagline { display: inline-block; margin-left: 8px; font-size: 12px; color: var(--blue); }
  .small { font-size: 12px; margin-top: 6px; }
  .form { display: flex; flex-direction: column; gap: 10px; margin-top: 10px; max-width: 560px; }
  .form label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
  .run { margin-top: 10px; }
  .done { margin-top: 12px; padding: 10px 12px; border: 1px solid var(--line2); border-radius: 8px; background: var(--panel2); font-size: 14px; }
  progress { width: 100%; accent-color: var(--blue); }
  .bad { color: #f08a7a; }
  .btn.danger { border-color: #8a3b30; color: #f08a7a; }
  .check { display: flex; gap: 10px; align-items: flex-start; font-size: 14px; }
  .check input { margin-top: 3px; flex: none; }
</style>
