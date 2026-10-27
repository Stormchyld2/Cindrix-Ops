<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    musicStatus, musicScan, musicQuery, musicFacets, musicPlaylists, musicPlaylistsAll, musicPlaylistLoad, musicPlaylistSave, musicPlaylistDelete, musicClearCache, musicResolve,
    type MusicStatus, type MusicQuery, type MusicEntry, type PlaylistOwned, type MusicTrack, type MusicFacet, type PlaylistMeta,
  } from '../lib/api';
  import { who } from '../lib/person.svelte';
  import PersonPick from '../lib/PersonPick.svelte';
  import { player, fmtTime, EQ_BANDS, EQ_PRESETS } from '../lib/player.svelte';

  const tabs = ['Library', 'Queue', 'Playlists', 'Equalizer'] as const;
  let tab: (typeof tabs)[number] = $state('Library');
  let status: MusicStatus | null = $state(null);
  let error = $state('');
  let note = $state('');

  // browsing
  type Mode = 'All' | 'Albums' | 'Genres' | 'Folders';
  const modes: Mode[] = ['Folders', 'Genres', 'Albums', 'All'];
  const modeName: Record<Mode, string> = { Folders: 'Categories', Genres: 'Genres', Albums: 'Albums', All: 'All songs' };
  type Field = 'all' | 'title' | 'genre';
  let field = $state<Field>('all');
  let genre: string | null = $state(null);
  let mode = $state<Mode>('Folders');
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

  const showFacets = $derived(!text && ((mode === 'Albums' && album === null) || (mode === 'Genres' && genre === null) || mode === 'Folders'));
  const showTracks = $derived(mode === 'All' || !!text || (mode === 'Albums' && album !== null) || (mode === 'Genres' && genre !== null) || mode === 'Folders');

  async function reload() {
    const mine = ++seq;
    busy = true;
    try {
      if (showFacets) {
        const f = mode === 'Albums' ? await musicFacets('albums') : mode === 'Genres' ? await musicFacets('genres') : await musicFacets('folders', folder ?? '');
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
    if (text) return { text, field };
    if (mode === 'Albums') return { artist, album };
    if (mode === 'Genres') return { genre };
    if (mode === 'Folders') return { folder: folder ?? '', direct: true };
    return {};
  }
  async function more() {
    const r = await musicQuery(queryArgs(), tracks.length, PAGE);
    tracks = [...tracks, ...r.tracks];
  }
  function onSearch() { clearTimeout(debounce); debounce = setTimeout(reload, 200); }
  function setMode(m: Mode) { mode = m; artist = null; album = null; genre = null; folder = null; text = ''; void reload(); }
  function openFacet(f: MusicFacet) {
    if (mode === 'Albums') { album = f.name; artist = f.sub; }
    else if (mode === 'Genres') genre = f.name;
    else folder = f.folder;
    void reload();
  }
  function up() {
    if (mode === 'Folders' && folder) folder = folder.includes('/') ? folder.slice(0, folder.lastIndexOf('/')) : null;
    else { artist = null; album = null; genre = null; }
    void reload();
  }
  const crumb = $derived(mode === 'Albums' ? (album ? `${album}${artist ? ' · ' + artist : ''}` : null) : mode === 'Genres' ? (genre === null ? null : genre || 'Not sorted') : mode === 'Folders' ? folder : null);

  async function everything(given?: MusicQuery): Promise<MusicTrack[]> {
    const q = given ?? (text ? { text, field } : mode === 'Folders' ? { folder: folder ?? '' } : queryArgs());
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
  function facetQuery(f: MusicFacet): MusicQuery {
    if (mode === 'Albums') return { artist: f.sub || null, album: f.name };
    if (mode === 'Genres') return { genre: f.name };
    return { folder: f.folder };
  }
  async function playFacet(f: MusicFacet, shuffle: boolean) {
    try {
      const all = await everything(facetQuery(f));
      if (!all.length) return;
      player.setShuffle(shuffle);
      await player.playTracks(all, shuffle ? Math.floor(Math.random() * all.length) : 0);
    } catch (e) { error = String(e); }
  }
  /** Put every song of a category, genre, album or the current list into a playlist (no doubles). */
  async function addManyTo(name: string, q: MusicQuery) {
    if (!name) return;
    try {
      const all = await everything(q);
      const entries = await musicPlaylistLoad(name, who.name);
      const have = new Set(entries.map((e) => e.path));
      let n = 0;
      for (const t of all) if (!have.has(t.rel)) { entries.push({ path: t.rel, title: t.title, artist: t.artist, album: t.album }); n++; }
      await musicPlaylistSave(name, entries, who.name);
      note = `Added ${n} song${n === 1 ? '' : 's'} to “${name}”.`; await loadPlaylists();
    } catch (e) { error = String(e); }
  }
  /** A brand-new playlist from the songs shown (or empty). */
  async function createPlaylist(name: string, q: MusicQuery | null) {
    name = name.trim();
    if (!name) return;
    if (playlists.some((p) => p.name.toLowerCase() === name.toLowerCase())) { error = 'There is already a playlist with that name.'; return; }
    try {
      const all = q ? await everything(q) : [];
      await musicPlaylistSave(name, all.map((t) => ({ path: t.rel, title: t.title, artist: t.artist, album: t.album })), who.name);
      note = q ? `Made “${name}” with ${all.length} songs.` : `Made the empty playlist “${name}”.`; plName = ''; await loadPlaylists();
    } catch (e) { error = String(e); }
  }
  let plName = $state('');
  // songs ticked from any folder, genre or search: kept while you move around, then added to a playlist
  let picks: Map<string, MusicTrack> = $state(new Map());
  function togglePick(t: MusicTrack) {
    const m = new Map(picks);
    if (m.has(t.rel)) m.delete(t.rel); else m.set(t.rel, t);
    picks = m;
  }
  const pickedList = $derived([...picks.values()]);
  const asEntries = (l: MusicTrack[]): MusicEntry[] => l.map((t) => ({ path: t.rel, title: t.title, artist: t.artist, album: t.album }));
  async function addPicksTo(name: string) {
    if (!name) return;
    try {
      const entries = await musicPlaylistLoad(name, who.name);
      const have = new Set(entries.map((e) => e.path));
      const fresh = asEntries(pickedList.filter((t) => !have.has(t.rel)));
      await musicPlaylistSave(name, [...entries, ...fresh], who.name);
      note = `Added ${fresh.length} song${fresh.length === 1 ? '' : 's'} to “${name}”.`; picks = new Map(); await loadPlaylists();
    } catch (e) { error = String(e); }
  }
  async function makeFromPicks() {
    const name = plName.trim();
    if (!name) return;
    if (playlists.some((x) => x.name.toLowerCase() === name.toLowerCase())) { error = 'You already have a playlist with that name.'; return; }
    try {
      await musicPlaylistSave(name, asEntries(pickedList), who.name);
      note = `Made “${name}” with ${pickedList.length} songs.`; plName = ''; picks = new Map(); await loadPlaylists();
    } catch (e) { error = String(e); }
  }
  async function playFrom(i: number) { await player.playTracks(tracks, i); }

  // playlists
  let everyonesLists: PlaylistOwned[] = $state([]);
  async function loadPlaylists() {
    try {
      playlists = await musicPlaylists(who.name);
      everyonesLists = await musicPlaylistsAll();
    } catch (e) { error = String(e); }
  }
  /** Other people's playlists (and the shared ones), grouped by owner. */
  const mineKey = $derived(who.name.trim().replace(/\s+/g, ' ').toLowerCase());
  const othersByOwner = $derived.by(() => {
    const m = new Map<string, PlaylistOwned[]>();
    for (const p of everyonesLists) {
      if (p.owner && p.owner.toLowerCase() === mineKey) continue;
      m.set(p.owner, [...(m.get(p.owner) ?? []), p]);
    }
    return [...m.entries()];
  });
  /** Copy somebody else's playlist into my own set. */
  async function copyToMine(owner: string, name: string) {
    try {
      const entries = await musicPlaylistLoad(name, owner);
      let to = name, n = 2;
      while (playlists.some((x) => x.name.toLowerCase() === to.toLowerCase())) to = `${name} (${n++})`;
      await musicPlaylistSave(to, entries, who.name);
      note = `Copied to your playlists as “${to}”.`; await loadPlaylists();
    } catch (e) { error = String(e); }
  }
  async function saveQueue() {
    const name = newName.trim();
    if (!name) return;
    try {
      await musicPlaylistSave(name, player.queue.map((t) => ({ path: t.rel, title: t.title, artist: t.artist, album: t.album })), who.name);
      note = `Saved “${name}”.`; newName = ''; await loadPlaylists();
    } catch (e) { error = String(e); }
  }
  async function playPlaylist(name: string, owner: string = who.name) {
    try {
      const entries = await musicPlaylistLoad(name, owner);
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
      const entries = await musicPlaylistLoad(name, who.name);
      if (!entries.some((e) => e.path === t.rel)) entries.push({ path: t.rel, title: t.title, artist: t.artist, album: t.album });
      await musicPlaylistSave(name, entries, who.name);
      note = `Added to “${name}”.`; await loadPlaylists();
    } catch (e) { error = String(e); }
  }
  let confirmDelete: string | null = $state(null);
  let open: string | null = $state(null);
  let openEntries: MusicEntry[] = $state([]);
  let renameTo = $state('');
  async function openPlaylist(name: string) {
    try { openEntries = await musicPlaylistLoad(name, who.name); open = name; renameTo = name; } catch (e) { error = String(e); }
  }
  async function moveEntry(name: string, i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= openEntries.length) return;
    const next = [...openEntries];
    [next[i], next[j]] = [next[j], next[i]];
    try { await musicPlaylistSave(name, next, who.name); openEntries = next; } catch (e) { error = String(e); }
  }
  async function dropEntry(name: string, i: number) {
    const next = openEntries.filter((_, k) => k !== i);
    try { await musicPlaylistSave(name, next, who.name); openEntries = next; await loadPlaylists(); } catch (e) { error = String(e); }
  }
  /** Save under the new name first, delete the old one only after that worked. */
  async function renamePlaylist(name: string) {
    const to = renameTo.trim();
    if (playlists.some((p) => p.name.toLowerCase() === to.toLowerCase() && p.name !== name)) { error = 'There is already a playlist with that name.'; return; }
    try {
      await musicPlaylistSave(to, openEntries, who.name);
      if (to.toLowerCase() !== name.toLowerCase()) await musicPlaylistDelete(name, who.name);
      open = to; note = `Renamed to “${to}”.`; await loadPlaylists();
    } catch (e) { error = String(e); }
  }
  async function removePlaylist(name: string) {
    try { await musicPlaylistDelete(name, who.name); confirmDelete = null; await loadPlaylists(); } catch (e) { error = String(e); }
  }

  async function rescan() { await musicScan(); await refreshStatus(); }
  async function clearCache() { const n = await musicClearCache(); note = `Freed ${(n / 1048576).toFixed(0)} MB.`; await refreshStatus(); }
  const label = (t: MusicTrack) => (t.artist ? `${t.artist}${t.album ? ' · ' + t.album : ''}` : t.album || t.ext.toUpperCase());
  const mb = (b: number) => (b / 1048576).toFixed(b > 1e9 ? 0 : 1);
</script>

<div>
  <h1>Music</h1>
  <p class="sub">Your own music, played from this drive. MP3, WAV, FLAC, OGG, Opus, M4A and AAC play straight from the file. WMA and a few others are changed into a playable form while they play; that copy is thrown away when the next song starts, so nothing piles up on the drive.</p>
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
      <p>Copy your music into the <b>music</b> folder on the Cindrix drive. Folders like <code>music/Artist/Album/01 - Song.mp3</code> work best; the program reads the tags inside the files and falls back to the folder and file names. Put songs in genre folders (<code>music/Rock/Artist/Album/01 - Song.mp3</code>) and the <b>Genres</b> button groups them by that folder whenever a file has no genre of its own.</p>
      <p class="muted">Your music is not counted against the drive’s size budget, and it is never changed or uploaded. Then press Look again.</p>
      <div class="row"><button class="btn primary" onclick={rescan}>Look again</button></div>
    </section>
  {:else}
    <section class="panel">
      <div class="row">
        <input class="input grow" type="search" placeholder={field === 'all' ? 'Search songs, categories, genres' : `Search by ${field}`} aria-label="Search music" bind:value={text} oninput={onSearch} />
        <select class="input" aria-label="Search in" title="Choose where the search looks" bind:value={field} onchange={onSearch}>
          <option value="all">Everything</option><option value="title">Song title</option><option value="genre">Genre</option>
        </select>
        <div class="modes" role="group" aria-label="Browse by">
          {#each modes as m}<button class:on={mode === m && !text} onclick={() => setMode(m)}>{modeName[m]}</button>{/each}
        </div>
      </div>
      <div class="row status muted">
        {#if status?.scanning}Looking through your music… {status.seen} songs so far.
        {:else if status}{status.count.toLocaleString()} songs{#if status.skipped} · {status.skipped} files skipped (names the drive cannot use){/if}{/if}
        <button class="link" onclick={rescan} disabled={status?.scanning}>Look again</button>
      </div>
      {#if picks.size}
        <div class="tray row">
          <b>{picks.size} song{picks.size === 1 ? '' : 's'} picked</b>
          <span class="muted small">from any folder; keep browsing and ticking more</span>
          {#if playlists.length}
            <select class="input" aria-label="Add the picked songs to a playlist" onchange={(e) => { void addPicksTo(e.currentTarget.value); e.currentTarget.value = ''; }}>
              <option value="">♫ Add to…</option>
              {#each playlists as p}<option value={p.name}>{p.name}</option>{/each}
            </select>
          {/if}
          <input class="input" maxlength="60" placeholder="Name a new playlist" bind:value={plName} aria-label="New playlist name for the picked songs" />
          <button class="btn primary" disabled={!plName.trim()} onclick={makeFromPicks}>Make playlist</button>
          <button class="btn" onclick={() => (picks = new Map())}>Clear</button>
        </div>
      {/if}
      {#if crumb && !text}
        <div class="row"><button class="btn" onclick={up}>← Back</button><b>{crumb}</b></div>
      {/if}
      {#if showTracks && total > 0}
        <div class="row">
          <button class="btn primary" onclick={() => playAll(false)}>▶ Play {text || crumb ? 'these' : 'all'}</button>
          <button class="btn" onclick={() => playAll(true)}>⤮ Shuffle</button>
          <span class="muted">{total.toLocaleString()} songs</span>
        </div>
        <div class="row">
          {#if playlists.length}
            <select class="input" aria-label="Add these songs to a playlist" onchange={(e) => { void addManyTo(e.currentTarget.value, text ? { text, field } : queryArgs()); e.currentTarget.value = ''; }}>
              <option value="">♫ Add these songs to a playlist…</option>
              {#each playlists as p}<option value={p.name}>{p.name}</option>{/each}
            </select>
          {/if}
          <input class="input" maxlength="60" placeholder="Or name a new playlist" bind:value={plName} aria-label="New playlist name" />
          <button class="btn" disabled={!plName.trim()} onclick={() => createPlaylist(plName, text ? { text, field } : queryArgs())}>Make it from these</button>
        </div>
      {/if}
      {#if showFacets}
        <ul class="facets">
          {#each facets as f}
            <li class="facet">
              <button class="open" onclick={() => openFacet(f)}><span>{f.name || (mode === 'Genres' ? 'Not sorted' : '(unknown)')}{#if f.sub && mode !== 'Albums'}<small> · {f.sub}</small>{/if}</span><small>{f.count}</small></button>
              <button class="icon" title="Play all of these" aria-label={'Play ' + f.name} onclick={() => playFacet(f, false)}>▶</button>
              <button class="icon" title="Shuffle these" aria-label={'Shuffle ' + f.name} onclick={() => playFacet(f, true)}>⤮</button>
              {#if playlists.length}
                <select class="pl" aria-label="Add all of these to a playlist" onchange={(e) => { void addManyTo(e.currentTarget.value, facetQuery(f)); e.currentTarget.value = ''; }}>
                  <option value="">♫</option>
                  {#each playlists as p}<option value={p.name}>{p.name}</option>{/each}
                </select>
              {/if}
            </li>
          {/each}
          {#if !facets.length && !busy}<li class="muted">Nothing here.</li>{/if}
        </ul>
      {/if}
      {#if showTracks}
        <ol class="tracks">
          {#each tracks as t, i (t.rel)}
            <li class:now={player.current?.rel === t.rel}>
              <input type="checkbox" class="pick" checked={picks.has(t.rel)} onchange={() => togglePick(t)} aria-label={'Pick ' + t.title + ' for a playlist'} title="Tick songs from any folder, then put them in a playlist" />
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
            <div class="warn">A few of your songs (WMA, AIFF and similar) cannot be played yet because a small helper is missing. The helper is a file called <code>ffmpeg.exe</code>. Put it in the <code>engines</code> folder next to the program and look again. Songs in MP3, WAV, FLAC, M4A and OGG play without it.</div>
          {:else if status.converter === 'unknown'}Some files need converting; that is checked the first time one is played.
          {:else}Converter ready: other formats are converted to {status.converter === 'opus' ? 'Opus (small)' : 'FLAC (large)'} the first time they play.{/if}
        {/if}
        {#if status.cache_bytes > 0}
          Converted copies use {mb(status.cache_bytes)} MB. This is the copy of a song the player cannot read as it is (such as WMA). It is thrown away when the next song starts, and when the program closes. Your own file is never changed. <button class="link" onclick={clearCache}>Delete it now</button>
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
    <p class="muted small">Make as many playlists as you like. They are spreadsheet files in <code>user/playlists</code>. Add songs from the Library (the ♫ button on a song, a category, a genre or an album), or save the queue from the Queue tab.</p>
    <div class="row"><PersonPick onchange={loadPlaylists} /><span class="muted small">These are {who.name}’s playlists. Everyone can play everyone’s.</span></div>
    <div class="row">
      <input class="input grow" maxlength="60" placeholder="Name for a new, empty playlist" bind:value={plName} aria-label="New playlist name" />
      <button class="btn" disabled={!plName.trim()} onclick={() => createPlaylist(plName, null)}>Make playlist</button>
    </div>
    {#if !playlists.length}<p class="muted">{who.name} has no playlists yet.</p>{/if}
    <ul class="plist">
      {#each playlists as p (p.name)}
        <li>
          <span><b>{p.name}</b> <small class="muted">{p.count} songs</small></span>
          <span class="row">
            <button class="btn" onclick={() => playPlaylist(p.name)}>▶ Play</button>
            <button class="btn" onclick={() => (open === p.name ? (open = null) : openPlaylist(p.name))}>{open === p.name ? 'Close' : 'Edit'}</button>
            {#if confirmDelete === p.name}
              <button class="btn danger" onclick={() => removePlaylist(p.name)}>Delete “{p.name}”</button>
              <button class="btn" onclick={() => (confirmDelete = null)}>Keep</button>
            {:else}
              <button class="btn" onclick={() => (confirmDelete = p.name)}>Delete</button>
            {/if}
          </span>
          {#if open === p.name}
            <div class="edit">
              <div class="row">
                <input class="input grow" maxlength="60" bind:value={renameTo} aria-label="New name" />
                <button class="btn" disabled={!renameTo.trim() || renameTo.trim() === p.name} onclick={() => renamePlaylist(p.name)}>Rename</button>
              </div>
              {#if !openEntries.length}<p class="muted">Empty. Add songs from the Library.</p>{/if}
              <ol class="tracks">
                {#each openEntries as e, i (e.path)}
                  <li>
                    <span class="main"><span class="t">{e.title || e.path}</span><span class="a muted">{e.artist || e.album || e.path}</span></span>
                    <button class="icon" title="Move up" aria-label="Move up" disabled={i === 0} onclick={() => moveEntry(p.name, i, -1)}>▲</button>
                    <button class="icon" title="Move down" aria-label="Move down" disabled={i === openEntries.length - 1} onclick={() => moveEntry(p.name, i, 1)}>▼</button>
                    <button class="icon" title="Take out of this playlist" aria-label="Remove" onclick={() => dropEntry(p.name, i)}>✕</button>
                  </li>
                {/each}
              </ol>
            </div>
          {/if}
        </li>
      {/each}
    </ul>
    {#each othersByOwner as [owner, lists] (owner)}
      <h3 class="owner">{owner ? owner + '’s playlists' : 'Shared playlists'}</h3>
      <ul class="plist">
        {#each lists as p (p.name)}
          <li>
            <span><b>{p.name}</b> <small class="muted">{p.count} songs</small></span>
            <span class="row">
              <button class="btn" onclick={() => playPlaylist(p.name, owner)}>▶ Play</button>
              <button class="btn" title="Make your own copy that you can change" onclick={() => copyToMine(owner, p.name)}>Copy to mine</button>
            </span>
          </li>
        {/each}
      </ul>
    {/each}
  </section>
{:else}
  <section class="panel">
    <div class="row">
      <label class="row" title="Songs whose file says they are loud (ReplayGain) are turned down, so the volume stays about the same from song to song"><input type="checkbox" checked={player.level} onchange={(e) => player.setLevel(e.currentTarget.checked)} /> Match loudness between songs</label>
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
  .facet { display: flex; align-items: center; gap: 2px; }
  .facets .open { flex: 1; min-width: 0; width: auto; }
  .facets button.icon { width: auto; padding: 0; border: 1px solid transparent; background: none; }
  .edit { flex: 1 1 100%; display: flex; flex-direction: column; gap: 8px; padding: 6px 0 4px 10px; border-left: 3px solid var(--line2); }
  .facets button:not(.icon) { width: 100%; display: flex; justify-content: space-between; gap: 8px; text-align: left; padding: 9px 11px; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; color: var(--text); font: inherit; }
  .facets button:hover { border-color: var(--blue-line); }
  .tracks li { display: flex; align-items: center; gap: 4px; border-bottom: 1px solid var(--line); }
  .tracks li.now { background: var(--blue-soft); }
  .tracks .main { flex: 1; min-width: 0; display: flex; flex-direction: column; text-align: left; padding: 8px 6px; background: none; border: 0; color: var(--text); font: inherit; cursor: pointer; }
  .tracks .t { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tracks .a { font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .icon { min-width: 36px; min-height: 36px; background: none; border: 1px solid transparent; border-radius: 8px; color: var(--muted); font-size: 15px; cursor: pointer; }
  .icon:hover { border-color: var(--blue-line); color: var(--blue); }
  .pl { width: 44px; min-height: 36px; background: var(--panel2); color: var(--muted); border: 1px solid var(--line); border-radius: 8px; }
  .tray { position: sticky; top: 0; z-index: 2; padding: 8px 10px; background: var(--panel2); border: 1px solid var(--blue-line); border-radius: 8px; flex-wrap: wrap; align-items: center; gap: 8px; }
  .pick { width: 20px; height: 20px; flex: none; }
  .owner { margin: 14px 0 0; font-size: 15px; }
  .plist li { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 8px; padding: 8px 0; border-bottom: 1px solid var(--line); }
  .eq { display: grid; grid-template-columns: repeat(10, minmax(0, 1fr)); gap: 4px; align-items: end; padding: 8px 0; }
  .eq label { display: flex; flex-direction: column; align-items: center; gap: 6px; }
  .eq input[type='range'] { writing-mode: vertical-lr; direction: rtl; height: 150px; width: 28px; accent-color: var(--blue); }
  .db, .hz { font-family: var(--mono); font-size: 11px; color: var(--muted); }
  @media (max-width: 560px) { .eq input[type='range'] { height: 120px; width: 22px; } }
</style>
