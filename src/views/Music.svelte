<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    musicStatus, musicScan, musicQuery, musicFacets, musicPlaylists, musicPlaylistLoad, musicPlaylistSave, musicPlaylistDelete, musicClearCache, musicResolve,
    type MusicStatus, type MusicTrack, type MusicFacet, type PlaylistMeta,
  } from '../lib/api';
  import { player, fmtTime, EQ_BANDS, EQ_PRESETS } from '../lib/player.svelte';

  const tabs = ['Library', 'Queue', 'Playlists', 'Equalizer'] as const;
  let tab: (typeof tabs)[number] = $state('Library');
  let status: MusicStatus | null = $state(null);
  let error = $state('');
  let note = $state('');

  // browsing
  type Mode = 'All' | 'Artists' | 'Albums' | 'Folders';
  const modes: Mode[] = ['All', 'Artists', 'Albums', 'Folders'];
  let mode = $state<Mode>('All');
  let text = $state('');
  let artist: string | null = $state(null);
  let album: string | null = $state(null);
  let folder: string | null = $state(null);
  let facets: MusicFacet[] = $state([]);
  let tracks: MusicTrack[] = $state([]);
  let total = $state(0);
  let busy = $state(false);
  const PAGE = 200;
  let seq = 0;

  let playlists: PlaylistMeta[] = $state([]);
  let newName = $state('');
  let timer: ReturnType<typeof setInterval>;
  let debounce: ReturnType<typeof setTimeout>;

  onMount(() => {
    void player.init();
    void refreshStatus().then(reload);
    void loadPlaylists();
    timer = setInterval(refreshStatus, 2000);
  });
  onDestroy(() => { clearInterval(timer); clearTimeout(debounce); });

  let lastCount = -1;
  async function refreshStatus() {
    try {
      status = await musicStatus();
      if (status.count !== lastCount && !status.scanning) { lastCount = status.count; await reload(); }
    } catch (e) { error = String(e); }
  }

  const showFacets = $derived(!text && ((mode === 'Artists' && artist === null) || (mode === 'Albums' && album === null) || mode === 'Folders'));
  const showTracks = $derived(mode === 'All' || !!text || (mode === 'Artists' && artist !== null) || (mode === 'Albums' && album !== null) || mode === 'Folders');

  async function reload() {
    const mine = ++seq;
    busy = true;
    try {
      if (showFacets) {
        const f = mode === 'Artists' ? await musicFacets('artists') : mode === 'Albums' ? await musicFacets('albums') : await musicFacets('folders', folder ?? '');
        if (mine !== seq) return;
        facets = f;
      } else facets = [];
      if (showTracks) {
        const r = await musicQuery(queryArgs(), 0, PAGE);
        if (mine !== seq) return;
        tracks = r.tracks; total = r.total;
      } else { tracks = []; total = 0; }
      error = '';
    } catch (e) { error = String(e); }
    finally { if (mine === seq) busy = false; }
  }
  function queryArgs() {
    if (text) return { text };
    if (mode === 'Artists') return { artist };
    if (mode === 'Albums') return { artist, album };
    if (mode === 'Folders') return { folder: folder ?? '', direct: true };
    return {};
  }
  async function more() {
    const r = await musicQuery(queryArgs(), tracks.length, PAGE);
    tracks = [...tracks, ...r.tracks];
  }
  function onSearch() { clearTimeout(debounce); debounce = setTimeout(reload, 200); }
  function setMode(m: Mode) { mode = m; artist = null; album = null; folder = null; text = ''; void reload(); }
  function openFacet(f: MusicFacet) {
    if (mode === 'Artists') artist = f.name;
    else if (mode === 'Albums') { album = f.name; artist = f.sub; }
    else folder = f.folder;
    void reload();
  }
  function up() {
    if (mode === 'Folders' && folder) folder = folder.includes('/') ? folder.slice(0, folder.lastIndexOf('/')) : null;
    else { artist = null; album = null; }
    void reload();
  }
  const crumb = $derived(mode === 'Artists' ? artist : mode === 'Albums' ? (album ? `${album}${artist ? ' · ' + artist : ''}` : null) : mode === 'Folders' ? folder : null);

  async function everything(): Promise<MusicTrack[]> {
    const q = text ? { text } : mode === 'Folders' ? { folder: folder ?? '' } : queryArgs();
    const r = await musicQuery(q, 0, 500);
    let all = r.tracks;
    while (all.length < r.total && all.length < 5000) all = [...all, ...(await musicQuery(q, all.length, 500)).tracks];
    return all;
  }
  async function playAll(shuffle: boolean) {
    const all = await everything();
    player.setShuffle(shuffle);
    await player.playTracks(all, shuffle ? Math.floor(Math.random() * all.length) : 0);
  }
  async function playFrom(i: number) { await player.playTracks(tracks, i); }

  // playlists
  async function loadPlaylists() { try { playlists = await musicPlaylists(); } catch (e) { error = String(e); } }
  async function saveQueue() {
    const name = newName.trim();
    if (!name) return;
    try {
      await musicPlaylistSave(name, player.queue.map((t) => ({ path: t.rel, title: t.title, artist: t.artist, album: t.album })));
      note = `Saved “${name}”.`; newName = ''; await loadPlaylists();
    } catch (e) { error = String(e); }
  }
  async function playPlaylist(name: string) {
    try {
      const entries = await musicPlaylistLoad(name);
      const found = await musicResolve(entries.map((e) => e.path));
      const byRel = new Map(found.map((t) => [t.rel, t]));
      const list = entries.map((e) => byRel.get(e.path)).filter((t): t is MusicTrack => !!t);
      if (!list.length) { error = 'None of this playlist’s songs are on the drive any more.'; return; }
      note = list.length < entries.length ? `${entries.length - list.length} song(s) in this playlist are missing and were skipped.` : '';
      await player.playTracks(list, 0);
    } catch (e) { error = String(e); }
  }
  async function addToPlaylist(name: string, t: MusicTrack) {
    if (!name) return;
    try {
      const entries = await musicPlaylistLoad(name);
      if (!entries.some((e) => e.path === t.rel)) entries.push({ path: t.rel, title: t.title, artist: t.artist, album: t.album });
      await musicPlaylistSave(name, entries);
      note = `Added to “${name}”.`; await loadPlaylists();
    } catch (e) { error = String(e); }
  }
  let confirmDelete: string | null = $state(null);
  async function removePlaylist(name: string) {
    try { await musicPlaylistDelete(name); confirmDelete = null; await loadPlaylists(); } catch (e) { error = String(e); }
  }

  async function rescan() { await musicScan(); await refreshStatus(); }
  async function clearCache() { const n = await musicClearCache(); note = `Freed ${(n / 1048576).toFixed(0)} MB.`; await refreshStatus(); }
  const label = (t: MusicTrack) => (t.artist ? `${t.artist}${t.album ? ' · ' + t.album : ''}` : t.album || t.ext.toUpperCase());
  const mb = (b: number) => (b / 1048576).toFixed(b > 1e9 ? 0 : 1);
</script>

<div>
  <h1>Music</h1>
  <p class="sub">Your own music, played from this drive. MP3, WAV, FLAC, OGG, Opus, M4A and AAC play directly; WMA and a few others are converted once, then kept.</p>
</div>

{#if error}<div class="warn" role="alert">{error}</div>{/if}
{#if note}<div class="note" role="status">{note}</div>{/if}

<div class="subtabs" role="tablist">
  {#each tabs as t}
    <button role="tab" aria-selected={tab === t} class:on={tab === t} onclick={() => (tab = t)}>{t}{t === 'Queue' && player.queue.length ? ` (${player.queue.length})` : ''}</button>
  {/each}
</div>

{#if tab === 'Library'}
  {#if status && (!status.folder_exists || (status.scanned_once && status.count === 0 && !status.scanning))}
    <section class="panel empty">
      <h2>Put your music on the drive</h2>
      <p>Copy your music into the <b>music</b> folder on the Cindrix drive. Folders like <code>music/Artist/Album/01 - Song.mp3</code> work best; the program reads the tags inside the files and falls back to the folder and file names.</p>
      <p class="muted">Your music is not counted against the drive’s size budget, and it is never changed or uploaded. Then press Look again.</p>
      <div class="row"><button class="btn primary" onclick={rescan}>Look again</button></div>
    </section>
  {:else}
    <section class="panel">
      <div class="row">
        <input class="input grow" type="search" placeholder="Search songs, artists, albums" aria-label="Search music" bind:value={text} oninput={onSearch} />
        <div class="modes" role="group" aria-label="Browse by">
          {#each modes as m}<button class:on={mode === m && !text} onclick={() => setMode(m)}>{m}</button>{/each}
        </div>
      </div>
      <div class="row status muted">
        {#if status?.scanning}Looking through your music… {status.seen} songs so far.
        {:else if status}{status.count.toLocaleString()} songs{#if status.skipped} · {status.skipped} files skipped (names the drive cannot use){/if}{/if}
        <button class="link" onclick={rescan} disabled={status?.scanning}>Look again</button>
      </div>
      {#if crumb && !text}
        <div class="row"><button class="btn" onclick={up}>← Back</button><b>{crumb}</b></div>
      {/if}
      {#if showTracks && total > 0}
        <div class="row">
          <button class="btn primary" onclick={() => playAll(false)}>▶ Play {text || crumb ? 'these' : 'all'}</button>
          <button class="btn" onclick={() => playAll(true)}>⤮ Shuffle</button>
          <span class="muted">{total.toLocaleString()} songs</span>
        </div>
      {/if}
      {#if showFacets}
        <ul class="facets">
          {#each facets as f}
            <li><button onclick={() => openFacet(f)}><span>{f.name || '(unknown)'}{#if f.sub}<small> · {f.sub}</small>{/if}</span><small>{f.count}</small></button></li>
          {/each}
          {#if !facets.length && !busy}<li class="muted">Nothing here.</li>{/if}
        </ul>
      {/if}
      {#if showTracks}
        <ol class="tracks">
          {#each tracks as t, i (t.rel)}
            <li class:now={player.current?.rel === t.rel}>
              <button class="main" onclick={() => playFrom(i)} aria-label={`Play ${t.title}`}>
                <span class="t">{t.title}</span><span class="a muted">{label(t)}</span>
              </button>
              {#if t.convert}<span class="badge" title="Converted the first time you play it">{t.ext}</span>{/if}
              <button class="icon" title="Play next" aria-label="Play next" onclick={() => player.playNext([t])}>⤴</button>
              <button class="icon" title="Add to queue" aria-label="Add to queue" onclick={() => player.enqueue([t])}>＋</button>
              {#if playlists.length}
                <select class="pl" aria-label="Add to playlist" onchange={(e) => { void addToPlaylist(e.currentTarget.value, t); e.currentTarget.value = ''; }}>
                  <option value="">♫</option>
                  {#each playlists as p}<option value={p.name}>{p.name}</option>{/each}
                </select>
              {/if}
            </li>
          {/each}
        </ol>
        {#if tracks.length < total}<div class="row"><button class="btn" onclick={more}>Show more ({total - tracks.length} left)</button></div>{/if}
      {/if}
    </section>

    {#if status}
      <section class="panel small muted">
        {#if status.needs_converter}
          {#if status.converter === 'none'}
            <div class="warn">Some of your music (WMA or similar) needs the converter, ffmpeg, which is not in this drive’s <code>engines</code> folder. Add it, or convert those files to MP3 or FLAC.</div>
          {:else if status.converter === 'unknown'}Some files need converting; that is checked the first time one is played.
          {:else}Converter ready: other formats are converted to {status.converter === 'opus' ? 'Opus (small)' : 'FLAC (large)'} the first time they play.{/if}
        {/if}
        {#if status.cache_bytes > 0}
          Converted copies use {mb(status.cache_bytes)} MB. <button class="link" onclick={clearCache}>Delete them</button> (your music is not touched; they are made again when needed).
        {/if}
      </section>
    {/if}
  {/if}
{:else if tab === 'Queue'}
  <section class="panel">
    <div class="row">
      <b>{player.queue.length} in the queue</b>
      <button class="btn" onclick={() => player.clear()} disabled={!player.queue.length}>Clear</button>
    </div>
    <div class="row">
      <input class="input grow" placeholder="Save the queue as a playlist" maxlength="60" bind:value={newName} aria-label="Playlist name" />
      <button class="btn" onclick={saveQueue} disabled={!newName.trim() || !player.queue.length}>Save</button>
    </div>
    {#if !player.queue.length}<p class="muted">Nothing queued. Pick a song in the Library.</p>{/if}
    <ol class="tracks">
      {#each player.queue as t, i (t.rel + ':' + i)}
        <li class:now={player.index === i}>
          <button class="main" onclick={() => player.jump(i)}><span class="t">{t.title}</span><span class="a muted">{label(t)}</span></button>
          <button class="icon" aria-label="Move up" onclick={() => player.move(i, -1)}>↑</button>
          <button class="icon" aria-label="Move down" onclick={() => player.move(i, 1)}>↓</button>
          <button class="icon" aria-label="Remove from queue" onclick={() => player.removeAt(i)}>✕</button>
        </li>
      {/each}
    </ol>
  </section>
{:else if tab === 'Playlists'}
  <section class="panel">
    <p class="muted small">Playlists are spreadsheet files in <code>user/playlists</code>. Save the queue as a playlist from the Queue tab.</p>
    {#if !playlists.length}<p class="muted">No playlists yet.</p>{/if}
    <ul class="plist">
      {#each playlists as p}
        <li>
          <span><b>{p.name}</b> <small class="muted">{p.count} songs</small></span>
          <span class="row">
            <button class="btn" onclick={() => playPlaylist(p.name)}>▶ Play</button>
            {#if confirmDelete === p.name}
              <button class="btn danger" onclick={() => removePlaylist(p.name)}>Delete “{p.name}”</button>
              <button class="btn" onclick={() => (confirmDelete = null)}>Keep</button>
            {:else}
              <button class="btn" onclick={() => (confirmDelete = p.name)}>Delete</button>
            {/if}
          </span>
        </li>
      {/each}
    </ul>
  </section>
{:else}
  <section class="panel">
    <div class="row">
      <label class="row"><input type="checkbox" checked={player.eqOn} onchange={(e) => player.setEqOn(e.currentTarget.checked)} /> Equalizer on</label>
      <select class="input" aria-label="Preset" value={player.eqPreset} onchange={(e) => player.setPreset(e.currentTarget.value)}>
        {#each Object.keys(EQ_PRESETS) as n}<option>{n}</option>{/each}
        {#if player.eqPreset === 'Custom'}<option>Custom</option>{/if}
      </select>
    </div>
    <div class="eq" role="group" aria-label="Equalizer bands">
      {#each EQ_BANDS as f, i}
        <label>
          <span class="db">{player.eq[i] > 0 ? '+' : ''}{player.eq[i]}</span>
          <input type="range" min="-12" max="12" step="0.5" value={player.eq[i]} aria-label={`${f >= 1000 ? f / 1000 + ' kHz' : f + ' Hz'}`} oninput={(e) => player.setBand(i, Number(e.currentTarget.value))} disabled={!player.eqOn} />
          <span class="hz">{f >= 1000 ? f / 1000 + 'k' : f}</span>
        </label>
      {/each}
    </div>
    <div class="row">
      <label class="row">Speed
        <select class="input" value={player.rate} onchange={(e) => player.setRate(Number(e.currentTarget.value))}>
          {#each [0.75, 0.9, 1, 1.1, 1.25, 1.5, 2] as r}<option value={r}>{r}×</option>{/each}
        </select>
      </label>
      <label class="row">Sleep timer
        <select class="input" onchange={(e) => { player.setSleep(Number(e.currentTarget.value)); }}>
          <option value={0}>Off</option>
          {#each [15, 30, 45, 60, 90] as m}<option value={m}>{m} min</option>{/each}
        </select>
      </label>
      {#if player.sleepLeft}<span class="muted">Fading out in {fmtTime(player.sleepLeft)}</span>{/if}
    </div>
    <p class="muted small">The equalizer only starts working the first time you change a slider.</p>
  </section>
{/if}

<style>
  .subtabs { display: flex; flex-wrap: wrap; gap: 4px; }
  .subtabs button { padding: 7px 14px; background: transparent; border: 0; border-bottom: 2px solid transparent; color: var(--muted); font-family: var(--mono); text-transform: uppercase; letter-spacing: .1em; font-size: 12px; }
  .subtabs button.on { color: var(--blue); border-bottom-color: var(--blue); font-weight: 600; }
  .panel { display: flex; flex-direction: column; gap: 12px; }
  .grow { flex: 1 1 220px; min-width: 0; }
  .modes { display: flex; gap: 2px; }
  .modes button { padding: 8px 10px; background: transparent; border: 1px solid var(--line2); color: var(--muted); font-family: var(--mono); font-size: 12px; text-transform: uppercase; }
  .modes button.on { color: var(--blue); border-color: var(--blue-line); background: var(--blue-soft); }
  .status { font-size: 13px; }
  .small { font-size: 13px; }
  .link { background: none; border: 0; color: var(--link); text-decoration: underline; padding: 0 4px; font: inherit; cursor: pointer; }
  .note { padding: 8px 12px; border-radius: 8px; background: #12211a; border: 1px solid #25533a; font-size: 13px; }
  .empty h2 { font-size: 20px; }
  code { font-family: var(--mono); font-size: 12px; background: var(--panel2); padding: 1px 5px; border-radius: 4px; }
  ul, ol { list-style: none; margin: 0; padding: 0; }
  .facets { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 6px; }
  .facets button { width: 100%; display: flex; justify-content: space-between; gap: 8px; text-align: left; padding: 9px 11px; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; color: var(--text); font: inherit; }
  .facets button:hover { border-color: var(--blue-line); }
  .tracks li { display: flex; align-items: center; gap: 4px; border-bottom: 1px solid var(--line); }
  .tracks li.now { background: var(--blue-soft); }
  .tracks .main { flex: 1; min-width: 0; display: flex; flex-direction: column; text-align: left; padding: 8px 6px; background: none; border: 0; color: var(--text); font: inherit; cursor: pointer; }
  .tracks .t { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tracks .a { font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .icon { min-width: 36px; min-height: 36px; background: none; border: 1px solid transparent; border-radius: 8px; color: var(--muted); font-size: 15px; cursor: pointer; }
  .icon:hover { border-color: var(--blue-line); color: var(--blue); }
  .pl { width: 44px; min-height: 36px; background: var(--panel2); color: var(--muted); border: 1px solid var(--line); border-radius: 8px; }
  .plist li { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 8px; padding: 8px 0; border-bottom: 1px solid var(--line); }
  .eq { display: grid; grid-template-columns: repeat(10, minmax(0, 1fr)); gap: 4px; align-items: end; padding: 8px 0; }
  .eq label { display: flex; flex-direction: column; align-items: center; gap: 6px; }
  .eq input[type='range'] { writing-mode: vertical-lr; direction: rtl; height: 150px; width: 28px; accent-color: var(--blue); }
  .db, .hz { font-family: var(--mono); font-size: 11px; color: var(--muted); }
  @media (max-width: 560px) { .eq input[type='range'] { height: 120px; width: 22px; } }
</style>
