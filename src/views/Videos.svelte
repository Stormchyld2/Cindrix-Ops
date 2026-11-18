<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { inShare, videosList, videoSource, videoConvertStart, videoConvertStatus, videoConvertStop, videosConvertedClear, type ConvertStatus, type VideoList, type VideoTopic, type VideoItem } from '../lib/api';
  import { videoPlayer as vp } from '../lib/videostate.svelte';

  let list = $state<VideoList | null>(null);
  let error = $state('');
  let busy = $state(false);
  let query = $state('');
  let openTopic: string | null = $state(null);
  let thumbs = $state<Record<string, string>>({});

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let job = $state<ConvertStatus | null>(null);
  let jobTimer: ReturnType<typeof setInterval> | undefined;
  let note = $state('');

  async function load() {
    busy = true;
    error = '';
    try {
      list = await videosList(query);
      void vp.init();
      void loadThumbs();
      if (!inShare) void watchJob();
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
  onDestroy(() => { clearTimeout(searchTimer); clearInterval(jobTimer); });

  // making a playable copy can take a while: ask how it is going until it is over
  let sawRunning = false;
  async function watchJob() {
    clearInterval(jobTimer);
    const check = async () => {
      try { job = await videoConvertStatus(); } catch { return; }
      if (job?.state === 'running') { sawRunning = true; return; }
      clearInterval(jobTimer);
      // reload the list once, when a job we watched has just finished (not again for the same one)
      if (sawRunning && job?.state === 'done') { sawRunning = false; await load(); }
      sawRunning = false;
    };
    await check();
    if (job?.state === 'running') jobTimer = setInterval(check, 2000);
  }
  async function convert(v: VideoItem) {
    note = '';
    try { await videoConvertStart(v.rel); await watchJob(); } catch (e) { note = String(e); }
  }
  async function stopJob() { try { await videoConvertStop(); } catch { /* it ends by itself */ } }
  async function clearConverted() {
    note = '';
    try {
      const freed = await videosConvertedClear();
      note = freed > 0 ? `Deleted the converted copies and freed ${size(freed)}. Your original videos were not touched.` : 'There were no converted copies to delete.';
      await load();
    } catch (e) { note = String(e); }
  }
  function mins(s: number): string { return s < 90 ? `${s} seconds` : `${Math.round(s / 60)} minutes`; }

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

  const canPlay = (v: VideoItem) => v.playable || !!v.converted;

  function play(v: VideoItem) {
    if (canPlay(v)) void vp.play({ rel: v.rel, title: v.title, converted: v.converted });
  }
  function markDone(v: VideoItem) { vp.markDone(v.rel, !vp.progress[v.rel]?.done); }
  function stoppedAt(v: VideoItem): string {
    const p = vp.progress[v.rel];
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
    return t.videos.filter((v) => !vp.progress[v.rel]?.done).length;
  }
</script>

<section class="wrap">
  <h1>Videos</h1>
  {#if error}<p class="err" role="alert">{error}</p>{/if}

  {#if note}<p class="muted small" role="status">{note}</p>{/if}
  {#if job && job.state === 'running'}
    <div class="job" role="status">
      <span>Making <strong>{job.rel.split('/').pop()}</strong> playable… {job.percent === null ? 'getting ready' : `${job.percent}% done`}, {mins(job.seconds)} so far. You can keep using the program.</span>
      <button class="btn" onclick={stopJob}>Stop</button>
      <div class="meter" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={job.percent ?? undefined} aria-label="How far the converting has got"><div class="fill" class:wait={job.percent === null} style={`width:${job.percent ?? 100}%`}></div></div>
    </div>
  {:else if job && job.state === 'failed'}
    <p class="err" role="alert">Could not make {job.rel.split('/').pop()} playable: {job.message}</p>
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

    {#if !inShare && list.topics.some((t) => t.videos.some((v) => !v.playable)) && !list.converter_ready}
      <p class="muted small">Some videos are not MP4 files. To make them playable, put the small helper <code>ffmpeg.exe</code> in the <code>engines</code> folder on the drive, then look again.</p>
    {/if}
    {#if !inShare && list.converted_bytes > 0}
      <p class="muted small">Converted copies take {size(list.converted_bytes)}. <button class="link" onclick={clearConverted} disabled={job?.state === 'running'}>Delete the converted copies</button> (your own videos are not touched).</p>
    {/if}

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
              <li class:now={vp.current?.rel === v.rel} class:off={!v.playable}>
                <button class="main" onclick={() => play(v)} disabled={!canPlay(v)} aria-label={`Play ${v.title}`}>
                  {#if v.thumb && thumbs[v.thumb]}<img src={thumbs[v.thumb]} alt="" loading="lazy" />{:else}<span class="ph" aria-hidden="true">▶</span>{/if}
                  <span class="txt">
                    <span class="t">{v.title}</span>
                    <span class="muted small">{size(v.size)}{stoppedAt(v) ? ` · ${stoppedAt(v)}` : ''}{canPlay(v) ? (v.converted ? ` · ${v.ext.toUpperCase()} file, converted copy ready` : '') : inShare ? ` · ${v.ext.toUpperCase()} file: it has to be converted on the main computer first` : ` · ${v.ext.toUpperCase()} file: it has to be converted before it can play`}</span>
                  </span>
                </button>
                {#if !canPlay(v) && !inShare}
                  <button class="btn" title="Make a copy of this video that the player can play. Your original is not changed." disabled={job?.state === 'running'} onclick={() => convert(v)}>Make playable</button>
                {/if}
                {#if canPlay(v)}
                  <button class="icon" title={vp.progress[v.rel]?.done ? 'Mark as not watched' : 'Mark as watched'} aria-label={vp.progress[v.rel]?.done ? 'Mark as not watched' : 'Mark as watched'} onclick={() => markDone(v)}>{vp.progress[v.rel]?.done ? '✔' : '○'}</button>
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
  .job { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; justify-content: space-between; padding: 10px 12px; background: var(--blue-soft); border: 1px solid var(--blue-line); border-radius: 8px; }
  .meter { flex-basis: 100%; height: 8px; background: var(--panel2); border-radius: 4px; overflow: hidden; }
  .fill { height: 100%; background: var(--blue); transition: width 0.4s; }
  .fill.wait { opacity: 0.35; }
  .link { background: none; border: 0; padding: 0; color: var(--blue); text-decoration: underline; font: inherit; cursor: pointer; }
  .small { font-size: 13px; }
  code { font-family: var(--mono); font-size: 12px; background: var(--panel2); padding: 1px 5px; border-radius: 4px; }
  ul { list-style: none; margin: 0; padding: 0; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .search { flex: 1; min-width: 200px; padding: 9px 12px; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; color: var(--text); font: inherit; }
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
