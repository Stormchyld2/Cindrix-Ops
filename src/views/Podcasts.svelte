<script lang="ts">
  import { onMount } from 'svelte';
  import { podcastsList, type PodList, type PodShow, type PodEpisode } from '../lib/api';
  import { podPlayer, fmtLong, SPEEDS } from '../lib/podplayer.svelte';

  let list: PodList | null = $state(null);
  let error = $state('');
  let busy = $state(false);
  let openShow: string | null = $state(null);
  let hideDone = $state(false);

  async function load() {
    busy = true;
    error = '';
    try {
      list = await podcastsList();
      await podPlayer.init();
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }
  onMount(() => { void load(); });

  const shown: PodShow | null = $derived(list?.shows.find((s) => s.folder === openShow) ?? null);
  const episodes: PodEpisode[] = $derived(shown ? shown.episodes.filter((e) => !hideDone || !podPlayer.progress[e.rel]?.done) : []);

  function play(ep: PodEpisode) {
    if (!shown) return;
    // The queue is the whole show, so "next" goes to the next older episode even when finished ones are hidden.
    const i = shown.episodes.findIndex((e) => e.rel === ep.rel);
    void podPlayer.open(shown.episodes, i, shown.name);
  }
  function left(ep: PodEpisode): string {
    const p = podPlayer.progress[ep.rel];
    if (!p) return '';
    if (p.done) return 'Played';
    return p.seconds > 5 ? `Stopped at ${fmtLong(p.seconds)}` : '';
  }
  function size(n: number): string {
    return n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1000))} KB`;
  }
  function unplayed(s: PodShow): number {
    return s.episodes.filter((e) => !podPlayer.progress[e.rel]?.done).length;
  }
</script>

<section class="wrap">
  <h1>Podcasts</h1>
  {#if error}<p class="err" role="alert">{error}</p>{/if}

  {#if podPlayer.current}
    <div class="now">
      <div class="meta">
        <div class="t">{podPlayer.current.title}</div>
        <div class="muted small">{podPlayer.show}{podPlayer.error ? ` · ${podPlayer.error}` : podPlayer.loading ? ' · Loading…' : ''}</div>
      </div>
      <input class="seek" type="range" min="0" max={podPlayer.duration || 0} step="1" value={podPlayer.time} aria-label="Position" oninput={(e) => podPlayer.seek(Number(e.currentTarget.value))} />
      <div class="times muted small"><span>{fmtLong(podPlayer.time)}</span><span>{fmtLong(podPlayer.duration)}</span></div>
      <div class="ctl">
        <button class="btn" onclick={() => podPlayer.prev()} aria-label="Previous episode">⏮</button>
        <button class="btn" onclick={() => podPlayer.back()} aria-label="Back 15 seconds">↺ 15</button>
        <button class="btn primary" onclick={() => podPlayer.toggle()} aria-label={podPlayer.playing ? 'Pause' : 'Play'}>{podPlayer.playing ? '⏸ Pause' : '▶ Play'}</button>
        <button class="btn" onclick={() => podPlayer.forward()} aria-label="Forward 30 seconds">30 ↻</button>
        <button class="btn" onclick={() => podPlayer.next()} aria-label="Next episode">⏭</button>
        <label class="speed">Speed
          <select value={podPlayer.rate} onchange={(e) => podPlayer.setRate(Number(e.currentTarget.value))}>
            {#each SPEEDS as s}<option value={s}>{s}×</option>{/each}
          </select>
        </label>
      </div>
    </div>
  {/if}

  {#if list && !list.folder_found}
    <div class="empty">
      <h2>No podcasts folder yet</h2>
      <p class="muted">Make a folder named <code>podcasts</code> on the drive. Put one folder in it for each show, and the episodes inside that show's folder. Cindrix Radio on your phone saves them in exactly this shape.</p>
    </div>
  {:else if list && list.shows.length === 0}
    <div class="empty">
      <h2>No episodes found</h2>
      <p class="muted">Copy the <code>Podcasts</code> folder from your phone's <code>Downloads/Cindrix</code> folder into the <code>podcasts</code> folder on the drive, then open this page again.</p>
      <button class="btn" onclick={load}>Look again</button>
    </div>
  {:else if list}
    {#if !shown}
      <div class="bar">
        <span class="muted small">{list.shows.length} {list.shows.length === 1 ? 'show' : 'shows'}</span>
        <button class="btn" onclick={load} disabled={busy}>Look again</button>
      </div>
      <ul class="shows">
        {#each list.shows as s (s.folder)}
          <li>
            <button class="show" onclick={() => (openShow = s.folder)}>
              <span class="name">{s.name}</span>
              <span class="muted small">{s.episodes.length} {s.episodes.length === 1 ? 'episode' : 'episodes'}{unplayed(s) < s.episodes.length ? ` · ${unplayed(s)} not played` : ''}</span>
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <div class="bar">
        <button class="btn" onclick={() => (openShow = null)}>← All shows</button>
        <label class="small"><input type="checkbox" bind:checked={hideDone} /> Hide episodes I have played</label>
      </div>
      <h2>{shown.name}</h2>
      <ul class="eps">
        {#each episodes as ep (ep.rel)}
          <li class:now={podPlayer.current?.rel === ep.rel}>
            <button class="main" onclick={() => play(ep)} aria-label={`Play ${ep.title}`}>
              <span class="t">{ep.title}</span>
              <span class="muted small">{ep.date}{ep.date ? ' · ' : ''}{size(ep.size)}{left(ep) ? ` · ${left(ep)}` : ''}</span>
            </button>
            <button class="icon" title={podPlayer.progress[ep.rel]?.done ? 'Mark as not played' : 'Mark as played'} aria-label={podPlayer.progress[ep.rel]?.done ? 'Mark as not played' : 'Mark as played'} onclick={() => podPlayer.markDone(ep.rel, !podPlayer.progress[ep.rel]?.done)}>{podPlayer.progress[ep.rel]?.done ? '✔' : '○'}</button>
          </li>
        {:else}
          <li class="muted">Every episode of this show has been played.</li>
        {/each}
      </ul>
    {/if}
  {:else if busy}
    <p class="muted">Looking through the podcasts folder…</p>
  {/if}
</section>

<style>
  .wrap { display: flex; flex-direction: column; gap: 14px; }
  h1 { margin: 0; font-size: 22px; }
  h2 { margin: 0; font-size: 18px; }
  .err { color: var(--bad, #e66); }
  .small { font-size: 13px; }
  code { font-family: var(--mono); font-size: 12px; background: var(--panel2); padding: 1px 5px; border-radius: 4px; }
  ul { list-style: none; margin: 0; padding: 0; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 10px; }
  .now { display: flex; flex-direction: column; gap: 8px; padding: 12px; background: var(--panel2); border: 1px solid var(--blue-line); border-radius: 10px; }
  .now .t { font-weight: 600; }
  .seek { width: 100%; accent-color: var(--blue); }
  .times { display: flex; justify-content: space-between; font-family: var(--mono); }
  .ctl { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .speed { display: flex; align-items: center; gap: 6px; margin-left: auto; font-size: 13px; }
  .shows { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 8px; }
  .show { width: 100%; display: flex; flex-direction: column; gap: 2px; text-align: left; padding: 12px; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; color: var(--text); font: inherit; cursor: pointer; }
  .show:hover { border-color: var(--blue-line); }
  .show .name { font-weight: 600; }
  .eps li { display: flex; align-items: center; gap: 4px; border-bottom: 1px solid var(--line); }
  .eps li.now { background: var(--blue-soft); }
  .eps .main { flex: 1; min-width: 0; display: flex; flex-direction: column; text-align: left; padding: 10px 6px; background: none; border: 0; color: var(--text); font: inherit; cursor: pointer; }
  .eps .t { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .icon { min-width: 36px; min-height: 36px; background: none; border: 1px solid transparent; border-radius: 8px; color: var(--muted); font-size: 15px; cursor: pointer; }
  .icon:hover { border-color: var(--blue-line); color: var(--blue); }
  .empty { padding: 16px 0; display: flex; flex-direction: column; gap: 10px; align-items: flex-start; }
  .empty h2 { font-size: 20px; }
</style>
