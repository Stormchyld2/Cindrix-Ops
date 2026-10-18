<script lang="ts">
  import { listCards, type Card } from '../lib/api';
  import { onMount } from 'svelte';

  let { cardToOpen = $bindable(null) }: { cardToOpen: string | null } = $props();

  let cards: Card[] = $state([]);
  let open: Card | null = $state(null);

  onMount(async () => {
    cards = await listCards();
    if (cardToOpen) {
      open = cards.find((c) => c.id === cardToOpen) ?? null;
      cardToOpen = null;
    }
  });
</script>

<section>
  <h1>Emergency cards</h1>
  <p class="sub">One screen each. No model needed.</p>
  <div class="list">
    {#each cards as c}
      <button class="card" class:on={open?.id === c.id} onclick={() => (open = c)}>
        <strong>{c.title}</strong>
        {#if c.status !== 'reviewed'}<span class="badge">Draft · clinical review pending</span>{/if}
      </button>
    {/each}
  </div>
  {#if open}
    <article class="panel">
      <h2>{open.title}</h2>
      <pre>{open.body}</pre>
    </article>
  {/if}
</section>

<style>
  .list { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; margin: 12px 0 16px; }
  .card { text-align: left; padding: 14px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line); display: flex; flex-direction: column; gap: 6px; }
  .card.on { border-color: var(--blue); }
  .badge { align-self: flex-start; }
  article h2 { margin-bottom: 8px; }
  pre { margin: 0; white-space: pre-wrap; font: inherit; }
</style>
