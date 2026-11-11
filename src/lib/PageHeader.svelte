<script lang="ts">
  import type { Snippet } from 'svelte';
  // The banner at the top of the main screens (mockups/*.html): small eyebrow, big title, one line, buttons on the right,
  // and a drawn night landscape behind it. The art is made here, so nothing is downloaded.
  let { eyebrow = 'CINDRIX OPS', title, tagline = '', children }: { eyebrow?: string; title: string; tagline?: string; children?: Snippet } = $props();
</script>

<header class="ph">
  <svg class="art" viewBox="0 0 900 180" preserveAspectRatio="xMaxYMid slice" aria-hidden="true">
    <defs>
      <linearGradient id="phsky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1b2a3d" /><stop offset="1" stop-color="#141c27" /></linearGradient>
      <radialGradient id="phmoon" cx=".5" cy=".5" r=".5"><stop offset="0" stop-color="#cfe6ff" stop-opacity=".95" /><stop offset="1" stop-color="#67b4ff" stop-opacity="0" /></radialGradient>
    </defs>
    <rect width="900" height="180" fill="url(#phsky)" />
    <circle cx="700" cy="52" r="46" fill="url(#phmoon)" />
    <circle cx="700" cy="52" r="15" fill="#e6f2ff" />
    <g fill="#9ec8f5" opacity=".7"><circle cx="520" cy="30" r="1.4" /><circle cx="580" cy="62" r="1.1" /><circle cx="800" cy="28" r="1.4" /><circle cx="850" cy="70" r="1.1" /><circle cx="620" cy="22" r="1" /></g>
    <path d="M300 180 L420 92 L470 128 L560 58 L690 150 L760 108 L900 170 L900 180 Z" fill="#202f43" />
    <path d="M380 180 L520 112 L600 150 L700 96 L820 156 L900 130 L900 180 Z" fill="#192433" />
    <g fill="#0f1620"><path d="M610 180 l14 -46 l14 46z M632 180 l12 -36 l12 36z M826 180 l16 -52 l16 52z M850 180 l12 -40 l12 40z M580 180 l10 -32 l10 32z" /></g>
    <rect y="168" width="900" height="12" fill="#0f1620" />
  </svg>
  <div class="txt">
    <div class="eyebrow">{eyebrow}</div>
    <h1>{title}</h1>
    {#if tagline}<p class="tag">{tagline}</p>{/if}
  </div>
  {#if children}<div class="acts">{@render children()}</div>{/if}
</header>

<style>
  .ph { position: relative; display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px 18px; min-height: 132px; padding: 20px 24px; border: 1px solid var(--line2); border-radius: 14px; overflow: hidden; background: #141c27; }
  .art { position: absolute; inset: 0; width: 100%; height: 100%; }
  .ph::after { content: ''; position: absolute; inset: 0; background: linear-gradient(90deg, rgba(20, 21, 23, .92) 0%, rgba(20, 21, 23, .55) 55%, rgba(20, 21, 23, 0) 100%); pointer-events: none; }
  .txt, .acts { position: relative; z-index: 1; }
  .eyebrow { font-family: var(--mono); font-size: 11px; letter-spacing: .22em; color: var(--blue); }
  h1 { margin: 4px 0 2px; font-size: 34px; line-height: 1.05; letter-spacing: .08em; }
  .tag { margin: 4px 0 0; max-width: 560px; color: var(--muted); }
  .acts { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
</style>
