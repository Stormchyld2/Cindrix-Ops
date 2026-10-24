<script lang="ts">
  // "Connected to": the things a task, event or project points at, with a search to add more.
  // The same chips show everywhere, so one idea (a water filter) leads to its item, its plan and its guide.
  import { connectResolve, connectSearch, type Link, type Ref, type Hit } from './api';
  import { openLink } from './nav.svelte';

  let { links = [], onchange }: { links: Link[]; onchange?: (v: Link[]) => void } = $props();
  let names: Ref[] = $state([]);
  let adding = $state(false);
  let q = $state('');
  let hits: Hit[] = $state([]);
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    const l = links;
    connectResolve(l).then((r) => (names = r)).catch(() => (names = l.map((x) => ({ ...x, title: x.id }))));
  });
  const KIND: Record<string, string> = { item: 'Inventory', plan: 'Plan', card: 'Card', guide: 'Guide', person: 'Person', project: 'Project', task: 'To-do', event: 'Calendar', course: 'Course', drill: 'Drill', chat: 'Chat' };
  function look() {
    clearTimeout(timer);
    timer = setTimeout(async () => { hits = q.trim().length >= 2 ? await connectSearch(q.trim()).catch(() => []) : []; }, 200);
  }
  function add(h: Hit) {
    if (!links.some((l) => l.kind === h.kind && l.id === h.id)) onchange?.([...links, { kind: h.kind, id: h.id }]);
    adding = false; q = ''; hits = [];
  }
  const remove = (l: Link) => onchange?.(links.filter((x) => !(x.kind === l.kind && x.id === l.id)));
</script>

<div class="conn">
  {#each names as r (r.kind + ':' + r.id)}
    <span class="chip">
      <button class="open" title="Open this" onclick={() => openLink(r.kind, r.id)}><small>{KIND[r.kind] ?? r.kind}</small> {r.title}</button>
      {#if onchange}<button class="x" aria-label={'Take off ' + r.title} title="Take this link off" onclick={() => remove(r)}>&times;</button>{/if}
    </span>
  {/each}
  {#if onchange}
    {#if adding}
      <div class="find">
        <input class="input" placeholder="Search items, plans, cards, guides, tasks…" bind:value={q} oninput={look} aria-label="Search for something to link" />
        {#if hits.length}
          <ul>
            {#each hits.slice(0, 8) as h (h.kind + ':' + h.id)}
              <li><button onclick={() => add(h)}><small>{KIND[h.kind] ?? h.kind}</small> {h.title}</button></li>
            {/each}
          </ul>
        {/if}
        <button class="btn" onclick={() => { adding = false; q = ''; hits = []; }}>Cancel</button>
      </div>
    {:else}
      <button class="btn" onclick={() => (adding = true)} title="Link this to an item, plan, card or guide">+ Link</button>
    {/if}
  {/if}
</div>

<style>
  .conn { display: flex; flex-wrap: wrap; gap: 6px; align-items: flex-start; }
  .chip { display: inline-flex; border: 1px solid var(--line2); border-radius: 999px; overflow: hidden; background: var(--panel2); }
  .chip button { border: 0; background: transparent; padding: 3px 10px; font-size: 13px; }
  .chip .open:hover { color: var(--blue); }
  .chip .x { padding: 3px 8px; color: var(--muted); }
  small { font-family: var(--mono); font-size: 10px; text-transform: uppercase; letter-spacing: .08em; color: var(--muted); margin-right: 4px; }
  .find { display: flex; flex-direction: column; gap: 6px; min-width: 280px; }
  .find ul { list-style: none; margin: 0; padding: 0; border: 1px solid var(--line2); border-radius: 8px; overflow: hidden; }
  .find li button { width: 100%; text-align: left; border: 0; background: var(--panel); padding: 7px 10px; border-bottom: 1px solid var(--line); }
  .find li button:hover { background: var(--blue-soft); }
</style>
