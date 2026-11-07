<script lang="ts">
  import { podPlayer, fmtLong } from './podplayer.svelte';
  let { open }: { open: () => void } = $props();
  const ep = $derived(podPlayer.current);
  const pct = $derived(podPlayer.duration ? Math.min(100, (podPlayer.time / podPlayer.duration) * 100) : 0);
</script>

{#if ep}
  <div class="mini no-print" role="region" aria-label="Podcast player">
    <div class="bar" style={`width:${pct}%`} aria-hidden="true"></div>
    <div class="inner">
      <button class="info" onclick={open} aria-label="Open the podcasts screen">
        <span class="title">{ep.title}</span>
        <span class="sub2">{podPlayer.error ? podPlayer.error : podPlayer.loading ? 'Loading…' : podPlayer.show}</span>
      </button>
      <span class="time" aria-hidden="true">{fmtLong(podPlayer.time)} / {fmtLong(podPlayer.duration)}</span>
      <div class="ctl">
        <button aria-label="Back 15 seconds" onclick={() => podPlayer.back()}>↺15</button>
        <button class="big" aria-label={podPlayer.playing ? 'Pause' : 'Play'} onclick={() => podPlayer.toggle()}>{podPlayer.playing ? '⏸' : '▶'}</button>
        <button aria-label="Forward 30 seconds" onclick={() => podPlayer.forward()}>30↻</button>
        <button aria-label="Close the podcast player" onclick={() => podPlayer.stop()}>✕</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .mini { position: sticky; bottom: 0; z-index: 5; background: var(--panel); border-top: 2px solid var(--blue); }
  .bar { position: absolute; top: 0; left: 0; height: 2px; background: var(--blue); }
  .inner { display: flex; align-items: center; gap: 10px; padding: 6px 12px; }
  .info { flex: 1; min-width: 0; display: flex; flex-direction: column; text-align: left; background: none; border: 0; color: var(--text); font: inherit; cursor: pointer; }
  .title, .sub2 { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub2 { font-size: 12px; color: var(--muted); }
  .time { font-family: var(--mono); font-size: 12px; color: var(--muted); }
  .ctl { display: flex; gap: 4px; }
  .ctl button { min-width: 40px; min-height: 36px; background: none; border: 1px solid var(--line2); border-radius: 8px; color: var(--text); cursor: pointer; }
  .ctl .big { border-color: var(--blue-line); color: var(--blue); }
  @media (max-width: 560px) { .time { display: none; } }
</style>
