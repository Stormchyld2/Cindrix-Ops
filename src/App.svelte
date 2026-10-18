<script lang="ts">
  import Ask from './views/Ask.svelte';
  import Library from './views/Library.svelte';
  import Start from './views/Start.svelte';
  import Inventory from './views/Inventory.svelte';
  import Family from './views/Family.svelte';
  import Cascade from './views/Cascade.svelte';
  import Soon from './views/Soon.svelte';
  import { onMount } from 'svelte';
  import { modelStatus, type ModelStatus } from './lib/api';
  let ms: ModelStatus | null = $state(null);
  onMount(() => { const t = setInterval(async () => { ms = await modelStatus(); }, 3000); modelStatus().then((m) => (ms = m)); return () => clearInterval(t); });

  type Group = 'Ask' | 'Library' | 'Maps' | 'Plan' | 'Settings';
  const groups: Group[] = ['Ask', 'Library', 'Maps', 'Plan', 'Settings'];
  const planTabs = ['Start', 'Inventory', 'Family', 'What fails'] as const;

  let group: Group = $state('Ask');
  let plan: (typeof planTabs)[number] = $state('Start');
  let cardToOpen: string | null = $state(null);

  function openCard(id: string) {
    cardToOpen = id;
    group = 'Library';
  }
  function goPlan(tab: string) {
    group = 'Plan';
    if ((planTabs as readonly string[]).includes(tab)) plan = tab as (typeof planTabs)[number];
  }
</script>

<div class="shell">
  <header>
    <div class="brand">
      <svg width="30" height="30" viewBox="0 0 24 24" aria-hidden="true">
        <defs><linearGradient id="fl" x1="0" y1="1" x2="0" y2="0"><stop offset="0" stop-color="#ff5a1f" /><stop offset="1" stop-color="#ffc247" /></linearGradient></defs>
        <path d="M12 2.2c.9 3.3 4.7 5.3 4.7 9.9a4.7 4.7 0 0 1-9.4 0c0-2.1 1-3.4 2.1-4.5.2 1.5.9 2.2 1.7 2.4-.4-2.8-.2-5.4.9-7.8z" fill="url(#fl)" />
        <path d="M12 12.6c.5 1.2 1.9 1.9 1.9 3.5a1.9 1.9 0 0 1-3.8 0c0-1 .6-1.6 1.2-2.2.4.7.7.8.7-1.3z" fill="#ffe3a0" />
      </svg>
      <div>
        <div class="name">CINDRIX <span>OPS</span></div>
        <div class="tag">Carry the library. Keep the fire.</div>
      </div>
    </div>
    <nav aria-label="Sections">
      {#each groups as g}
        <button class:on={group === g} onclick={() => (group = g)} aria-current={group === g ? 'page' : undefined}>{g}</button>
      {/each}
    </nav>
    {#if group === 'Plan'}
      <div class="subnav">
        {#each planTabs as t}
          <button class:on={plan === t} onclick={() => (plan = t)}>{t === 'Inventory' ? 'Inventory' : t}</button>
        {/each}
      </div>
    {/if}
  </header>

  <main>
    {#if group === 'Ask'}
      <Ask {openCard} />
    {:else if group === 'Library'}
      <Library bind:cardToOpen />
    {:else if group === 'Plan'}
      {#if plan === 'Start'}
        <Start go={goPlan} />
      {:else if plan === 'Inventory'}
        <Inventory />
      {:else if plan === 'Family'}
        <Family />
      {:else}
        <Cascade go={goPlan} />
      {/if}
    {:else}
      <Soon title={group} />
    {/if}
  </main>

  <footer>
    <span><b class="dot">●</b> Library on this drive</span>
    <span>Model: {ms?.state === 'ready' ? ms.model : ms?.state === 'loading' ? 'loading…' : 'none loaded'}</span>
    <span>This app only talks to itself on this device.</span>
    <span class="right">Sample content</span>
  </footer>
</div>

<style>
  .shell { min-height: 100vh; display: flex; flex-direction: column; }
  header { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 10px 24px;
    padding: 12px 24px; background: var(--panel2); border-bottom: 1px solid var(--line); border-top: 3px solid var(--accent); }
  .brand { display: flex; gap: 12px; align-items: center; }
  .name { font-family: var(--display); font-weight: 700; font-size: 24px; letter-spacing: .12em; line-height: 1; }
  .name span { color: var(--accent); }
  .tag { font-family: var(--mono); text-transform: uppercase; letter-spacing: .14em; font-size: 10px; color: var(--muted); margin-top: 4px; }
  nav { display: flex; flex-wrap: wrap; gap: 6px; }
  nav button { padding: 8px 14px; border-radius: 2px; border: 1px solid transparent; background: transparent; color: #aab3ad; font-family: var(--mono); font-size: 12px; text-transform: uppercase; letter-spacing: .12em; font-weight: 500; }
  nav button.on { background: var(--blue-soft); color: var(--blue); border-color: var(--blue-line); font-weight: 600; }
  .subnav { flex: 1 1 100%; display: flex; flex-wrap: wrap; gap: 4px; padding-top: 6px; border-top: 1px solid var(--line); }
  .subnav button { padding: 6px 10px; background: transparent; border: 0; border-bottom: 2px solid transparent; color: var(--muted); font-family: var(--mono); text-transform: uppercase; letter-spacing: .1em; font-size: 12px; }
  .subnav button.on { color: var(--blue); border-bottom-color: var(--blue); font-weight: 600; }
  main { flex: 1; padding: 20px 24px; display: flex; flex-direction: column; gap: 14px; max-width: 1040px; width: 100%; margin: 0 auto; }
  footer { display: flex; flex-wrap: wrap; gap: 6px 22px; padding: 9px 24px; background: var(--panel2); border-top: 1px solid var(--line);
    font-family: var(--mono); font-size: 12px; color: var(--muted); }
  footer .right { margin-left: auto; }
  .dot { color: var(--ok); }
</style>
