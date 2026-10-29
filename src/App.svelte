<script lang="ts">
  import Ask from './views/Ask.svelte';
  import Library from './views/Library.svelte';
  import Start from './views/Start.svelte';
  import Academy from './views/Academy.svelte';
  import Inventory from './views/Inventory.svelte';
  import Family from './views/Family.svelte';
  import Cascade from './views/Cascade.svelte';
  import Soon from './views/Soon.svelte';
  import Todo from './views/Todo.svelte';
  import Calendar from './views/Calendar.svelte';
  import Projects from './views/Projects.svelte';
  import { nav } from './lib/nav.svelte';
  import Maps from './views/Maps.svelte';
  import Shelf from './views/Shelf.svelte';
  import Archives from './views/Archives.svelte';
  import Intel from './views/Intel.svelte';
  import Drills from './views/Drills.svelte';
  import Binder from './views/Binder.svelte';
  import Settings from './views/Settings.svelte';
  import Dashboard from './views/Dashboard.svelte';
  import Plans from './views/Plans.svelte';
  import SeedVault from './views/SeedVault.svelte';
  import Music from './views/Music.svelte';
  import Guides from './views/Guides.svelte';
  import Bible from './views/Bible.svelte';
  import DrugLabels from './views/DrugLabels.svelte';
  import Journal from './views/Journal.svelte';
  import Photos from './views/Photos.svelte';
  import CommsPlan from './views/CommsPlan.svelte';
  import MiniPlayer from './lib/MiniPlayer.svelte';
  import Splash from './lib/Splash.svelte';
  import { onMount } from 'svelte';
  import { intelRequest, sourceRequest, viewRequest, bibleRequest } from './lib/chat.svelte';
  import { inShare, modelStatus, libraryStatus, productInfo, librariesList, libraryAnswer, privacySetIdle, type ModelStatus, type ProductInfo, type LibraryEntry } from './lib/api';
  import { settings, applyUiScale } from './lib/settings.svelte';
  import { installTooltips, removeTooltips } from './lib/tooltips';
  let ms: ModelStatus | null = $state(null);
  let sharing = $state(false);
  let online = $state(false);
  let product: ProductInfo | null = $state(null);
  // The Cindrix drives plugged in. A drive nobody has answered for is asked about before anything is read from it.
  let libs: LibraryEntry[] = $state([]);
  let dismissed: string[] = $state([]);
  let libNote = $state('');
  // The loading screen shows in the real program, and in a browser tab only when the address ends with ?splash.
  const showSplash = typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || new URLSearchParams(window.location.search).has('splash'));
  let polled = $state(false);
  const asking = $derived(libs.find((l) => l.state === 'ask' && l.present && !dismissed.includes(l.path)));
  async function poll() {
    ms = await modelStatus();
    const l = await libraryStatus();
    sharing = l.shared;
    online = l.online;
    product = await productInfo();
    libs = await librariesList();
    polled = true;
  }
  async function answerLib(path: string, a: 'always' | 'once' | 'never') {
    libNote = '';
    try { await libraryAnswer(path, a); } catch (e) { libNote = String(e); }
    await poll();
  }
  onMount(() => {
    const t = setInterval(poll, 3000);
    poll();
    // the saved settings: open on the screen the owner chose
    void settings.load().then(() => {
      // The Dashboard became the opening screen: switch it once for people who still have the old default.
      if (!settings.values.dashboardDefaultDone) {
        if (settings.values.startScreen === 'Ask') settings.set('startScreen', 'Dashboard');
        settings.set('dashboardDefaultDone', true);
      }
      const want = settings.values.startScreen;
      if (group === 'Dashboard' && want !== 'Dashboard') group = want;
    });
    return () => clearInterval(t);
  });
  // hover text on every button (Settings, General, can be switched off)
  $effect(() => {
    if (!settings.loaded || !settings.values.showHints) return;
    const stop = installTooltips();
    return () => { stop(); removeTooltips(); };
  });
  // interface size (Settings, General)
  $effect(() => { if (settings.loaded) void applyUiScale(settings.values.uiScale); });
  $effect(() => { if (settings.loaded) void privacySetIdle(settings.values.privateLockMinutes); });

  // Intel is the offline internet: the archives read like web pages. It goes with the library.
  type Group = 'Dashboard' | 'Ask' | 'Library' | 'Intel' | 'Academy' | 'Maps' | 'Music' | 'Plan' | 'Inventory' | 'Settings' | 'Corner' | 'Guides' | 'Journal' | 'Photos' | 'CommsPlan' | 'Todo' | 'Calendar' | 'Projects' | 'DrugLabels' | 'Bible' | 'Soon';
  const allGroups: Group[] = ['Dashboard', 'Ask', 'Library', 'Intel', 'Academy', 'Maps', 'Music', 'Plan', 'Inventory', 'Settings', 'Corner', 'Journal', 'Photos', 'CommsPlan', 'Todo', 'Calendar', 'Projects', 'DrugLabels', 'Bible'];
  // Each product switches parts of the program on or off (core/src/product.rs).
  const groups = $derived(
    allGroups.filter((g) => {
      if (!product) return true;
      if (g === 'Ask') return product.assistant;
      if (g === 'Library' || g === 'Intel' || g === 'Corner' || g === 'Bible') return product.library;
      if (g === 'DrugLabels') return product.library && !inShare;
      if (g === 'Maps') return product.maps;
      if (g === 'Music') return product.music;
      if (g === 'Plan' || g === 'Inventory') return product.plan;
      if (g === 'Todo' || g === 'Calendar' || g === 'Projects') return !inShare;
      return true;
    })
  );
  const planTabs = ['Start', 'What next', 'Drills', 'Family', 'Binder', 'Seed vault', 'What fails'] as const;

  let group: Group = $state('Dashboard');
  // If the current screen is switched off for this product, move to the first one that is on.
  $effect(() => { if (groups.length && group !== 'Soon' && group !== 'Guides' && !groups.includes(group)) group = groups[0]; });
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
  /** An Ask source: an emergency card, or a page in an archive (opened in Intel). */
  function openSource(link: string) {
    if (link.startsWith('card:')) { openCard(link.slice(5)); return; }
    if (link.startsWith('/content/')) { intelRequest.path = link; group = 'Intel'; return; }
    if (link.startsWith('bible:')) {
      const r = link.slice(6);
      if (r.startsWith('term:')) bibleRequest.term = r.slice(5); else bibleRequest.ref = r;
      group = 'Bible';
      return;
    }
    if (link.startsWith('academy:')) { sourceRequest.lesson = link.slice(8); group = 'Academy'; return; }
    if (link.startsWith('guide:')) { guideSection = link.slice(6).split('/')[0]; group = 'Guides'; return; }
    if (link.startsWith('plan:')) {
      try { sessionStorage.setItem('cindrix.openPlan', link.slice(5)); } catch { /* the plan list opens instead */ }
      goPlan('What next');
      return;
    }
    if (link.startsWith('book:')) {
      const [id, page] = [link.slice(5, link.lastIndexOf(':')), Number(link.slice(link.lastIndexOf(':') + 1)) || 1];
      sourceRequest.book = { id, page };
      lib = 'Shelf';
      group = 'Library';
    }
  }
  /** A link from a task, event or search hit: open the screen that owns the thing it points at. */
  function openLinkTarget(kind: string, id: string) {
    switch (kind) {
      case 'item': group = 'Inventory'; break;
      case 'plan': openSource('plan:' + id); break;
      case 'card': openCard(id); break;
      case 'book': openSource('book:' + id + ':1'); break;
      case 'guide': guideSection = id.split('/')[0]; group = 'Guides'; break;
      case 'task': group = 'Todo'; break;
      case 'event': group = 'Calendar'; break;
      case 'seed': goPlan('Seed vault'); break;
      case 'season': group = 'Calendar'; break;
      case 'project': group = 'Projects'; break;
      case 'course': sourceRequest.lesson = id; group = 'Academy'; break;
      case 'drill': group = 'Plan'; plan = 'Drills'; break;
      case 'chat': try { sessionStorage.setItem('cindrix.openChat', id); } catch { /* Ask opens on its own */ } group = 'Ask'; break;
    }
  }
  $effect(() => {
    const o = nav.open;
    if (o) { openLinkTarget(o.kind, o.id); setTimeout(() => { if (nav.open === o) nav.open = null; }, 1500); }
  });
  function openIntel() {
    group = 'Intel';
  }
  /** The Dashboard's links: a top-level screen, or a screen inside Plan, Library and so on. */
  function goTo(tab: string) {
    // screens that are planned but not built yet, the Settings tabs, and the Academy's Training sign-off
    if (!tab.startsWith('settings:')) settingsFromComms = false;
    if (tab.startsWith('guide:')) { guideSection = tab.slice(6); group = 'Guides'; return; }
    if (tab.startsWith('soon:')) { soonTitle = tab.slice(5); group = 'Soon'; return; }
    if (tab.startsWith('settings:')) { settingsFromComms = tab === 'settings:Sharing'; viewRequest.settingsTab = tab.slice(9); group = 'Settings'; return; }
    if (tab === 'training') { viewRequest.training = true; group = 'Academy'; return; }
    if ((allGroups as string[]).includes(tab)) { group = tab as Group; return; }
    if (tab === 'Shelf' || tab === 'Archives' || tab === 'Cards') { lib = tab as (typeof libTabs)[number]; group = 'Library'; return; }
    goPlan(tab);
  }
  function goPlan(tab: string) {
    if (tab === 'Inventory') { group = 'Inventory'; return; }
    group = 'Plan';
    if ((planTabs as readonly string[]).includes(tab)) plan = tab as (typeof planTabs)[number];
  }

  // ---- the top bar: groups with a menu each (mockups/Navigation.html) ----
  let soonTitle = $state('');
  let guideSection = $state('crop-guides');
  const guideInfo: Record<string, { title: string; intro: string }> = {
    'crop-guides': { title: 'Crop guides', intro: 'What to grow and how: one page for each crop. Check them against your own area: planting times depend on your frost dates.' },
    'planting-calendar': { title: 'Planting calendar', intro: 'When to plant and harvest, worked from your last and first frost dates.' },
    'food-preservation': { title: 'Food preservation', intro: 'Keeping the harvest safe to eat. Food safety rules here come from the official guides and are not shortened.' },
    'livestock': { title: 'Livestock', intro: 'Keeping animals for eggs, milk and meat. Nothing here gives medicine amounts: for sick animals, ask a vet.' },
    'soil-and-water': { title: 'Soil and water', intro: 'Looking after the ground and the water your food depends on.' },
    'home-remedies': { title: 'Home remedies', intro: 'Simple home measures and herbal products, from CDC, MedlinePlus and the U.S. National Institutes of Health. These pages are drafts until a qualified person has checked them. They give no medicine amounts: for medicines, ask a pharmacist or doctor.' },
    'radio-reference': { title: 'Radio reference', intro: 'Family radios, ham radio, antennas, check-in plans, signalling, beacons and mesh radios, with the rules for the United States. Rules differ in other countries, and they change: check fcc.gov.' },
  };
  let settingsFromComms = $state(false);
  let openMenu: string | null = $state(null);
  let barOpen = $state(false);
  type Entry = { label: string; tab: string; note?: string; show?: boolean; rule?: boolean };
  type Top = { id: string; cat: string; tab?: string; entries?: Entry[]; show: boolean };
  const tops: Top[] = $derived.by(() => {
    const p = product;
    const plan_ = p ? p.plan : true;
    const lib_ = p ? p.library : true;
    return [
      { id: 'Dashboard', cat: 'home', tab: 'Dashboard', show: true },
      { id: 'Ask', cat: 'home', tab: 'Ask', show: p ? p.assistant : true },
      { id: 'Operations', cat: 'ops', show: plan_, entries: [
        { label: 'Get ready', tab: 'Start' }, { label: 'Plans', tab: 'What next' }, { label: 'Drills', tab: 'Drills' },
        { label: 'Inventory', tab: 'Inventory' }, { label: 'Family', tab: 'Family' }, { label: 'Binder', tab: 'Binder' },
        { label: 'What fails next', tab: 'What fails' }, { label: '', tab: '', rule: true, show: !inShare },
        { label: 'Calendar', tab: 'Calendar', show: !inShare }, { label: 'To-do list', tab: 'Todo', show: !inShare },
        { label: 'Projects', tab: 'Projects', show: !inShare } ] },
      { id: 'Intel', cat: 'intel', show: lib_, entries: [
        { label: 'Offline browser', tab: 'Intel', note: 'archives' }, { label: 'Shelf', tab: 'Shelf', note: 'manuals' },
        { label: 'Archives', tab: 'Archives' }, { label: 'Emergency cards', tab: 'Cards' }, { label: 'Bible study', tab: 'Bible', note: 'Canonleaf' },
        { label: 'Home remedies', tab: 'guide:home-remedies' }, { label: 'Drug labels', tab: 'DrugLabels', note: 'FDA', show: !inShare } ] },
      { id: 'Academy', cat: 'academy', show: true, entries: [
        { label: 'Courses', tab: 'Academy' }, { label: 'Training sign-off', tab: 'training' } ] },
      { id: 'Agro', cat: 'agro', show: plan_, entries: [
        { label: 'Crop guides', tab: 'guide:crop-guides' }, { label: 'Planting calendar', tab: 'guide:planting-calendar' },
        { label: 'Food preservation', tab: 'guide:food-preservation' }, { label: 'Livestock', tab: 'guide:livestock' },
        { label: 'Soil and water', tab: 'guide:soil-and-water' }, { label: '', tab: '', rule: true },
        { label: 'Seed vault', tab: 'Seed vault' } ] },
      { id: 'Maps', cat: 'maps', tab: 'Maps', show: p ? p.maps : true },
      { id: 'Morale', cat: 'morale', show: true, entries: [
        { label: 'Reading Corner', tab: 'Corner', note: 'enjoyment books', show: p ? p.library : true },
        { label: 'Music', tab: 'Music', show: p ? p.music : true }, { label: '', tab: '', rule: true },
        { label: 'Journal', tab: 'Journal', note: 'private', show: !inShare }, { label: 'Photos', tab: 'Photos' } ] },
      { id: 'Comms', cat: 'comms', show: true, entries: [
        { label: 'Comms plan', tab: 'CommsPlan', note: 'private', show: !inShare }, { label: 'Radio reference', tab: 'guide:radio-reference' },
        { label: '', tab: '', rule: true, show: !inShare },
        { label: 'Sharing', tab: 'settings:Sharing', note: 'hotspot', show: !inShare } ] },
      { id: 'System', cat: 'system', show: true, entries: [
        { label: 'Settings', tab: 'settings:General' }, { label: 'My details', tab: 'settings:My details', show: !inShare } ] },
    ];
  });
  /** Which word in the top bar the current screen belongs under. */
  const activeTop = $derived.by(() => {
    switch (group) {
      case 'Library': case 'Intel': case 'DrugLabels': case 'Bible': return 'Intel';
      case 'Music': case 'Corner': case 'Journal': case 'Photos': return 'Morale';
      case 'CommsPlan': return 'Comms';
      case 'Plan': return plan === 'Seed vault' ? 'Agro' : 'Operations';
      case 'Inventory': case 'Todo': case 'Calendar': case 'Projects': return 'Operations';
      case 'Settings': return settingsFromComms ? 'Comms' : 'System';
      case 'Guides': {
        const hit = tops.find((t) => t.entries?.some((e) => e.tab === 'guide:' + guideSection));
        return hit ? hit.id : 'Agro';
      }
      case 'Soon': {
        const hit = tops.find((t) => t.entries?.some((e) => e.tab === 'soon:' + soonTitle));
        return hit ? hit.id : soonTitle;
      }
      default: return group;
    }
  });
  function pick(tab: string) {
    openMenu = null;
    barOpen = false;
    goTo(tab);
  }
  function topClick(t: Top) {
    if (t.entries) openMenu = openMenu === t.id ? null : t.id;
    else if (t.tab) pick(t.tab);
  }
</script>

<svelte:window onclick={() => (openMenu = null)} onkeydown={(e) => { if (e.key === 'Escape') openMenu = null; }} />

{#if showSplash}<Splash ready={polled && settings.loaded} />{/if}

<div class="shell">
  <header class="no-print">
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
    <button class="burger" aria-label="Menu" aria-expanded={barOpen} onclick={(e) => { e.stopPropagation(); barOpen = !barOpen; }}>&#9776;</button>
    <nav aria-label="Sections" class:show={barOpen}>
      {#each tops as t (t.id)}
        {#if t.show}
          <div class="item" class:on={activeTop === t.id} class:open={openMenu === t.id} style={`--c: var(--cat-${t.cat})`}>
            <button aria-label={t.id} onclick={(e) => { e.stopPropagation(); topClick(t); }} aria-current={activeTop === t.id ? 'page' : undefined} aria-haspopup={t.entries ? 'menu' : undefined} aria-expanded={t.entries ? openMenu === t.id : undefined}>{t.id}{#if t.entries}<span class="caret">&#9660;</span>{/if}</button>
            {#if t.entries && openMenu === t.id}
              <div class="menu" role="menu">
                {#each t.entries as e}
                  {#if e.rule}{#if e.show !== false}<hr />{/if}
                  {:else if e.show !== false}
                    <button role="menuitem" aria-label={e.label} onclick={(ev) => { ev.stopPropagation(); pick(e.tab); }}>{e.label}{#if e.note}<small>{e.note}</small>{/if}</button>
                  {/if}
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      {/each}
    </nav>
  </header>

  {#if libNote}
    <div class="warn libnote" role="status">{libNote} <button class="btn" onclick={() => (libNote = '')}>OK</button></div>
  {/if}

  <main class:wide={group === 'Intel' || group === 'Bible' || group === 'Calendar' || group === 'Ask'}>
    {#if group === 'Dashboard'}
      <Dashboard go={goTo} />
    {:else if group === 'Ask'}
      <Ask {openCard} {openSource} />
    {:else if group === 'Library'}
      {#if lib === 'Shelf'}
        <Shelf />
      {:else if lib === 'Archives'}
        <Archives {openIntel} />
      {:else}
        <Library bind:cardToOpen />
      {/if}
    {:else if group === 'Intel'}
      <Intel {openArchives} />
    {:else if group === 'Academy'}
      <Academy go={goTo} />
    {:else if group === 'Maps'}
      <Maps />
    {:else if group === 'Music'}
      <Music />
    {:else if group === 'Guides'}
      <Guides section={guideSection} title={guideInfo[guideSection]?.title ?? 'Guides'} intro={guideInfo[guideSection]?.intro ?? ''} />
    {:else if group === 'Bible'}
      <Bible />
    {:else if group === 'DrugLabels'}
      <DrugLabels />
    {:else if group === 'Journal'}
      <Journal />
    {:else if group === 'Photos'}
      <Photos />
    {:else if group === 'CommsPlan'}
      <CommsPlan />
    {:else if group === 'Todo'}
      <Todo />
    {:else if group === 'Calendar'}
      <Calendar />
    {:else if group === 'Projects'}
      <Projects openChat={(n) => openLinkTarget('chat', n)} />
    {:else if group === 'Corner'}
      <Shelf corner />
    {:else if group === 'Plan'}
      {#if plan === 'Start'}
        <Start go={goTo} />
      {:else if plan === 'What next'}
        <Plans go={goTo} />
      {:else if plan === 'Drills'}
        <Drills />
      {:else if plan === 'Binder'}
        <Binder />
      {:else if plan === 'Family'}
        <Family />
      {:else if plan === 'Seed vault'}
        <SeedVault go={goTo} />
      {:else}
        <Cascade go={goTo} />
      {/if}
    {:else if group === 'Inventory'}
      <Inventory />
    {:else if group === 'Settings'}
      <Settings {openArchives} />
    {:else}
      <Soon title={group === 'Soon' ? soonTitle : group} />
    {/if}
  </main>

  {#if asking}
    {@const a = asking}
    <dialog open aria-label="A Cindrix drive was found">
      <div class="dlg">
        <h2>A Cindrix drive was found</h2>
        <p><strong>{a.name}</strong>{a.edition ? ' · ' + a.edition + ' edition' : ''}</p>
        <p class="path">{a.path}</p>
        <p>Do you want the program to read the library, archives and files on it? Nothing on it is ever run, and nothing is read until you say yes.</p>
        <div class="row end">
          <button class="btn" onclick={() => (dismissed = [...dismissed, a.path])}>Not now</button>
          <button class="btn" onclick={() => answerLib(a.path, 'never')}>Never use it</button>
          <button class="btn" onclick={() => answerLib(a.path, 'once')}>Just this time</button>
          <button class="btn" class:primary={a.suggest_always} onclick={() => answerLib(a.path, 'always')}>Always use it{a.suggest_always ? ' (suggested)' : ''}</button>
        </div>
        <p class="muted fine">You can change this later in Settings, Drive.</p>
      </div>
    </dialog>
  {/if}

  {#if product?.music !== false && settings.values.showMiniPlayer}<MiniPlayer open={() => (group = 'Music')} />{/if}

  {#if settings.values.showStatusBar}
  <footer class="no-print">
    <span><b class="dot">●</b> Library on this drive</span>
    <span>Model: {ms?.state === 'ready' ? ms.model : ms?.state === 'loading' ? 'loading…' : 'none loaded'}</span>
    <span class:warn-text={sharing || online}>{online ? 'Saving a website: the app is using the internet right now, and only for that.' : sharing ? 'Sharing the library with your network (read only). Everything else stays on this device.' : 'This app only talks to itself on this device.'}</span>
    <span class="right">{product ? product.name + ' · ' : ''}Sample content</span>
  </footer>
  {/if}
</div>

<style>
  .dlg { padding: 18px; display: flex; flex-direction: column; gap: 10px; }
  .dlg h2 { margin: 0; }
  .dlg p { margin: 0; }
  .dlg .path { font-family: var(--mono); font-size: 12px; word-break: break-all; color: var(--muted, #9aa7b4); }
  .dlg .fine { font-size: 12px; }
  .dlg .end { justify-content: flex-end; }
  .libnote { margin: 8px 16px 0; display: flex; gap: 12px; align-items: center; justify-content: space-between; }
  .shell { min-height: 100vh; display: flex; flex-direction: column; }
  /* the top bar stays put while the page scrolls under it */
  header { position: sticky; top: 0; z-index: 50; display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 10px 24px;
    padding: 12px 24px; background: var(--panel2); border-bottom: 1px solid var(--line); border-top: 3px solid var(--accent); }
  .brand { display: flex; gap: 12px; align-items: center; }
  .name { font-family: var(--display); font-weight: 700; font-size: 24px; letter-spacing: .12em; line-height: 1; }
  .name span { color: var(--accent); }
  .name small { font-size: 12px; letter-spacing: .2em; color: var(--muted); }
  .tag { font-family: var(--mono); text-transform: uppercase; letter-spacing: .14em; font-size: 10px; color: var(--muted); margin-top: 4px; }
  nav { display: flex; flex-wrap: wrap; gap: 4px; }
  .item { position: relative; }
  .item > button { display: flex; gap: 6px; align-items: center; padding: 8px 10px; border-radius: 6px; border: 1px solid transparent; background: transparent; color: #aab3ad; font-family: var(--mono); font-size: 11.5px; text-transform: uppercase; letter-spacing: .1em; font-weight: 500; }
  .item > button::before { content: '\25C6'; font-size: 9px; color: var(--c, var(--muted)); }
  .item > button:hover { color: var(--text); }
  .item.on > button { background: var(--blue-soft); color: var(--blue); border-color: var(--blue-line); font-weight: 600; }
  .item.on > button, .item.open > button { box-shadow: inset 0 -2px 0 var(--c, var(--blue)); }
  .caret { font-size: 9px; opacity: .7; }
  .menu { position: absolute; top: calc(100% + 6px); left: 0; min-width: 210px; z-index: 60; background: #1b1f1d; border: 1px solid var(--line); border-top: 2px solid var(--c, var(--blue)); border-radius: 0 0 6px 6px; padding: 6px; box-shadow: 0 10px 24px #000a; }
  .menu button { display: flex; width: 100%; justify-content: space-between; gap: 14px; padding: 9px 10px; border: 0; border-radius: 4px; background: transparent; color: var(--text); font-family: var(--display); font-size: 15px; text-align: left; }
  .menu button:hover { background: var(--blue-soft); }
  .menu small { color: var(--muted); font-family: var(--mono); font-size: 10px; letter-spacing: .08em; text-transform: uppercase; align-self: center; }
  .menu hr { border: 0; border-top: 1px solid var(--line); margin: 6px 0; }
  .burger { display: none; margin-left: auto; padding: 7px 11px; font-size: 16px; border: 1px solid var(--line); border-radius: 6px; background: transparent; color: var(--text); }
  @media (max-width: 900px) {
    .burger { display: block; }
    nav { display: none; flex: 1 1 100%; flex-direction: column; }
    nav.show { display: flex; }
    .menu { position: static; box-shadow: none; margin: 4px 0 4px 12px; }
  }
  main { flex: 1; padding: 20px 24px; display: flex; flex-direction: column; gap: 14px; max-width: 1040px; width: 100%; margin: 0 auto; }
  /* the offline browser gets the whole window, so pages have room */
  main.wide { max-width: none; padding: 14px 16px; }
  footer { display: flex; flex-wrap: wrap; gap: 6px 22px; padding: 9px 24px; background: var(--panel2); border-top: 1px solid var(--line);
    font-family: var(--mono); font-size: 12px; color: var(--muted); }
  footer .right { margin-left: auto; }
  .dot { color: var(--ok); }
  .warn-text { color: var(--blue); }
</style>
