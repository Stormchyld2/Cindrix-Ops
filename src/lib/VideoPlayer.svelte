<script lang="ts">
  // The floating video window: stays on screen (and keeps playing) while you move between screens.
  import { videoPlayer as vp } from './videoplayer.svelte';
  let el: HTMLVideoElement | undefined = $state();
  $effect(() => { vp.attach(el ?? null); return () => vp.attach(null); });
</script>

{#if vp.current}
  <aside class="vp" class:big={vp.big} aria-label="Video player">
    <div class="head">
      <span class="title" title={vp.current.title}>{vp.current.title}</span>
      <button class="btn" onclick={() => (vp.big = !vp.big)} title={vp.big ? 'Make the window small. The video keeps playing.' : 'Make the window bigger'}>{vp.big ? 'Smaller' : 'Bigger'}</button>
      <button class="btn" onclick={() => vp.close()} title="Stop the video and close this window">Close</button>
    </div>
    <!-- svelte-ignore a11y_media_has_caption -->
    <video bind:this={el} src={vp.src} controls preload="metadata"
      onloadedmetadata={() => vp.loadedMeta()} ontimeupdate={() => vp.onTime()}
      onplay={() => (vp.playing = true)} onpause={() => { vp.playing = false; vp.save(false); }}
      onended={() => { vp.playing = false; vp.save(true); }} onerror={() => vp.failed()}></video>
    {#if vp.error}<p class="err" role="alert">{vp.error}</p>{/if}
  </aside>
{/if}

<style>
  .vp { position: fixed; right: 14px; bottom: 64px; z-index: 40; width: 320px; max-width: calc(100vw - 28px); background: var(--panel); border: 1px solid var(--blue-line); border-radius: 10px; box-shadow: 0 8px 28px rgba(0, 0, 0, 0.5); padding: 8px; display: flex; flex-direction: column; gap: 6px; }
  .vp.big { width: min(760px, calc(100vw - 28px)); }
  .head { display: flex; align-items: center; gap: 6px; }
  .title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; font-size: 13px; }
  video { width: 100%; max-height: 62vh; background: #000; border-radius: 6px; }
  .err { margin: 0; color: var(--danger-text, var(--danger)); font-size: 13px; }
  @media print { .vp { display: none; } }
</style>
