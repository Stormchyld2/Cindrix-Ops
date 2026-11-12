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
  import Alerts from './lib/Alerts.svelte';
  import Maps from './views/Maps.svelte';
  import Shelf from './views/Shelf.svelte';
  import Archives from './views/Archives.svelte';
  import Intel from './views/Intel.svelte';
  import Drills from './views/Drills.svelte';
  import Binder from './views/Binder.svelte';
  import Settings from './views/Settings.svelte';
  import Dashboard from './views/Dashboard.svelte';
  import SecurityOps from './views/SecurityOps.svelte';
  import Plans from './views/Plans.svelte';
  import SeedVault from './views/SeedVault.svelte';
  import Music from './views/Music.svelte';
  import Guides from './views/Guides.svelte';
  import DrugLabels from './views/DrugLabels.svelte';
  import Journal from './views/Journal.svelte';
  import Photos from './views/Photos.svelte';
  import CommsPlan from './views/CommsPlan.svelte';
  import MiniPlayer from './lib/MiniPlayer.svelte';
  import Splash from './lib/Splash.svelte';
  import { onMount } from 'svelte';
  import { intelRequest, sourceRequest, viewRequest } from './lib/chat.svelte';
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
  type Group = 'Dashboard' | 'Ask' | 'Library' | 'Intel' | 'Academy' | 'Maps' | 'Music' | 'Plan' | 'Inventory' | 'Settings' | 'Corner' | 'Guides' | 'Journal' | 'Photos' | 'CommsPlan' | 'Todo' | 'Calendar' | 'Projects' | 'DrugLabels' | 'Security' | 'Soon';
  const allGroups: Group[] = ['Dashboard', 'Ask', 'Library', 'Intel', 'Academy', 'Maps', 'Music', 'Plan', 'Inventory', 'Settings', 'Corner', 'Journal', 'Photos', 'CommsPlan', 'Todo', 'Calendar', 'Projects', 'DrugLabels', 'Security'];
  // Each product switches parts of the program on or off (core/src/product.rs).
  const groups = $derived(
    allGroups.filter((g) => {
      if (!product) return true;
      if (g === 'Ask') return product.assistant;
      if (g === 'Library' || g === 'Intel' || g === 'Corner') return product.library;
      if (g === 'DrugLabels') return product.library && !inShare;
      if (g === 'Maps') return product.maps;
      if (g === 'Music') return product.music;
      if (g === 'Security') return product.security && !inShare;
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
  // sections of the left nav that are folded out; the one holding the current screen opens by itself
  let folded: string[] = $state([]);
  function toggleSection(id: string) { folded = folded.includes(id) ? folded.filter((x) => x !== id) : [...folded, id]; }
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
        { label: 'Archives', tab: 'Archives' }, { label: 'Emergency cards', tab: 'Cards' },
        { label: 'Home remedies', tab: 'guide:home-remedies' }, { label: 'Drug labels', tab: 'DrugLabels', note: 'FDA', show: !inShare } ] },
      { id: 'Academy', cat: 'academy', show: true, entries: [
        { label: 'Courses', tab: 'Academy' }, { label: 'Training sign-off', tab: 'training' } ] },
      { id: 'Agro', cat: 'agro', show: plan_, entries: [
        { label: 'Crop guides', tab: 'guide:crop-guides' }, { label: 'Planting calendar', tab: 'guide:planting-calendar' },
        { label: 'Food preservation', tab: 'guide:food-preservation' }, { label: 'Livestock', tab: 'guide:livestock' },
        { label: 'Soil and water', tab: 'guide:soil-and-water' }, { label: '', tab: '', rule: true },
        { label: 'Seed vault', tab: 'Seed vault' } ] },
      { id: 'Maps', cat: 'maps', tab: 'Maps', show: p ? p.maps : true },
      { id: 'Security', cat: 'security', tab: 'Security', show: !inShare && (p ? p.security : true) },
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
      case 'Library': case 'Intel': case 'DrugLabels': return 'Intel';
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
    if (t.entries) folded = folded.includes(t.id) ? [] : [t.id];
    else if (t.tab) pick(t.tab);
  }
  /** Is this entry the screen on show? Used to light it in the left nav. */
  function entryOn(top: Top, e: Entry): boolean {
    if (activeTop !== top.id) return false;
    if (e.tab === group) return true;
    if (e.tab === plan && group === 'Plan') return true;
    if (e.tab === 'guide:' + guideSection && group === 'Guides') return true;
    if (e.tab === 'settings:Sharing') return group === 'Settings' && settingsFromComms;
    if (e.tab.startsWith('settings:') && group === 'Settings') return !settingsFromComms;
    if (e.tab === 'training') return false;
    if (group === 'Library' && e.tab === lib) return true;
    return false;
  }
</script>

<svelte:window onkeydown={(e) => { if (e.key === 'Escape') barOpen = false; }} />

{#if showSplash}<Splash ready={polled && settings.loaded} />{/if}

<div class="shell">
  <div class="topbar no-print">
    <button class="burger" aria-label="Menu" aria-expanded={barOpen} onclick={(e) => { e.stopPropagation(); barOpen = !barOpen; }}>&#9776;</button>
    <img src="/cindrix-logo-full.png" alt="CindrixOps" />
  </div>
  {#if barOpen}<button class="scrim no-print" aria-label="Close the menu" onclick={() => (barOpen = false)}></button>{/if}
  <aside class="no-print" class:show={barOpen}>
    <img class="logo" src="/cindrix-logo-full.png" alt="CindrixOps: Survival, Preparedness, Self-Reliance" />
    <nav aria-label="Sections">
      {#each tops as t (t.id)}
        {#if t.show}
          {@const open = !!t.entries && (folded.includes(t.id) || activeTop === t.id)}
          <div class="item" class:on={activeTop === t.id} style={`--c: var(--cat-${t.cat})`}>
            <button class="top" aria-label={t.id} onclick={(e) => { e.stopPropagation(); topClick(t); }} aria-current={activeTop === t.id && !t.entries ? 'page' : undefined} aria-expanded={t.entries ? open : undefined}>
              <span class="dia"></span>{t.id}{#if t.entries}<span class="caret">{open ? '▾' : '▸'}</span>{/if}
            </button>
            {#if t.entries && open}
              <div class="subs" role="menu">
                {#each t.entries as e}
                  {#if e.rule}{#if e.show !== false}<hr />{/if}
                  {:else if e.show !== false}
                    <button role="menuitem" class:on={entryOn(t, e)} aria-label={e.label} onclick={(ev) => { ev.stopPropagation(); pick(e.tab); }}>{e.label}{#if e.note}<small>{e.note}</small>{/if}</button>
                  {/if}
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      {/each}
    </nav>
  </aside>
  <div class="content">

  {#if !inShare && product?.security}
    <Alerts go={goTo} />
  {/if}

  {#if libNote}
    <div class="warn libnote" role="status">{libNote} <button class="btn" onclick={() => (libNote = '')}>OK</button></div>
  {/if}

  <main class:wide={group === 'Intel' || group === 'Calendar' || group === 'Ask' || group === 'Academy'}>
    {#if group === 'Dashboard'}
      <Dashboard go={goTo} />
    {:else if group === 'Security'}
      <SecurityOps />
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
    {:else if group === 'DrugLabels'}
      <DrugLabels />
    {:else if group === 'Journal'}
      <Journal />
    {:else if group === 'Photos'}
      <Photos />
    {:else if group === 'CommsPlan'}
      <CommsPlan go={goTo} />
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
        <Drills go={goTo} />
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
      <Settings {openArchives} go={goTo} />
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
</div>

<style>
  .dlg { padding: 18px; display: flex; flex-direction: column; gap: 10px; }
  .dlg h2 { margin: 0; }
  .dlg p { margin: 0; }
  .dlg .path { font-family: var(--mono); font-size: 12px; word-break: break-all; color: var(--muted, #9aa7b4); }
  .dlg .fine { font-size: 12px; }
  .dlg .end { justify-content: flex-end; }
  .libnote { margin: 8px 16px 0; display: flex; gap: 12px; align-items: center; justify-content: space-between; }
  .shell { overflow-x: clip; min-height: 100vh; display: grid; grid-template-columns: 230px minmax(0, 1fr); }
  .content { display: flex; flex-direction: column; min-width: 0; min-height: 100vh; }
  aside { position: sticky; top: 0; height: 100vh; overflow-y: auto; background: #101113; border-right: 1px solid var(--line); padding: 14px 10px; z-index: 50; }
  .logo { display: block; width: 130px; margin: 0 auto 12px; }
  nav { display: flex; flex-direction: column; gap: 2px; }
  .item > .top { display: flex; width: 100%; gap: 10px; align-items: center; padding: 9px 12px; border-radius: 8px; border: 0; background: transparent; color: var(--muted); font-size: 14px; text-align: left; }
  .item > .top:hover { color: var(--text); background: #ffffff08; }
  .dia { width: 8px; height: 8px; transform: rotate(45deg); background: var(--c, var(--muted)); flex: none; }
  .item.on > .top { background: var(--blue-soft); color: var(--blue); font-weight: 600; }
  .caret { margin-left: auto; font-size: 10px; opacity: .7; }
  .subs { margin: 2px 0 6px 18px; padding-left: 8px; border-left: 1px solid var(--line); display: flex; flex-direction: column; }
  .subs button { display: flex; justify-content: space-between; gap: 8px; padding: 6px 10px; border: 0; border-left: 2px solid transparent; background: transparent; color: var(--muted); font-size: 13px; text-align: left; border-radius: 0 6px 6px 0; }
  .subs button:hover { color: var(--text); background: #ffffff08; }
  .subs button.on { color: var(--blue); border-left-color: var(--blue); }
  .subs small { color: #6d727a; font-size: 10px; letter-spacing: .06em; text-transform: uppercase; align-self: center; }
  .subs hr { border: 0; border-top: 1px solid var(--line); margin: 6px 0; }
  .topbar, .scrim { display: none; }
  @media (max-width: 900px) {
    .shell { display: flex; flex-direction: column; }
    .topbar { display: flex; align-items: center; gap: 12px; position: sticky; top: 0; z-index: 40; padding: 10px 14px; background: #101113; border-bottom: 1px solid var(--line); }
    .topbar img { height: 34px; }
    .burger { width: 40px; height: 40px; font-size: 22px; border: 1px solid var(--line); border-radius: 10px; background: transparent; color: var(--text); }
    aside { position: fixed; left: 0; top: 0; bottom: 0; width: min(300px, 82vw); height: auto; transform: translateX(-102%); transition: transform .18s ease; box-shadow: 10px 0 30px #000c; }
    aside.show { transform: none; }
    .scrim { display: block; position: fixed; inset: 0; z-index: 45; background: #0009; border: 0; }
  }
  main { flex: 1; padding: 20px 24px; display: flex; flex-direction: column; gap: 14px; max-width: 1560px; width: 100%; margin: 0 auto; }
  /* the offline browser gets the whole window, so pages have room */
  main.wide { max-width: none; padding: 14px 16px; }
  main.wide:has(> .ac) { padding: 0; gap: 0; }
  footer { display: flex; flex-wrap: wrap; gap: 6px 22px; padding: 9px 24px; margin-top: auto; background: var(--panel2); border-top: 1px solid var(--line);
    font-family: var(--mono); font-size: 12px; color: var(--muted); }
  footer .right { margin-left: auto; }
  .dot { color: var(--ok); }
  .warn-text { color: var(--blue); }
</style>
