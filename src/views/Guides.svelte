<script lang="ts">
  // Plain pages in a themed folder: Agro (crops, planting, food, livestock, soil and water) and the radio reference.
  // They are read like emergency cards; the person can add their own in user/guides/<section>.
  import { guidesList, type Card } from '../lib/api';
  import { parts, blocks } from '../lib/cardtext';

  let { section, title, intro }: { section: string; title: string; intro: string } = $props();

  let pages: Card[] = $state([]);
  let open: Card | null = $state(null);
  let q = $state('');
  let loaded = $state(false);
  let error = $state('');

  $effect(() => {
    const s = section;
    open = null;
    q = '';
    loaded = false;
    error = '';
    guidesList(s).then((l) => { pages = l; loaded = true; }).catch((e) => { error = String(e); loaded = true; });
  });
  const shown = $derived(pages.filter((c) => !q.trim() || (c.title + ' ' + c.body).toLowerCase().includes(q.trim().toLowerCase())));
</script>

<section>
  <h1>{title}</h1>
  <p class="sub">{intro}</p>
  {#if error}<p class="warn" role="alert">{error}</p>{/if}
  {#if !open}
    {#if pages.length > 4}
      <input class="input find" type="search" placeholder="Find a page" aria-label="Find a page" bind:value={q} />
    {/if}
    {#if loaded && !pages.length && !error}
      <div class="panel empty">
        <strong>Nothing here yet.</strong>
        <p class="muted">Pages are plain text files. Put one in <b>user\guides\{section}</b> on the drive (copy a card from <b>cards</b> as a template) and it shows up here. Every page needs a <b>Source:</b> line at the end.</p>
      </div>
    {/if}
    <div class="list">
      {#each shown as c (c.id)}
        <button class="card" onclick={() => { open = c; window.scrollTo?.(0, 0); }}>
          <strong>{c.title}</strong>
          {#if c.status !== 'reviewed'}<span class="badge">Draft · review pending</span>{/if}
        </button>
      {/each}
    </div>
    {#if pages.length && !shown.length}<p class="muted">No page matches “{q}”.</p>{/if}
  {:else}
    <div class="row"><button class="btn" onclick={() => (open = null)}>← All of {title}</button></div>
    <article class="panel">
      <h2>{open.title}</h2>
      {#each blocks(open.body) as b}
        {#if b.kind === 'p'}<p>{#each parts(b.lines[0]) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</p>
        {:else if b.kind === 'ol'}<ol>{#each b.lines as l}<li>{#each parts(l) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</li>{/each}</ol>
        {:else}<ul>{#each b.lines as l}<li>{#each parts(l) as x}{#if x.b}<b>{x.t}</b>{:else}{x.t}{/if}{/each}</li>{/each}</ul>{/if}
      {/each}
    </article>
  {/if}
</section>

<style>
  .find { max-width: 420px; margin-top: 6px; }
  .empty { margin-top: 12px; max-width: 640px; }
  .list { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; margin: 12px 0 8px; }
  .card { text-align: left; padding: 14px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); display: flex; flex-direction: column; gap: 6px; }
  .card:hover { border-color: var(--blue-line); }
  .badge { align-self: flex-start; }
  article { margin-top: 10px; max-width: 760px; }
  article h2 { margin-bottom: 8px; }
  article p, article li { line-height: 1.6; }
  article ol, article ul { padding-left: 22px; }
</style>
