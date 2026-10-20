<script lang="ts">
  import Ask from './views/Ask.svelte';
  import Library from './views/Library.svelte';
  import Start from './views/Start.svelte';
  import Academy from './views/Academy.svelte';
  import Inventory from './views/Inventory.svelte';
  import Family from './views/Family.svelte';
  import Cascade from './views/Cascade.svelte';
  import Soon from './views/Soon.svelte';
  import Maps from './views/Maps.svelte';
  import Shelf from './views/Shelf.svelte';
  import Archives from './views/Archives.svelte';
  import Drills from './views/Drills.svelte';
  import Binder from './views/Binder.svelte';
  import Settings from './views/Settings.svelte';
  import Music from './views/Music.svelte';
  import MiniPlayer from './lib/MiniPlayer.svelte';
  import { onMount } from 'svelte';
  import { modelStatus, libraryStatus, productInfo, type ModelStatus, type ProductInfo } from './lib/api';
  let ms: ModelStatus | null = $state(null);
  let sharing = $state(false);
  let online = $state(false);
  let product: ProductInfo | null = $state(null);
  async function poll() { ms = await modelStatus(); const l = await libraryStatus(); sharing = l.shared; online = l.online; product = await productInfo(); }
  onMount(() => { const t = setInterval(poll, 3000); poll(); return () => clearInterval(t); });

  type Group = 'Ask' | 'Library' | 'Academy' | 'Maps' | 'Music' | 'Plan' | 'Settings';
  const allGroups: Group[] = ['Ask', 'Library', 'Academy', 'Maps', 'Music', 'Plan', 'Settings'];
  // Each product switches parts of the program on or off (core/src/product.rs).
  const groups = $derived(
    allGroups.filter((g) => {
      if (!product) return true;
      if (g === 'Ask') return product.assistant;
      if (g === 'Library') return product.library;
      if (g === 'Maps') return product.maps;
      if (g === 'Music') return product.music;
      if (g === 'Plan') return product.plan;
      return true;
    })
  );
  const planTabs = ['Start', 'Inventory', 'Drills', 'Family', 'Binder', 'What fails'] as const;

  let group: Group = $state('Ask');
  // If the current screen is switched off for this product, move to the first one that is on.
  $effect(() => { if (groups.length && !groups.includes(group)) group = groups[0]; });
  let plan: (typeof planTabs)[number] = $state('Start');
  const libTabs = ['Shelf', 'Archives', 'Cards'] as const;
  let lib: (typeof libTabs)[number] = $state('Shelf');
  let cardToOpen: string | null = $state(null);

  function openCard(id: string) {
    cardToOpen = id;
    lib = 'Cards';
    group = 'Library';
  }
  function openArchives() {
    lib = 'Archives';
    group = 'Library';
  }
  function goPlan(tab: string) {
    group = 'Plan';
    if ((planTabs as readonly string[]).includes(tab)) plan = tab as (typeof planTabs)[number];
  }
</script>

<div class="shell">
  <header class="no-print">
    <div class="brand">
      <svg width="30" height="30" viewBox="0 0 24 24" aria-hidden="true">
        <defs><linearGradient id="fl" x1="0" y1="1" x2="0" y2="0"><stop offset="0" stop-color="#ff5a1f" /><stop offset="1" stop-color="#ffc247" /></linearGradient></defs>
        <path d="M12 2.2c.9 3.3 4.7 5.3 4.7 9.9a4.7 4.7 0 0 1-9.4 0c0-2.1 1-3.4 2.1-4.5.2 1.5.9 2.2 1.7 2.4-.4-2.8-.2-5.4.9-7.8z" fill="url(#fl)" />
        <path d="M12 12.6c.5 1.2 1.9 1.9 1.9 3.5a1.9 1.9 0 0 1-3.8 0c0-1 .6-1.6 1.2-2.2.4.7.7.8.7-1.3z" fill="#ffe3a0" />
      </svg>
      <div>
        <div class="name">CINDRIX <span>OPS</span>{#if product && product.id !== 'go'}<small> {product.id.toUpperCase()}</small>{/if}</div>
        <div class="tag">Carry the library. Keep the fire.</div>
      </div>
    </div>
    <nav aria-label="Sections">
      {#each groups as g}
        <button class:on={group === g} onclick={() => (group = g)} aria-current={group === g ? 'page' : undefined}>{g}</button>
      {/each}
    </nav>
    {#if group === 'Library'}
      <div class="subnav">
        {#each libTabs as t}
          <button class:on={lib === t} onclick={() => (lib = t)}>{t === 'Cards' ? 'Emergency cards' : t}</button>
        {/each}
      </div>
    {/if}
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
      {#if lib === 'Shelf'}
        <Shelf />
      {:else if lib === 'Archives'}
        <Archives />
      {:else}
        <Library bind:cardToOpen />
      {/if}
    {:else if group === 'Academy'}
      <Academy />
    {:else if group === 'Maps'}
      <Maps />
    {:else if group === 'Music'}
      <Music />
    {:else if group === 'Plan'}
      {#if plan === 'Start'}
        <Start go={goPlan} />
      {:else if plan === 'Inventory'}
        <Inventory />
      {:else if plan === 'Drills'}
        <Drills />
      {:else if plan === 'Binder'}
        <Binder />
      {:else if plan === 'Family'}
        <Family />
      {:else}
        <Cascade go={goPlan} />
      {/if}
    {:else if group === 'Settings'}
      <Settings {openArchives} />
    {:else}
      <Soon title={group} />
    {/if}
  </main>

  {#if product?.music !== false}<MiniPlayer open={() => (group = 'Music')} />{/if}

  <footer class="no-print">
    <span><b class="dot">●</b> Library on this drive</span>
    <span>Model: {ms?.state === 'ready' ? ms.model : ms?.state === 'loading' ? 'loading…' : 'none loaded'}</span>
    <span class:warn-text={sharing || online}>{online ? 'Saving a website: the app is using the internet right now, and only for that.' : sharing ? 'Sharing the library with your network (read only). Everything else stays on this device.' : 'This app only talks to itself on this device.'}</span>
    <span class="right">{product ? product.name + ' · ' : ''}Sample content</span>
  </footer>
</div>

<style>
  .shell { min-height: 100vh; display: flex; flex-direction: column; }
  header { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 10px 24px;
    padding: 12px 24px; background: var(--panel2); border-bottom: 1px solid var(--line); border-top: 3px solid var(--accent); }
  .brand { display: flex; gap: 12px; align-items: center; }
  .name { font-family: var(--display); font-weight: 700; font-size: 24px; letter-spacing: .12em; line-height: 1; }
  .name span { color: var(--accent); }
  .name small { font-size: 12px; letter-spacing: .2em; color: var(--muted); }
  .tag { font-family: var(--mono); text-transform: uppercase; letter-spacing: .14em; font-size: 10px; color: var(--muted); margin-top: 4px; }
  nav { display: flex; flex-wrap: wrap; gap: 6px; }
  nav button { padding: 8px 14px; border-radius: 8px; border: 1px solid transparent; background: transparent; color: #aab3ad; font-family: var(--mono); font-size: 12px; text-transform: uppercase; letter-spacing: .12em; font-weight: 500; }
  nav button.on { background: var(--blue-soft); color: var(--blue); border-color: var(--blue-line); font-weight: 600; }
  .subnav { flex: 1 1 100%; display: flex; flex-wrap: wrap; gap: 4px; padding-top: 6px; border-top: 1px solid var(--line); }
  .subnav button { padding: 6px 10px; background: transparent; border: 0; border-bottom: 2px solid transparent; color: var(--muted); font-family: var(--mono); text-transform: uppercase; letter-spacing: .1em; font-size: 12px; }
  .subnav button.on { color: var(--blue); border-bottom-color: var(--blue); font-weight: 600; }
  main { flex: 1; padding: 20px 24px; display: flex; flex-direction: column; gap: 14px; max-width: 1040px; width: 100%; margin: 0 auto; }
  footer { display: flex; flex-wrap: wrap; gap: 6px 22px; padding: 9px 24px; background: var(--panel2); border-top: 1px solid var(--line);
    font-family: var(--mono); font-size: 12px; color: var(--muted); }
  footer .right { margin-left: auto; }
  .dot { color: var(--ok); }
  .warn-text { color: var(--blue); }
</style>
