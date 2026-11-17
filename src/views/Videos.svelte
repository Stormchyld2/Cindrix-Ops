<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { videosList, videosProgressLoad, videosProgressSave, videoSource, type VideoList, type VideoTopic, type VideoItem, type VideoProgress } from '../lib/api';

  let list = $state<VideoList | null>(null);
  let error = $state('');
  let busy = $state(false);
  let query = $state('');
  let openTopic: string | null = $state(null);
  let progress = $state<Record<string, VideoProgress>>({});
  let thumbs = $state<Record<string, string>>({});

  let current = $state<VideoItem | null>(null);
  let src = $state('');
  let el: HTMLVideoElement | undefined = $state();
  let playError = $state('');
  let lastSaved = 0;
  let searchTimer: ReturnType<typeof setTimeout> | undefined;

  async function load() {
    busy = true;
    error = '';
    try {
      list = await videosList(query);
      const saved = await videosProgressLoad();
      progress = Object.fromEntries(saved.map((p) => [p.rel, p]));
      void loadThumbs();
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }
  async function loadThumbs() {
    if (!list) return;
    for (const t of list.topics) {
      for (const v of t.videos) {
        if (v.thumb && !thumbs[v.thumb]) {
          try { thumbs[v.thumb] = await videoSource(v.thumb); } catch { /* no picture: the card still works */ }
        }
      }
    }
  }
  onMount(() => { void load(); });
  onDestroy(() => { saveNow(false); clearTimeout(searchTimer); });

  function typed() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => { void load(); }, 250);
  }

  // a search shows every match at once; otherwise one topic at a time
  const searching = $derived(query.trim() !== '');
  const shownTopics: VideoTopic[] = $derived(
    !list ? [] : searching ? list.topics : list.topics.filter((t) => t.folder === openTopic),
  );
  const openTopicName = $derived(list?.topics.find((t) => t.folder === openTopic)?.name ?? '');

  function groupsOf(t: VideoTopic): { name: string; videos: VideoItem[] }[] {
    const out: { name: string; videos: VideoItem[] }[] = [];
    for (const v of t.videos) {
      let g = out.find((x) => x.name === v.group);
      if (!g) { g = { name: v.group, videos: [] }; out.push(g); }
      g.videos.push(v);
    }
    return out;
  }

  async function play(v: VideoItem) {
    if (!v.playable) return;
    saveNow(false);
    playError = '';
    current = v;
    src = await videoSource(v.rel);
  }
  function loaded() {
    // pick up where it was left off, unless it was finished (then start again)
    const p = current ? progress[current.rel] : undefined;
    if (el && p && !p.done && p.seconds > 5 && p.seconds < (el.duration || 0) - 10) el.currentTime = p.seconds;
    void el?.play().catch(() => {});
  }
  function saveNow(finished: boolean) {
    if (!el || !current) return;
    const secs = Math.max(0, Math.floor(el.currentTime));
    const len = Math.floor(el.duration || 0);
    const done = finished || (len > 0 && secs + 30 >= len);
    progress[current.rel] = { rel: current.rel, seconds: done ? 0 : secs, done };
    lastSaved = Date.now();
    void videosProgressSave(current.rel, done ? 0 : secs, done).catch(() => {});
  }
  function tick() {
    if (Date.now() - lastSaved > 5000) saveNow(false);
  }
  function markDone(v: VideoItem) {
    const done = !progress[v.rel]?.done;
    progress[v.rel] = { rel: v.rel, seconds: 0, done };
    void videosProgressSave(v.rel, 0, done).catch(() => {});
  }
  function stoppedAt(v: VideoItem): string {
    const p = progress[v.rel];
    if (!p) return '';
    if (p.done) return 'Watched';
    return p.seconds > 5 ? `Stopped at ${clock(p.seconds)}` : '';
  }
  function clock(s: number): string {
    const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), r = Math.floor(s % 60);
    return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${String(r).padStart(2, '0')}` : `${m}:${String(r).padStart(2, '0')}`;
  }
  function size(n: number): string {
    return n >= 1_000_000_000 ? `${(n / 1_000_000_000).toFixed(1)} GB` : n >= 1_000_000 ? `${Math.round(n / 1_000_000)} MB` : `${Math.max(1, Math.round(n / 1000))} KB`;
  }
  function unwatched(t: VideoTopic): number {
    return t.videos.filter((v) => !progress[v.rel]?.done).length;
  }
</script>

<section class="wrap">
  <h1>Videos</h1>
  {#if error}<p class="err" role="alert">{error}</p>{/if}

  {#if current}
    <div class="player">
      <div class="meta">
        <div class="t">{current.title}</div>
        <div class="muted small">{current.group ? current.group + ' · ' : ''}{playError}</div>
      </div>
      <!-- svelte-ignore a11y_media_has_caption -->
      <video bind:this={el} {src} controls preload="metadata" onloadedmetadata={loaded} ontimeupdate={tick} onpause={() => saveNow(false)} onended={() => saveNow(true)}
        onerror={() => (playError = 'This video could not be played. It may be damaged or not an MP4 file.')}></video>
      <div><button class="btn" onclick={() => { saveNow(false); current = null; src = ''; }}>Close the video</button></div>
    </div>
  {/if}

  {#if list && !list.folder_found}
    <div class="empty">
      <h2>No videos folder yet</h2>
      <p class="muted">Make a folder named <code>videos</code> inside the <code>library</code> folder of a drive. Put one folder in it for each topic (for example <code>Bushcraft</code>), and the MP4 videos inside that topic's folder. A picture with the same name as a video becomes its thumbnail.</p>
      <button class="btn" onclick={load}>Look again</button>
    </div>
  {:else if list}
    <div class="bar">
      <input class="search" type="search" placeholder="Search videos, for example deer or fire" aria-label="Search videos" bind:value={query} oninput={typed} />
      <button class="btn" onclick={load} disabled={busy}>Look again</button>
    </div>

    {#if list.topics.length === 0}
      <div class="empty">
        <h2>{searching ? 'Nothing matches that' : 'No videos found'}</h2>
        <p class="muted">{searching ? 'Try fewer or different words.' : 'The videos folder is there but holds no video files yet. Put MP4 files inside a topic folder, then look again.'}</p>
      </div>
    {:else if !searching && openTopic === null}
      <p class="muted small">{list.topics.length} {list.topics.length === 1 ? 'topic' : 'topics'}</p>
      <ul class="topics">
        {#each list.topics as t (t.folder)}
          <li>
            <button class="topic" onclick={() => (openTopic = t.folder)}>
              <span class="name">{t.name}</span>
              <span class="muted small">{t.videos.length} {t.videos.length === 1 ? 'video' : 'videos'}{unwatched(t) < t.videos.length ? ` · ${unwatched(t)} not watched` : ''}</span>
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      {#if !searching}
        <div class="bar"><button class="btn" onclick={() => (openTopic = null)}>← All topics</button></div>
        <h2>{openTopicName}</h2>
      {/if}
      {#each shownTopics as t (t.folder)}
        {#if searching}<h2>{t.name}</h2>{/if}
        {#each groupsOf(t) as g (g.name)}
          {#if g.name}<h3>{g.name}</h3>{/if}
          <ul class="vids">
            {#each g.videos as v (v.rel)}
              <li class:now={current?.rel === v.rel} class:off={!v.playable}>
                <button class="main" onclick={() => play(v)} disabled={!v.playable} aria-label={`Play ${v.title}`}>
                  {#if v.thumb && thumbs[v.thumb]}<img src={thumbs[v.thumb]} alt="" loading="lazy" />{:else}<span class="ph" aria-hidden="true">▶</span>{/if}
                  <span class="txt">
                    <span class="t">{v.title}</span>
                    <span class="muted small">{size(v.size)}{stoppedAt(v) ? ` · ${stoppedAt(v)}` : ''}{v.playable ? '' : ` · ${v.ext.toUpperCase()} file: save it as an MP4 to watch it here`}</span>
                  </span>
                </button>
                {#if v.playable}
                  <button class="icon" title={progress[v.rel]?.done ? 'Mark as not watched' : 'Mark as watched'} aria-label={progress[v.rel]?.done ? 'Mark as not watched' : 'Mark as watched'} onclick={() => markDone(v)}>{progress[v.rel]?.done ? '✔' : '○'}</button>
                {/if}
              </li>
            {/each}
          </ul>
        {/each}
      {/each}
    {/if}
  {:else if busy}
    <p class="muted">Looking through the videos folder…</p>
  {/if}
</section>

<style>
  .wrap { display: flex; flex-direction: column; gap: 14px; }
  h1 { margin: 0; font-size: 22px; }
  h2 { margin: 0; font-size: 18px; }
  h3 { margin: 6px 0 0; font-size: 15px; color: var(--muted); }
  .err { color: var(--bad, #e66); }
  .small { font-size: 13px; }
  code { font-family: var(--mono); font-size: 12px; background: var(--panel2); padding: 1px 5px; border-radius: 4px; }
  ul { list-style: none; margin: 0; padding: 0; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .search { flex: 1; min-width: 200px; padding: 9px 12px; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; color: var(--text); font: inherit; }
  .player { display: flex; flex-direction: column; gap: 8px; padding: 12px; background: var(--panel2); border: 1px solid var(--blue-line); border-radius: 10px; }
  .player .t { font-weight: 600; }
  video { width: 100%; max-height: 70vh; background: #000; border-radius: 8px; }
  .topics { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 8px; }
  .topic { width: 100%; display: flex; flex-direction: column; gap: 2px; text-align: left; padding: 12px; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; color: var(--text); font: inherit; cursor: pointer; }
  .topic:hover { border-color: var(--blue-line); }
  .topic .name { font-weight: 600; }
  .vids li { display: flex; align-items: center; gap: 4px; border-bottom: 1px solid var(--line); }
  .vids li.now { background: var(--blue-soft); }
  .vids li.off { opacity: 0.6; }
  .vids .main { flex: 1; min-width: 0; display: flex; align-items: center; gap: 10px; text-align: left; padding: 8px 6px; background: none; border: 0; color: var(--text); font: inherit; cursor: pointer; }
  .vids .main:disabled { cursor: default; }
  .vids img, .vids .ph { width: 96px; height: 54px; flex: none; border-radius: 6px; object-fit: cover; background: var(--panel2); display: flex; align-items: center; justify-content: center; color: var(--muted); }
  .txt { display: flex; flex-direction: column; min-width: 0; }
  .txt .t { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .icon { min-width: 36px; min-height: 36px; background: none; border: 1px solid transparent; border-radius: 8px; color: var(--muted); font-size: 15px; cursor: pointer; }
  .icon:hover { border-color: var(--blue-line); color: var(--blue); }
  .empty { padding: 16px 0; display: flex; flex-direction: column; gap: 10px; align-items: flex-start; }
  .empty h2 { font-size: 20px; }
</style>
