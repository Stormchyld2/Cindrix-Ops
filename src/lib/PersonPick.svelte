<script lang="ts">
  import { onMount } from 'svelte';
  import { who, choosePerson, everyone, loadPersonList } from './person.svelte';
  import { learner } from './learner.svelte';
  let { onchange }: { onchange?: () => void } = $props();
  onMount(() => { void loadPersonList(); });
</script>

<label class="pick">Whose:
  <select class="input" value={who.name} onchange={(e) => { choosePerson(e.currentTarget.value); onchange?.(); }} aria-label="Choose whose">
    {#each everyone() as n}<option value={n}>{n}</option>{/each}
  </select>
</label>
{#if learner.locked}<span class="muted small">The Family page is locked, so only “Me” is shown. Unlock it on the Family page to choose a person.</span>{/if}

<style>
  .pick { display: inline-flex; gap: 8px; align-items: center; }
  .pick select { min-width: 160px; }
  .small { font-size: 13px; }
</style>
