<script lang="ts">
  // Tells the host when something in Security Operations needs a look, while the program is open (docs/48):
  // a bar at the top, a short beep for something new, and a number in the window title. The program checks again
  // every minute. It cannot tell anyone when it is closed or the computer is asleep, and it says so below.
  import { onMount, onDestroy } from 'svelte';
  import { securityOverview, secLockStatus } from './api';

  let { go }: { go: (tab: string) => void } = $props();

  type Item = { key: string; text: string; urgent: boolean };
  let items = $state<Item[]>([]);
  let seen = $state<string[]>([]); // keys the host has pressed "Got it" on
  let sound = $state(true);
  let timer: ReturnType<typeof setInterval> | undefined;
  let announced = new Set<string>();
  let first = true;
  const baseTitle = typeof document !== 'undefined' ? document.title : '';

  try { sound = localStorage.getItem('alertSound') !== 'off'; } catch { /* fine */ }

  function beep(urgent: boolean) {
    if (!sound) return;
    try {
      const Ctx = window.AudioContext ?? (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
      const ctx = new Ctx();
      const tones = urgent ? [880, 660, 880] : [660];
      tones.forEach((f, i) => {
        const o = ctx.createOscillator(); const g = ctx.createGain();
        o.frequency.value = f; g.gain.value = 0.08;
        o.connect(g); g.connect(ctx.destination);
        o.start(ctx.currentTime + i * 0.25); o.stop(ctx.currentTime + i * 0.25 + 0.18);
      });
      setTimeout(() => void ctx.close(), 1500);
    } catch { /* no sound available: the bar still shows */ }
  }

  async function check() {
    try {
      const s = await secLockStatus();
      if (s.on && !s.unlocked) { items = []; return; }
      const o = await securityOverview();
      items = o.alerts ?? [];
    } catch { return; }
    const keys = new Set(items.map((i) => i.key));
    const fresh = items.filter((i) => !announced.has(i.key) && !seen.includes(i.key));
    // the first check after start only fills the bar: no beep for things that were already there
    if (!first && fresh.length) beep(fresh.some((i) => i.urgent));
    first = false;
    announced = keys;
    seen = seen.filter((k) => keys.has(k));
  }

  const open = $derived(items.filter((i) => !seen.includes(i.key)));
  $effect(() => { if (typeof document !== 'undefined') document.title = open.length ? `(${open.length}) ${baseTitle}` : baseTitle; });
  const urgent = $derived(open.some((i) => i.urgent));
  function toggleSound() { sound = !sound; try { localStorage.setItem('alertSound', sound ? 'on' : 'off'); } catch { /* fine */ } if (sound) beep(false); }

  onMount(() => { void check(); timer = setInterval(check, 60_000); });
  onDestroy(() => { if (timer) clearInterval(timer); if (typeof document !== 'undefined') document.title = baseTitle; });
</script>

{#if open.length}
  <div class="alerts" class:urgent role="alert">
    <div class="list">
      {#each open.slice(0, 3) as i (i.key)}<div>{i.text}</div>{/each}
      {#if open.length > 3}<div class="more">and {open.length - 3} more</div>{/if}
    </div>
    <button class="btn" onclick={() => go('Security')} title="Open Security Operations">Open Security</button>
    <button class="btn" onclick={() => (seen = [...seen, ...open.map((i) => i.key)])} title="Hide these until something new comes up">Got it</button>
    <button class="btn" onclick={toggleSound} title="Turn the beep on or off" aria-pressed={sound}>{sound ? 'Sound on' : 'Sound off'}</button>
  </div>
{/if}

<style>
  .alerts { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; padding: 8px 16px; background: var(--warn-soft, #fff4d6); border-bottom: 1px solid var(--warn-line, #e0b84a); color: var(--text); }
  .alerts.urgent { background: var(--bad-soft, #fde2e2); border-color: var(--bad-line, #d66); }
  .list { flex: 1 1 320px; font-size: 14px; }
  .more { color: var(--muted); }
</style>
