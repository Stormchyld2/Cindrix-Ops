<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    modelStatus, listModels, selectModel, libraryStatus, sourcesList, pickFolder,
    librariesList, libraryAnswer, libraryForget, ownContent, openOwnFolder, type OwnFolder,
    type ModelStatus, type ModelChoice, type LibraryStatus, type Sources, type LibraryEntry,
  } from '../lib/api';
  import MyDetails from './MyDetails.svelte';
  import Sharing from './Sharing.svelte';
  import { viewRequest } from '../lib/chat.svelte';
  import { settings, RANGES, SCREENS, type Settings } from '../lib/settings.svelte';

  let { openArchives }: { openArchives: () => void } = $props();

  let own: OwnFolder[] = $state([]);
  let ownNote: Record<string, string> = $state({});
  async function loadOwn() {
    try { own = await ownContent(); } catch { own = []; }
  }
  async function openFolder(key: string) {
    try {
      const p = await openOwnFolder(key);
      ownNote[key] = 'Opened ' + p;
      await loadOwn();
    } catch (e) {
      ownNote[key] = String(e);
    }
  }

  const tabs = ['General', 'Assistant', 'My details', 'Sharing', 'Library', 'Intel', 'Music', 'Maps', 'Plan', 'Your own cards and plans', 'Drive', 'Sources and licenses', 'About'] as const;
  let tab: (typeof tabs)[number] = $state('General');
  // the tabs, in groups, down the left side (the short names stay the same; only what is shown changes)
  const tabGroups: { title: string; tabs: (typeof tabs)[number][] }[] = [
    { title: 'YOU', tabs: ['General', 'My details', 'Sharing'] },
    { title: 'THE SCREENS', tabs: ['Assistant', 'Library', 'Intel', 'Music', 'Maps', 'Plan'] },
    { title: 'THE DRIVE', tabs: ['Drive', 'Your own cards and plans', 'Sources and licenses', 'About'] },
  ];
  const tabLabel: Record<string, string> = { Drive: 'Drives and libraries', Library: 'Shelf and Archives' };
  // the top bar can ask for a tab (Comms > Sharing, System > My details)
  $effect(() => {
    const want = viewRequest.settingsTab;
    if (want && (tabs as readonly string[]).includes(want)) { tab = want as (typeof tabs)[number]; viewRequest.settingsTab = ''; }
  });

  type BoolKey = { [K in keyof Settings]: Settings[K] extends boolean ? K : never }[keyof Settings];
  type NumKey = { [K in keyof Settings]: Settings[K] extends number ? K : never }[keyof Settings];
  type ChoiceKey = 'startScreen' | 'answerLength' | 'answerStyle' | 'searchSpeed' | 'intelStart' | 'intelReader' | 'mapUnits';

  let ms = $state<ModelStatus | null>(null);
  let models: ModelChoice[] = $state([]);
  let lib = $state<LibraryStatus | null>(null);
  let libs: LibraryEntry[] = $state([]);
  let libMsg = $state('');
  let sources: Sources = $state({ groups: [] });
  let switching = $state('');
  let error = $state('');
  let timer: ReturnType<typeof setInterval> | undefined;

  async function refresh() {
    try {
      ms = await modelStatus();
      lib = await libraryStatus();
      libs = await librariesList();
    } catch (e) {
      error = String(e);
    }
  }
  async function libAnswer(path: string, a: 'always' | 'once' | 'never') {
    libMsg = '';
    try { await libraryAnswer(path, a); } catch (e) { libMsg = String(e); }
    await refresh();
  }
  async function libForget(id: string) {
    libMsg = '';
    try { await libraryForget(id); } catch (e) { libMsg = String(e); }
    await refresh();
  }
  const libText: Record<string, string> = {
    own: 'The drive this program is running from. It is always read.',
    always: 'Read every time it is plugged in.',
    once: 'Read just this time, until the program is closed.',
    never: 'Never read.',
    ask: 'Waiting for your answer.',
  };
  onMount(async () => {
    await refresh();
    models = await listModels();
    sources = await sourcesList();
    void loadOwn();
    timer = setInterval(refresh, 2500);
  });
  onDestroy(() => { if (timer) clearInterval(timer); });

  async function use(file: string) {
    error = '';
    switching = file;
    try {
      await selectModel(file);
    } catch (e) {
      error = String(e);
    }
    switching = '';
    await refresh();
  }

  async function addFolder() {
    error = '';
    try {
      const p = await pickFolder();
      if (p && !settings.values.extraFolders.includes(p)) settings.set('extraFolders', [...settings.values.extraFolders, p]);
    } catch (e) {
      error = String(e);
    }
  }
  async function chooseMusicFolder() {
    error = '';
    try {
      const p = await pickFolder();
      if (p) settings.set('musicFolder', [p]);
    } catch (e) {
      error = String(e);
    }
  }
  function removeFolder(p: string) {
    settings.set('extraFolders', settings.values.extraFolders.filter((x) => x !== p));
  }

  const badge = (s: string) => (s === 'in this build' ? 'ok' : s === 'not included' ? 'no' : 'later');
  const screenName = (s: string) => (s === 'Plan' ? 'Plan (inventory, drills, family)' : s);
</script>

{#snippet toggle(key: BoolKey, title: string, help: string)}
  <label class="setting">
    <input type="checkbox" checked={settings.values[key]} onchange={(e) => settings.set(key, e.currentTarget.checked)} />
    <span><b>{title}</b><span class="help">{help}</span></span>
  </label>
{/snippet}

{#snippet slider(key: NumKey, title: string, unit: string, help: string)}
  <label class="setting col">
    <span><b>{title}:</b> {settings.values[key]} {unit}</span>
    <input type="range" min={RANGES[key][0]} max={RANGES[key][1]} step="1" value={settings.values[key]} oninput={(e) => settings.set(key, Number(e.currentTarget.value))} />
    <span class="help">{help}</span>
  </label>
{/snippet}

{#snippet choose(key: ChoiceKey, title: string, help: string, options: [string, string][])}
  <label class="setting col">
    <span><b>{title}</b></span>
    <select class="input narrow" value={settings.values[key]} onchange={(e) => settings.set(key, e.currentTarget.value as Settings[ChoiceKey])}>
      {#each options as [value, label]}<option {value}>{label}</option>{/each}
    </select>
    <span class="help">{help}</span>
  </label>
{/snippet}

{#snippet reset(keys: (keyof Settings)[])}
  <div class="row"><button class="btn" onclick={() => settings.reset(keys)}>Put these back to the usual</button></div>
{/snippet}

<section class="wrap">
  <div>
    <h1>Settings</h1>
    <p class="sub">Change how the program looks and behaves. Changes apply straight away and are kept on the drive, so they travel with it.</p>
  </div>

  <div class="layout">
  <div class="tabs" role="tablist" aria-label="Settings sections" aria-orientation="vertical">
    {#each tabGroups as g}
      <div class="tg">{g.title}</div>
      {#each g.tabs as t}
        <button role="tab" aria-selected={tab === t} class:on={tab === t} onclick={() => (tab = t)}>{tabLabel[t] ?? t}</button>
      {/each}
    {/each}
  </div>
  <div class="body">
  {#if error}<div class="warn">{error}</div>{/if}
  {#if settings.saveError}<div class="warn" role="alert">{settings.saveError}</div>{/if}

  {#if tab === 'General'}
    <div class="panel">
      <div class="label">LOOK AND FEEL</div>
      {@render slider('uiScale', 'Interface size', '%', 'Makes everything bigger or smaller. 100 is the usual size. Go higher if the text is hard to read.')}
      {@render toggle('showStatusBar', 'Show the status bar at the bottom', 'The strip that says which model is loaded and whether the app is using the network.')}
      {@render toggle('showHints', 'Show a hint when the pointer rests on a button', 'A short sentence that says what the button does. Turn it off if the hints get in the way.')}
    </div>
    <div class="panel">
      <div class="label">STARTING UP</div>
      {@render choose('startScreen', 'Open on this screen', 'The screen you see when the program starts. If your edition does not have it, the first one it does have opens.',
        SCREENS.map((s) => [s, screenName(s)] as [string, string]))}
    </div>
    {@render reset(['uiScale', 'showStatusBar', 'showHints', 'startScreen'])}
  {:else if tab === 'Assistant'}
    <div class="panel">
      <div class="label">THE ASSISTANT</div>
      {#if ms}
        <p>
          {#if ms.state === 'ready'}<b>Ready:</b> {ms.model} ({ms.size}).
          {:else if ms.state === 'loading'}<b>Loading</b> {ms.model}…
          {:else}<b>No model running.</b> {ms.message}{/if}
        </p>
        <p class="muted fine">This computer has about {ms.ram_gb.toFixed(1)} GB of memory. Without a model the assistant still finds the right pages and cards; it just cannot write a summary.</p>
      {/if}
    </div>
    <div class="panel">
      <div class="label">MODEL FILES IN THE MODELS FOLDER</div>
      {#if models.length === 0}
        <p class="muted">None found. Copy a Qwen3.5 <code>.gguf</code> file (the 2B file for most computers, 4B if you have 12 GB or more of memory) into the <code>models</code> folder on the drive, then press Look again.</p>
      {:else}
        <ul class="list">
          {#each models as m (m.file)}
            <li>
              <div class="grow"><strong>{m.file}</strong> <span class="muted">· {m.size}</span>{#if m.recommended} <span class="tag">recommended for this computer</span>{/if}</div>
              {#if ms?.model === m.file && ms.state !== 'none'}
                <span class="muted">in use</span>
              {:else}
                <button class="btn" disabled={switching !== ''} onclick={() => use(m.file)}>{switching === m.file ? 'Starting…' : 'Use this one'}</button>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
      <div class="row"><button class="btn" onclick={async () => { models = await listModels(); await refresh(); }}>Look again</button></div>
      <p class="muted fine">The assistant never gives medicine or insulin doses. It points to your doctor's written plan.</p>
    </div>
    <div class="panel">
      <div class="label">WHAT IT MAY LOOK IN</div>
      <p class="muted fine">The emergency cards are always searched. Turn off anything you do not want it to use.</p>
      {@render toggle('searchBooks', 'Your books on the Shelf', 'Books you added or that were found on the drive, once their text has been read.')}
      {@render toggle('searchArchives', 'The archives (Wikipedia, guides, saved websites as archives)', 'Every .zim file on the drive, searched by title, and by full text when the library server is running.')}
      {@render toggle('searchFiles', 'Files and folders on the drive', 'Text, Markdown and web page files anywhere on the drive and in the folders you add below. PDFs are read through the Shelf.')}
      {@render toggle('searchSavedSites', 'Websites you saved with the website saver', 'The plain-text copies made when a site is saved.')}
    </div>
    <div class="panel">
      <div class="label">HOW IT ANSWERS</div>
      {@render slider('answerSources', 'Passages it reads', '', 'How many pieces of text it reads before answering. More can help with hard questions, but it is slower and needs more memory.')}
      {@render choose('answerLength', 'Answer length', 'Short answers finish sooner. Long answers can explain more.', [['short', 'Short'], ['normal', 'Normal'], ['long', 'Long']])}
      {@render choose('answerStyle', 'Answer style', 'Exact sticks closest to the pages. Steady is the everyday choice. Natural sounds more like talking, but may wander a little.', [['exact', 'Exact'], ['steady', 'Steady'], ['natural', 'Natural']])}
      {@render choose('searchSpeed', 'How long to look through files', 'Searching loose files is done fresh for each question. Quick stops after about a second; thorough keeps going for about eight, which a very large drive may need.', [['quick', 'Quick'], ['normal', 'Normal'], ['thorough', 'Thorough']])}
      {@render toggle('showSourceChips', 'Show where each answer came from', 'The small labels under an answer naming the page or file it used.')}
    </div>
    <div class="panel">
      <div class="label">EXTRA FOLDERS TO SEARCH</div>
      <p class="muted fine">The whole drive is already searched. Add folders that are somewhere else, such as a folder of documents on this computer. Only folders that exist are used, and nothing is copied.</p>
      {#if settings.values.extraFolders.length}
        <ul class="list">
          {#each settings.values.extraFolders as f (f)}
            <li><div class="grow path">{f}</div><button class="btn" onclick={() => removeFolder(f)}>Remove</button></li>
          {/each}
        </ul>
      {/if}
      <div class="row"><button class="btn" onclick={addFolder}>Add a folder…</button></div>
    </div>
    {@render reset(['searchBooks', 'searchArchives', 'searchFiles', 'searchSavedSites', 'answerSources', 'answerLength', 'answerStyle', 'searchSpeed', 'showSourceChips', 'extraFolders'])}
  {:else if tab === 'My details'}
    <MyDetails />
  {:else if tab === 'Sharing'}
    <Sharing />
    <div class="panel">
      {@render toggle('shareAuto', 'Switch sharing on by itself when my hotspot starts', 'Other devices can then reach the library as soon as they join your hotspot. They still need the PIN. Turn this off to switch sharing on only by hand.')}
    </div>
  {:else if tab === 'Library'}
    <div class="panel">
      <div class="label">BOOKS AND ARCHIVES</div>
      {@render toggle('autoScanShelf', 'Put books from the drive on the Shelf automatically', 'When you open the Shelf, books in the library, books and manuals folders are added. A book you remove is not put back.')}
      {@render toggle('autoRestartLibrary', 'Pick up new archives by themselves', 'When you copy a .zim file into the library folder, the library server restarts once the copy has finished. Turn off to restart it only with the Look for the drive button.')}
      <div class="row"><button class="btn" onclick={openArchives}>Archives, saving websites and sharing →</button></div>
      <p class="muted fine">Sharing the library with other devices is never turned on by a setting. It is off every time the program starts, and you turn it on in Archives.</p>
    </div>
    {@render reset(['autoScanShelf', 'autoRestartLibrary'])}
  {:else if tab === 'Intel'}
    <div class="panel">
      <div class="label">INTEL, THE OFFLINE INTERNET</div>
      {@render choose('intelStart', 'A new tab opens on', 'The Intel home shows every archive. Or go straight into the first archive on the drive.', [['archives', 'The Intel home'], ['first', 'The first archive']])}
      {@render choose('intelReader', 'Pages are shown by', 'The library server shows every kind of archive, including saved websites. The built-in reader keeps your tabs, page titles and text size following every link you click, but some saved websites stay blank in it. If the library server is not running, the built-in reader is used.', [['server', 'The library server'], ['builtin', 'The built-in reader']])}
      {@render toggle('intelRestoreTabs', 'Remember my open tabs', 'Keep the tabs you had open, and the pages in them, the next time you open the program. Intel itself always opens on its home page, with the cards.')}
      <p class="muted fine">Text size and bookmarks in Intel are kept automatically.</p>
    </div>
    {@render reset(['intelStart', 'intelReader', 'intelRestoreTabs'])}
  {:else if tab === 'Music'}
    <div class="panel">
      <div class="label">MUSIC</div>
      {@render toggle('showMiniPlayer', 'Show the small player at the bottom', 'Lets you pause or skip from any screen while music plays.')}
      {@render toggle('scanMusicOnStart', 'Look through the music folder when the program starts', 'Keeps the list up to date. Turn off if the music folder is huge and you would rather scan it yourself with Look again.')}
      <p class="muted fine">Volume, equalizer and playback order are kept by the player itself.</p>
    </div>
    <div class="panel">
      <div class="label">MUSIC FOLDER</div>
      <p class="muted fine">Music is read from the <b>music</b> folder on the drive. Choose another folder, for example one on this computer, to play from there instead. Nothing is copied or changed. After changing it, open Music and press Look again.</p>
      <div class="row">
        <div class="grow path">{settings.values.musicFolder[0] ?? 'The music folder on the drive'}</div>
        <button class="btn" onclick={chooseMusicFolder}>Choose a folder…</button>
        {#if settings.values.musicFolder.length}<button class="btn" onclick={() => settings.set('musicFolder', [])}>Use the drive's folder</button>{/if}
      </div>
    </div>
    {@render reset(['showMiniPlayer', 'scanMusicOnStart', 'musicFolder'])}
  {:else if tab === 'Maps'}
    <div class="panel">
      <div class="label">MAPS</div>
      {@render choose('mapUnits', 'Distances', 'Used for the scale on the map and for route lengths.', [['imperial', 'Miles and feet'], ['metric', 'Kilometres and metres']])}
    </div>
    {@render reset(['mapUnits'])}
  {:else if tab === 'Plan'}
    <div class="panel">
      <div class="label">WHEN STOCK NEEDS ATTENTION</div>
      <p class="muted fine">These set when an item in your inventory turns amber (check) or red (act now). They apply to the Inventory and What fails screens.</p>
      {@render slider('planActDays', 'Act now when it will run out in under', 'days', 'The red line.')}
      {@render slider('planCheckDays', 'Check when it will run out in under', 'days', 'The amber line. It is never lower than the red one.')}
      {@render slider('planExpiryMonths', 'Check when the use-by date is within', 'months', 'Items already past their date are always act now.')}
    </div>
    {@render reset(['planActDays', 'planCheckDays', 'planExpiryMonths'])}
  {:else if tab === 'Your own cards and plans'}
    <div class="panel">
      <div class="label">ADD YOUR OWN</div>
      <p>Drop a text file in the folder and it shows up by itself: an <strong>emergency card</strong> in the Library, a <strong>plan or “keep it from failing” walkthrough</strong> on the What next page. Each folder opens with a template and a short guide inside, so you can copy the template, change the words and save it with a new name. Nothing is sent anywhere.</p>
    </div>
    {#each own as f (f.key)}
      <div class="panel">
        <div class="label">{f.title.toUpperCase()}</div>
        <p class="muted fine">{f.help}</p>
        <p class="path">{f.path}</p>
        <p class="fine">{f.files} file{f.files === 1 ? '' : 's'} of your own in this folder.</p>
        <div class="row"><button class="btn primary" onclick={() => openFolder(f.key)} title="Opens the folder, and puts the template and the guide in it the first time">Open the folder</button></div>
        {#if ownNote[f.key]}<p class="fine">{ownNote[f.key]}</p>{/if}
        {#if f.problems.length}
          <details class="probs" open>
            <summary>{f.problems.length} file{f.problems.length === 1 ? ' was' : 's were'} left out</summary>
            <p class="muted fine">The rest still works. Fix the file and it appears next time.</p>
            <ul>{#each f.problems as p}<li><code>{p.place}</code>: {p.message}</li>{/each}</ul>
          </details>
        {/if}
      </div>
    {/each}
    <div class="panel">
      <div class="label">ALSO ON THE DRIVE</div>
      <p class="muted fine">Cards and plans are also read from a <code>cards</code> and a <code>plans</code> folder at the top of the drive (next to <code>music</code> and <code>library</code>), so a whole set can be copied in at once. A file with the same name in your own folder replaces the one that came with the program.</p>
    </div>
  {:else if tab === 'Drive'}
    <div class="panel">
      <div class="label">THE DRIVE</div>
      {#if lib?.drive}
        <p><strong>{lib.drive.name}</strong>{lib.drive.edition ? ' · ' + lib.drive.edition + ' edition' : ''}</p>
      {:else}
        <p><strong>No Cindrix drive found.</strong> Using the folder next to the program.</p>
      {/if}
      <p class="path">{lib?.folder}</p>
      <p class="muted fine">Everything you enter (family page, inventory, drill results, your books, these settings) is saved in the <code>user</code> folder here, as plain files you can copy. The program finds the drive by a marker file, whatever drive letter it gets.</p>
      <div class="row"><button class="btn" onclick={openArchives}>Archives and saving websites →</button></div>
    </div>
    <div class="panel">
      <div class="label">LIBRARIES</div>
      <p class="muted fine">A library is a Cindrix drive. The program asks before it reads one, and remembers your answer for that drive whatever drive letter it gets. Archives and loose files from every library you allow are searched together. Your own files and settings are kept on the main one.</p>
      {#if libMsg}<p class="warn">{libMsg}</p>{/if}
      {#if libs.length === 0}
        <p class="muted">No Cindrix drives found yet. Plug one in; this list updates by itself.</p>
      {:else}
        <ul class="liblist">
          {#each libs as l (l.path || l.id)}
            <li>
              <div class="libinfo">
                <strong>{l.name}</strong>{l.edition ? ' · ' + l.edition + ' edition' : ''}
                <div class="muted fine path">{l.present ? l.path : 'Not plugged in now'}</div>
                <div class="fine">{libText[l.state]}</div>
              </div>
              <span class="row">
                {#if l.present && l.state !== 'own'}
                  {#if l.state !== 'always'}<button class="btn" onclick={() => libAnswer(l.path, 'always')}>Always use it</button>{/if}
                  {#if l.state !== 'never'}<button class="btn" onclick={() => libAnswer(l.path, 'never')}>Never use it</button>{/if}
                {/if}
                {#if l.state === 'always' || l.state === 'never' || l.state === 'once'}
                  <button class="btn" onclick={() => libForget(l.id)}>Ask me again</button>
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
    <div class="panel">
      <div class="label">SATELLITE MAPS BY COUNTY</div>
      <p class="muted">Aerial photos for the Maps page come from the free USGS National Map. Put them on the drive once while you have internet: double-click scripts\Get-Imagery.bat (the whole US at a wide view), or see the top of scripts\get-imagery.ps1 for your own area in full detail.</p>
    </div>
    <div class="panel">
      <div class="label">ALL SETTINGS</div>
      <p class="muted fine">Put every setting in the program back to what it ships with. Your books, inventory, family page and music are not touched.</p>
      <div class="row"><button class="btn danger" onclick={() => settings.reset()}>Reset every setting</button></div>
    </div>
  {:else if tab === 'Sources and licenses'}
    {#if sources.note}<p class="muted">{sources.note}</p>{/if}
    {#each sources.groups as g}
      <div class="panel">
        <div class="label">{g.title.toUpperCase()}</div>
        <ul class="list">
          {#each g.items as it}
            <li class="src">
              <div class="grow">
                <strong>{it.name}</strong>
                <div class="muted">{it.note}</div>
              </div>
              <div class="side"><span class="lic">{it.license}</span><span class="st {badge(it.status)}">{it.status}</span></div>
            </li>
          {/each}
        </ul>
      </div>
    {/each}
  {:else}
    <div class="panel">
      <div class="label">ABOUT</div>
      <p><strong>Cindrix Ops Survival Library</strong> · Carry the library. Keep the fire.</p>
      <p class="muted">A reference tool, not a doctor, lawyer or rescuer. In an emergency, call 911. Content marked DRAFT has not been reviewed by a clinician. The assistant can be wrong: check anything that matters against the source it names.</p>
      <p class="muted">The program works without internet and only talks to itself on this device. The one exception is <b>Save a website</b> in Archives, which uses the internet only while you run it. Sharing the library with other devices is off unless you turn it on.</p>
      <p class="muted fine">The program is free software (GPL-3.0-or-later). The full license texts are in the LICENSES folder.</p>
    </div>
  {/if}
  </div>
  </div>
</section>

<style>
  .wrap { display: flex; flex-direction: column; gap: 14px; }
  .liblist { list-style: none; padding: 0; margin: 10px 0 0; display: flex; flex-direction: column; gap: 10px; }
  .liblist li { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 14px; padding: 10px 0; border-top: 1px solid var(--line); }
  .libinfo { flex: 1; min-width: 220px; }
  .layout { display: grid; grid-template-columns: 230px 1fr; gap: 18px; align-items: start; }
  .body { display: flex; flex-direction: column; gap: 14px; min-width: 0; }
  .tabs { display: flex; flex-direction: column; gap: 4px; position: sticky; top: 92px; padding: 10px; border: 1px solid var(--line); border-left: 3px solid var(--cat-system); border-radius: 10px; background: var(--panel); }
  .tg { font-family: var(--mono); font-size: 10.5px; letter-spacing: .16em; color: var(--muted); padding: 10px 8px 4px; }
  .tg:first-child { padding-top: 2px; }
  .tabs button { text-align: left; padding: 8px 12px; border-radius: 6px; border: 1px solid transparent; background: transparent; color: var(--text); font: inherit; cursor: pointer; }
  .tabs button:hover { border-color: var(--line2); }
  .tabs button.on { background: var(--blue-soft); border-color: var(--blue-line); color: var(--blue); }
  @media (max-width: 760px) {
    .layout { grid-template-columns: 1fr; }
    .tabs { position: static; flex-direction: row; flex-wrap: wrap; }
    .tg { display: none; }
  }
  .list { list-style: none; padding: 0; margin: 8px 0; display: flex; flex-direction: column; }
  .list li { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 14px; padding: 10px 0; border-top: 1px solid var(--line); }
  .grow { flex: 1; min-width: 220px; }
  .tag { font-size: 12px; color: var(--blue); }
  .fine { font-size: 12px; }
  .path { font-family: var(--mono); word-break: break-all; margin: 4px 0; }
  code { font-family: var(--mono); }
  .setting { display: flex; gap: 12px; align-items: flex-start; padding: 12px 0; border-top: 1px solid var(--line); cursor: pointer; }
  .setting:first-of-type { border-top: 0; }
  .setting.col { flex-direction: column; gap: 6px; align-items: stretch; }
  .setting input[type='checkbox'] { margin-top: 4px; flex: none; width: 18px; height: 18px; }
  .setting input[type='range'] { width: min(420px, 100%); accent-color: var(--blue); }
  .setting .help { display: block; margin-top: 2px; font-size: 12px; color: var(--muted); font-weight: 400; }
  .narrow { max-width: 360px; }
  .side { display: flex; flex-direction: column; align-items: flex-end; gap: 4px; font-size: 12px; }
  .lic { font-family: var(--mono); }
  .st { padding: 1px 8px; border-radius: 999px; border: 1px solid var(--line2); }
  .st.ok { color: #7bd88f; border-color: #2f6b3d; }
  .st.later { color: var(--blue); border-color: #2c5b86; }
  .st.no { color: #f08a7a; border-color: #8a3b30; }
</style>
