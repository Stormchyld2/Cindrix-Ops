<script lang="ts">
  // A half-round gauge with the percentage under the arch (Get ready, mockups/Start.dc.html).
  let { value, label = '' }: { value: number; label?: string } = $props();
  const v = $derived(Math.max(0, Math.min(100, Math.round(value))));
  const L = Math.PI * 70;
</script>

<div class="g" role="img" aria-label={`${label} ${v} percent`}>
  <svg viewBox="0 0 180 104" width="190" height="110">
    <path d="M20 90 A70 70 0 0 1 160 90" fill="none" stroke="#1d2a3a" stroke-width="14" stroke-linecap="round" />
    <path d="M20 90 A70 70 0 0 1 160 90" fill="none" stroke="#3ddc6a" stroke-width="14" stroke-linecap="round" stroke-dasharray={`${(L * v) / 100} ${L}`} style="filter: drop-shadow(0 0 6px #3ddc6a)" />
  </svg>
  <div class="mid"><strong>{v}%</strong><span>{label}</span></div>
</div>

<style>
  .g { position: relative; width: 190px; height: 118px; margin: 0 auto; }
  .mid { position: absolute; left: 0; right: 0; bottom: 0; text-align: center; }
  strong { display: block; font-size: 38px; line-height: 1; color: #fff; }
  span { font-size: 15px; letter-spacing: .06em; text-transform: uppercase; color: #e6edf5; }
</style>
