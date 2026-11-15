<script lang="ts">
  // A round gauge: a ring that fills to `value` percent with the number in the middle.
  let { value, size = 120, label = '' }: { value: number; size?: number; label?: string } = $props();
  const v = $derived(Math.max(0, Math.min(100, Math.round(value))));
  const r = 44;
  const c = 2 * Math.PI * r;
  const tone = $derived(v >= 70 ? 'var(--ok)' : v >= 35 ? 'var(--blue)' : 'var(--warn)');
</script>

<div class="ring" style="width:{size}px;height:{size}px" role="img" aria-label={`${label} ${v} percent`}>
  <svg viewBox="0 0 100 100" width={size} height={size}>
    <circle cx="50" cy="50" r={r} fill="none" stroke="var(--line)" stroke-width="8" />
    <circle cx="50" cy="50" r={r} fill="none" stroke={tone} stroke-width="8" stroke-linecap="round" stroke-dasharray={`${(c * v) / 100} ${c}`} transform="rotate(-90 50 50)" style="filter: drop-shadow(0 0 4px {tone})" />
  </svg>
  <div class="mid"><strong>{v}%</strong>{#if label}<span>{label}</span>{/if}</div>
</div>

<style>
  .ring { position: relative; flex: none; }
  .mid { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; }
  strong { font-size: 26px; line-height: 1; }
  span { margin-top: 3px; font-family: var(--mono); font-size: 10px; letter-spacing: .12em; text-transform: uppercase; color: var(--muted); }
</style>
