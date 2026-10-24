<script lang="ts">
  // A month on one page. Used by the Calendar and the Journal. Days with something in them are marked.
  let { selected = '', marks = {}, onpick, onmonth, compact = false }: {
    selected?: string;
    /** day (YYYY-MM-DD) -> how many things fall on it */
    marks?: Record<string, number>;
    onpick: (day: string) => void;
    /** told the first and last day shown whenever the month changes, so the screen can load what is in it */
    onmonth?: (first: string, last: string) => void;
    compact?: boolean;
  } = $props();

  const pad = (n: number) => String(n).padStart(2, '0');
  const iso = (d: Date) => d.getFullYear() + '-' + pad(d.getMonth() + 1) + '-' + pad(d.getDate());
  const now = new Date();
  const start = selected ? new Date(selected + 'T12:00:00') : now;
  let y = $state(start.getFullYear());
  let m = $state(start.getMonth());

  // six weeks starting on the Sunday on or before the 1st
  const days = $derived.by(() => {
    const first = new Date(y, m, 1, 12);
    const from = new Date(y, m, 1 - first.getDay(), 12);
    return Array.from({ length: 42 }, (_, i) => { const d = new Date(from.getFullYear(), from.getMonth(), from.getDate() + i, 12); return { iso: iso(d), n: d.getDate(), other: d.getMonth() !== m }; });
  });
  $effect(() => { const d = days; onmonth?.(d[0].iso, d[41].iso); });
  const title = $derived(new Date(y, m, 1, 12).toLocaleDateString(undefined, { month: 'long', year: 'numeric' }));
  const todayIso = iso(now);
  function step(k: number) { const d = new Date(y, m + k, 1, 12); y = d.getFullYear(); m = d.getMonth(); }
  function goToday() { y = now.getFullYear(); m = now.getMonth(); onpick(todayIso); }
</script>

<div class="mg" class:compact>
  <div class="bar">
    <button class="btn" aria-label="Last month" onclick={() => step(-1)}>&lsaquo;</button>
    <strong>{title}</strong>
    <button class="btn" aria-label="Next month" onclick={() => step(1)}>&rsaquo;</button>
    <button class="btn" onclick={goToday}>Today</button>
  </div>
  <div class="grid" role="grid" aria-label={title}>
    {#each ['S', 'M', 'T', 'W', 'T', 'F', 'S'] as w}<div class="wd">{w}</div>{/each}
    {#each days as d (d.iso)}
      <button class="cell" class:other={d.other} class:today={d.iso === todayIso} class:sel={d.iso === selected} onclick={() => onpick(d.iso)} aria-label={d.iso + (marks[d.iso] ? ', ' + marks[d.iso] + ' things' : '')}>
        <span>{d.n}</span>
        {#if marks[d.iso]}<i class="dot"></i>{/if}
      </button>
    {/each}
  </div>
</div>

<style>
  .mg { display: flex; flex-direction: column; gap: 8px; }
  .bar { display: flex; align-items: center; gap: 8px; }
  .bar strong { flex: 1; text-align: center; font-family: var(--display); letter-spacing: .08em; text-transform: uppercase; }
  .grid { display: grid; grid-template-columns: repeat(7, 1fr); gap: 3px; }
  .wd { text-align: center; font-family: var(--mono); font-size: 11px; color: var(--muted); padding: 2px 0; }
  .cell { position: relative; min-height: 56px; padding: 5px 6px; text-align: left; border: 1px solid var(--line); border-radius: 6px; background: var(--panel); display: flex; flex-direction: column; justify-content: space-between; }
  .compact .cell { min-height: 34px; padding: 3px 5px; }
  .cell.other { opacity: .45; }
  .cell.today { border-color: var(--accent-line); }
  .cell.sel { border-color: var(--blue-line); background: var(--blue-soft); }
  .cell:hover { border-color: var(--blue-line); }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--accent); align-self: flex-end; }
</style>
