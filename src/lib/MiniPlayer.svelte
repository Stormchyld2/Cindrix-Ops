<script lang="ts">
  import { onMount } from 'svelte';
  import { player, fmtTime } from './player.svelte';
  let { open }: { open: () => void } = $props();
  // Pick up what was playing last time, so the bar is there when the app opens.
  onMount(() => { void player.init(); });
  const t = $derived(player.current);
  const pct = $derived(player.duration ? Math.min(100, (player.time / player.duration) * 100) : 0);
</script>

{#if t}
  <div class="mini no-print" role="region" aria-label="Music player">
    <div class="bar" style={`width:${pct}%`} aria-hidden="true"></div>
    <div class="inner">
      {#if player.art}<img src={player.art} alt="" class="art" />{:else}<div class="art ph" aria-hidden="true">♪</div>{/if}
      <button class="info" onclick={open} aria-label="Open the music screen">
        <span class="title">{t.title}</span>
        <span class="sub2">{player.error ? player.error : player.loading ? 'Loading…' : (t.artist || t.album || t.ext.toUpperCase())}</span>
      </button>
      <span class="time" aria-hidden="true">{fmtTime(player.time)} / {fmtTime(player.duration)}</span>
      <div class="ctl">
        <button aria-label="Previous" onclick={() => player.prev()}>⏮</button>
        <button class="big" aria-label={player.playing ? 'Pause' : 'Play'} onclick={() => player.toggle()}>{player.playing ? '⏸' : '▶'}</button>
        <button aria-label="Next" onclick={() => player.next()}>⏭</button>
      </div>
      <input class="seek" type="range" min="0" max={player.duration || 0} step="1" value={player.time} aria-label="Position" oninput={(e) => player.seek(Number(e.currentTarget.value))} />
      <button class:on={player.shuffle} class="tog" aria-label="Shuffle" aria-pressed={player.shuffle} onclick={() => player.setShuffle(!player.shuffle)}>⤮</button>
      <button class:on={player.repeat !== 'off'} class="tog" aria-label={`Repeat: ${player.repeat}`} onclick={() => player.cycleRepeat()}>{player.repeat === 'one' ? '🔂' : '🔁'}</button>
      <input class="vol" type="range" min="0" max="1" step="0.02" value={player.volume} aria-label="Volume" oninput={(e) => player.setVolume(Number(e.currentTarget.value))} />
    </div>
  </div>
{/if}

<style>
  .mini { position: sticky; bottom: 0; z-index: 5; background: var(--panel); border-top: 2px solid var(--blue); }
  .bar { position: absolute; top: -2px; left: 0; height: 2px; background: var(--accent); }
  .inner { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 12px; padding: 8px 24px; max-width: 1040px; margin: 0 auto; }
  .art { width: 44px; height: 44px; border-radius: 6px; object-fit: cover; background: var(--panel2); flex: none; }
  .ph { display: grid; place-items: center; color: var(--muted); font-size: 22px; }
  .info { flex: 1 1 140px; min-width: 0; display: flex; flex-direction: column; text-align: left; background: none; border: 0; color: var(--text); font: inherit; cursor: pointer; padding: 0; }
  .title, .sub2 { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub2 { font-size: 12px; color: var(--muted); }
  .time { font-family: var(--mono); font-size: 12px; color: var(--muted); }
  .ctl { display: flex; gap: 2px; }
  button { min-width: 40px; min-height: 40px; background: none; border: 1px solid transparent; border-radius: 8px; color: var(--text); font-size: 16px; cursor: pointer; }
  button:hover { border-color: var(--blue-line); color: var(--blue); }
  .big { background: var(--blue); color: #07131d; min-width: 46px; }
  .big:hover { color: #07131d; background: #7bbdf0; }
  .tog { color: var(--muted); }
  .tog.on { color: var(--blue); border-color: var(--blue-line); }
  .seek { flex: 1 1 100%; order: 10; accent-color: var(--blue); }
  .vol { width: 90px; accent-color: var(--blue); }
  @media (max-width: 640px) { .inner { padding: 8px 12px; } .vol, .time { display: none; } }
</style>
