<script lang="ts">
  import { listCards, type Card } from '../lib/api';
  import { onMount } from 'svelte';
  import { parts, blocks } from '../lib/cardtext';

  let { cardToOpen = $bindable(null) }: { cardToOpen: string | null } = $props();

  let cards: Card[] = $state([]);
  let open: Card | null = $state(null);
  let q = $state('');

  onMount(async () => {
    cards = await listCards();
    if (cardToOpen) {
      open = cards.find((c) => c.id === cardToOpen) ?? null;
      cardToOpen = null;
    }
  });

  const GROUPS: [string, string][] = [
    ['medical', 'FIRST AID AND HEALTH'],
    ['disaster', 'WEATHER AND DISASTERS'],
    ['home', 'AT HOME: FIRE, GAS, POWER, WATER'],
    ['skills', 'SKILLS: FIND YOUR WAY, WATER, SHELTER, FIRE, SIGNALS'],
  ];
  const kindOf = (c: Card) => (c.kind === 'emergency' || !GROUPS.some(([k]) => k === c.kind) ? 'medical' : c.kind);
  const shown = $derived(cards.filter((c) => !q.trim() || (c.title + ' ' + c.body).toLowerCase().includes(q.trim().toLowerCase())));
  const groups = $derived(GROUPS.map(([k, t]) => ({ title: t, list: shown.filter((c) => kindOf(c) === k) })).filter((g) => g.list.length));

</script>

<section>
  <h1>Emergency cards</h1>
  <p class="sub">One screen each. No model needed. {cards.length} cards. You can add your own in Settings, “Your own cards and plans”.</p>
  {#if !open}
    <input class="input find" type="search" placeholder="Find a card (bleeding, flood, choking…)" aria-label="Find a card" bind:value={q} />
    {#each groups as g (g.title)}
      <div class="label gh">{g.title}</div>
      <div class="list">
        {#each g.list as c (c.id)}
          <button class="card" onclick={() => { open = c; window.scrollTo?.(0, 0); }}>
            <strong>{c.title}</strong>
            {#if c.status !== 'reviewed'}<span class="badge">Draft · review pending</span>{/if}
          </button>
        {/each}
      </div>
    {/each}
    {#if cards.length && !groups.length}<p class="muted">No card matches “{q}”.</p>{/if}
  {:else}
    <div class="row"><button class="btn" onclick={() => (open = null)}>← All cards</button></div>
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
  .gh { margin-top: 16px; }
  .list { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; margin: 8px 0 8px; }
  .card { text-align: left; padding: 14px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); display: flex; flex-direction: column; gap: 6px; }
  .card:hover { border-color: var(--blue-line); }
  .badge { align-self: flex-start; }
  article { margin-top: 10px; max-width: 760px; }
  article h2 { margin-bottom: 8px; }
  article p, article li { line-height: 1.6; }
  article ol, article ul { padding-left: 22px; }
</style>
