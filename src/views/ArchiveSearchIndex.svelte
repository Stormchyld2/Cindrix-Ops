<script lang="ts">
  // Archives > Search inside articles. The program makes its own word index for each archive (core/src/ftindex.rs),
  // so words inside articles can be searched in Intel and by Ask with no Kiwix program. One index is made at a time,
  // it can be stopped and carries on later, and the archive itself is never changed.
  import { onMount, onDestroy } from 'svelte';
  import { zimIndexList, zimIndexStart, zimIndexStop, zimIndexDelete, type IndexBook, type IndexJob } from '../lib/api';

  let books = $state<IndexBook[]>([]);
  let job = $state<IndexJob | null>(null);
  let note = $state('');
  let depth = $state<Record<string, 'lead' | 'full'>>({});
  let timer: ReturnType<typeof setInterval> | undefined;
  let removing = $state('');

  async function load() {
    try {
      const l = await zimIndexList();
      for (const b of l.books) if (!depth[b.name]) depth[b.name] = b.articles > 100_000 ? 'lead' : 'full';
      books = l.books; job = l.job;
      if (l.job.finished && l.job.message) note = l.job.message;
    } catch (e) { note = String(e); }
  }
  onMount(() => { void load(); timer = setInterval(load, 2000); });
  onDestroy(() => clearInterval(timer));

  async function start(b: IndexBook) { note = ''; try { await zimIndexStart(b.name, depth[b.name] ?? 'lead'); } catch (e) { note = String(e); } await load(); }
  async function stop() { await zimIndexStop(); note = 'Stopping…'; }
  async function remove(b: IndexBook) { try { await zimIndexDelete(b.name); removing = ''; note = 'The search index was removed. The archive is untouched.'; } catch (e) { note = String(e); } await load(); }
  const size = (mb: number) => (mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb} MB`);
  const busy = $derived(!!job?.running);
</script>

{#if books.length}
<div class="panel">
  <div class="label">SEARCH INSIDE ARTICLES</div>
  <p class="muted">Titles can always be searched. To also search the words inside articles, the program reads each archive once and makes its own search index on the same drive. It works with no Kiwix program, and Ask uses it too. Big archives take hours, so "Opening part" reads the start of every article (smaller and quicker). You can stop at any time and carry on later.</p>
  {#if note}<p class="note" role="status">{note}</p>{/if}
  <table class="grid"><thead><tr><th>Archive</th><th>Search index</th><th></th></tr></thead><tbody>
    {#each books as b (b.name)}
      <tr>
        <td><b>{b.title}</b><div class="small muted">{b.articles.toLocaleString()} pages · {size(b.mb)}</div></td>
        <td>
          {#if b.running && job}
            <div class="meter" aria-label="Progress"><div style={`width:${job.joining ? 100 : job.percent}%`}></div></div>
            <div class="small">{job.joining ? 'Joining the pieces together…' : `${job.percent}% read (${job.done.toLocaleString()} of ${job.total.toLocaleString()})`}</div>
          {:else if b.state === 'ready'}
            Ready: {b.docs.toLocaleString()} articles, {b.depth === 'lead' ? 'opening part' : 'whole articles'}, {size(b.index_mb)}
          {:else if b.state === 'partial'}
            Stopped after {b.done.toLocaleString()} of {b.articles.toLocaleString()}. It carries on from there.
          {:else if b.state === 'stale'}
            <span class="weak">The archive file changed since this index was made, so it is not used.</span>
          {:else}
            Not made yet. About {size(depth[b.name] === 'full' ? b.est_full_mb : b.est_lead_mb)} of space.
          {/if}
        </td>
        <td class="acts">
          {#if b.running}
            <button class="btn" onclick={stop}>Stop</button>
          {:else}
            {#if b.state !== 'ready'}
              <select class="input" bind:value={depth[b.name]} aria-label="How much of each article">
                <option value="lead">Opening part of each article</option>
                <option value="full">Whole articles</option>
              </select>
              <button class="btn primary" disabled={busy} onclick={() => start(b)} title={busy ? 'Another index is being made. Wait or stop it first.' : 'Read this archive and make its search index'}>{b.state === 'partial' ? 'Carry on' : b.state === 'stale' ? 'Make it again' : 'Make search index'}</button>
            {/if}
            {#if b.state !== 'none'}
              {#if removing === b.name}<button class="btn" onclick={() => remove(b)}>Yes, remove it</button><button class="btn" onclick={() => (removing = '')}>Keep it</button>
              {:else}<button class="btn" onclick={() => (removing = b.name)}>Remove index</button>{/if}
            {/if}
          {/if}
        </td>
      </tr>
    {/each}
  </tbody></table>
</div>
{/if}

<style>
  .meter { height: 8px; background: var(--panel2, #222); border-radius: 4px; overflow: hidden; min-width: 160px; }
  .meter > div { height: 100%; background: var(--blue, #4a8); transition: width 0.4s; }
</style>
