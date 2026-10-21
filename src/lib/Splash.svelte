<script lang="ts">
  // The loading screen: the logo over a darkened picture of the same fire, a turning wheel and "Loading...".
  // It stays up until the program says it is ready (and at least a moment, so it does not flash).
  let { ready }: { ready: boolean } = $props();
  const MIN_MS = 1600;
  // If the program never answers, do not trap the person behind this screen.
  const MAX_MS = 20000;
  let gaveUp = $state(false);
  $effect(() => {
    const t = setTimeout(() => (gaveUp = true), MAX_MS);
    return () => clearTimeout(t);
  });
  let waited = $state(false);
  let gone = $state(false);
  $effect(() => {
    const t = setTimeout(() => (waited = true), MIN_MS);
    return () => clearTimeout(t);
  });
  const leaving = $derived((ready && waited) || gaveUp);
  $effect(() => {
    if (!leaving) return;
    const t = setTimeout(() => (gone = true), 600);
    return () => clearTimeout(t);
  });
</script>

{#if !gone}
  <div class="splash" class:leaving role="status" aria-live="polite" aria-label="Loading">
    <div class="center">
      <img class="logo" src="/cindrix-logo.png" alt="Cindrix Ops: survival, preparedness, self-reliance. Built to endure." width="420" height="420" />
      <div class="wait">
        <svg class="wheel" width="44" height="44" viewBox="0 0 44 44" aria-hidden="true">
          <circle cx="22" cy="22" r="18" fill="none" stroke="#ff9a1f33" stroke-width="4" />
          <circle class="arc" cx="22" cy="22" r="18" fill="none" stroke="#ff9a1f" stroke-width="4" stroke-linecap="round" stroke-dasharray="30 84" />
        </svg>
        <span class="word">Loading...</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .splash {
    position: fixed; inset: 0; z-index: 1000;
    display: grid; place-items: center;
    background: #090807 url('/splash-bg.jpg') center / cover no-repeat;
    transition: opacity .55s ease;
  }
  /* a dark wash so the logo and the words stand out from the picture */
  .splash::before { content: ''; position: absolute; inset: 0; background: radial-gradient(ellipse at center, #0907066b 0%, #090706cc 100%); }
  .splash.leaving { opacity: 0; pointer-events: none; }
  .center { position: relative; display: flex; flex-direction: column; align-items: center; gap: 18px; padding: 16px; }
  .logo { width: min(420px, 70vw, 56vh); height: auto; filter: drop-shadow(0 8px 30px #000a); }
  .wait { display: flex; align-items: center; gap: 14px; }
  .word { font-family: var(--display); text-transform: uppercase; letter-spacing: .28em; font-size: 15px; color: #f3ddc0; }
  .wheel { animation: turn 1s linear infinite; }
  @keyframes turn { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) {
    .wheel { animation-duration: 3s; }
    .splash { transition: none; }
  }
</style>
