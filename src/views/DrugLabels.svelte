<script lang="ts">
  // Drug labels: the FDA's own label text, to read. Nothing here is advice and the assistant does not quote doses.
  import { onMount, onDestroy } from 'svelte';
  import { drugLabelsStatus, drugLabelsPrepare, drugLabelsSearch, drugLabelsGet, type DrugStatus, type DrugSummary, type DrugLabel } from '../lib/api';

  let st: DrugStatus = $state({ has_files: false, ready: false, count: 0, preparing: false, message: '' });
  let q = $state('');
  let found: DrugSummary[] = $state([]);
  let searched = $state(false);
  let open: DrugLabel | null = $state(null);
  let error = $state('');
  let timer: ReturnType<typeof setInterval> | undefined;
  let want = 0;

  async function refresh() {
    try { st = await drugLabelsStatus(); } catch (e) { error = String(e); }
  }
  onMount(() => { refresh(); timer = setInterval(refresh, 2000); });
  onDestroy(() => { if (timer) clearInterval(timer); });

  async function prepare() {
    error = '';
    try { await drugLabelsPrepare(); await refresh(); } catch (e) { error = String(e); }
  }
  async function find() {
    const n = ++want;
    error = '';
    if (!q.trim()) { found = []; searched = false; return; }
    try {
      const r = await drugLabelsSearch(q);
      if (n === want) { found = r; searched = true; }
    } catch (e) { error = String(e); }
  }
  async function show(id: string) {
    error = '';
    try { open = await drugLabelsGet(id); window.scrollTo?.(0, 0); } catch (e) { error = String(e); }
  }
</script>

<section>
  <h1>Drug labels</h1>
  <p class="sub">The label text for medicines, exactly as the U.S. FDA published it. Look up a brand or common name, or what it is for (for example “heartburn”).</p>
  <p class="warn note"><b>For reading only.</b> This is not advice for you or anyone else, and labels can be out of date. Do not start, stop or change a medicine because of this page: follow your doctor’s written plan or ask a pharmacist. In an emergency, call 911.</p>
  {#if error}<p class="warn" role="alert">{error}</p>{/if}

  {#if st.preparing}
    <div class="panel"><strong>Getting the labels ready…</strong><p class="muted">This can take a few minutes the first time. You can leave this page open.</p></div>
  {:else if !st.ready}
    <div class="panel empty">
      {#if st.has_files}
        <strong>The label files are on the drive.</strong>
        <p class="muted">One more step: make them searchable. It runs once and can take a few minutes.</p>
        <button class="btn primary" onclick={prepare}>Get the labels ready</button>
      {:else}
        <strong>The drug labels are not on this drive yet.</strong>
        <p class="muted">Double-click <b>Get-DrugLabels</b> in the <b>scripts</b> folder while you have internet. It saves the FDA’s label files to the drive (several gigabytes). Then come back here.</p>
      {/if}
      {#if st.message}<p class="muted">{st.message}</p>{/if}
    </div>
  {:else if !open}
    <form class="row" onsubmit={(e) => { e.preventDefault(); find(); }}>
      <input class="input find" type="search" placeholder="Brand, common name, or what it is for" aria-label="Search the drug labels" bind:value={q} />
      <button class="btn primary" type="submit">Look up</button>
    </form>
    <p class="muted">{st.count.toLocaleString()} labels on this drive.</p>
    {#if searched && !found.length}<p class="muted">Nothing matches “{q}”. Try the common name, or fewer words.</p>{/if}
    <div class="list">
      {#each found as d (d.id)}
        <button class="card" onclick={() => show(d.id)}>
          <strong>{d.brand || d.generic}</strong>
          {#if d.brand && d.generic}<span class="muted">{d.generic}</span>{/if}
          {#if d.maker}<span class="muted small">{d.maker}</span>{/if}
        </button>
      {/each}
    </div>
  {:else}
    <div class="row"><button class="btn" onclick={() => (open = null)}>← Back to the list</button></div>
    <article class="panel">
      <h2>{open.brand || open.generic}</h2>
      {#if open.brand && open.generic}<p class="muted">{open.generic}</p>{/if}
      {#if open.maker}<p class="muted small">{open.maker}</p>{/if}
      {#each open.parts as [name, text]}
        <h3>{name}</h3>
        {#each text.split('\n\n') as para}<p>{para}</p>{/each}
      {/each}
      {#if !open.parts.length}<p class="muted">This label has no readable parts here. The full label is on the FDA’s DailyMed site.</p>{/if}
      <p class="muted small">Source: U.S. Food and Drug Administration (openFDA drug labels). Parts of long labels are shortened here.</p>
    </article>
  {/if}
</section>

<style>
  .note { max-width: 760px; }
  .find { max-width: 420px; }
  .empty { margin-top: 12px; max-width: 640px; }
  .list { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; margin: 12px 0 8px; }
  .card { text-align: left; padding: 14px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); display: flex; flex-direction: column; gap: 4px; }
  .card:hover { border-color: var(--blue-line); }
  .small { font-size: 0.85em; }
  article { margin-top: 10px; max-width: 760px; }
  article h3 { margin: 16px 0 4px; }
  article p { line-height: 1.6; white-space: pre-line; }
</style>
