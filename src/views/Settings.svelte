<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { modelStatus, listModels, selectModel, libraryStatus, sourcesList, type ModelStatus, type ModelChoice, type LibraryStatus, type Sources } from '../lib/api';

  let { openArchives }: { openArchives: () => void } = $props();

  const tabs = ['Assistant', 'Drive', 'Sources and licenses', 'About'] as const;
  let tab: (typeof tabs)[number] = $state('Assistant');

  let ms: ModelStatus | null = $state(null);
  let models: ModelChoice[] = $state([]);
  let lib: LibraryStatus | null = $state(null);
  let sources: Sources = $state({ groups: [] });
  let switching = $state('');
  let error = $state('');
  let timer: ReturnType<typeof setInterval> | undefined;

  async function refresh() {
    try {
      ms = await modelStatus();
      lib = await libraryStatus();
    } catch (e) {
      error = String(e);
    }
  }
  onMount(async () => {
    await refresh();
    models = await listModels();
    sources = await sourcesList();
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

  const badge = (s: string) => (s === 'in this build' ? 'ok' : s === 'not included' ? 'no' : 'later');
</script>

<section class="wrap">
  <div>
    <h1>Settings</h1>
    <p class="sub">The assistant, the drive, where everything came from, and how this program is meant to be used.</p>
  </div>

  <div class="tabs" role="tablist">
    {#each tabs as t}
      <button role="tab" aria-selected={tab === t} class:on={tab === t} onclick={() => (tab = t)}>{t}</button>
    {/each}
  </div>
  {#if error}<div class="warn">{error}</div>{/if}

  {#if tab === 'Assistant'}
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
  {:else if tab === 'Drive'}
    <div class="panel">
      <div class="label">THE DRIVE</div>
      {#if lib?.drive}
        <p><strong>{lib.drive.name}</strong>{lib.drive.edition ? ' · ' + lib.drive.edition + ' edition' : ''}</p>
      {:else}
        <p><strong>No Cindrix drive found.</strong> Using the folder next to the program.</p>
      {/if}
      <p class="path">{lib?.folder}</p>
      <p class="muted fine">Everything you enter (family page, inventory, drill results, your books) is saved in the <code>user</code> folder here, as plain files you can copy. The program finds the drive by a marker file, whatever drive letter it gets.</p>
      <div class="row"><button class="btn" onclick={openArchives}>Archives and saving websites →</button></div>
    </div>
    <div class="panel">
      <div class="label">SATELLITE MAPS BY COUNTY</div>
      <p class="muted">Optional aerial photos for the Maps page (US counties, from the USGS NAIP program), downloaded once while you have internet. Arrives with Maps.</p>
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
</section>

<style>
  .wrap { display: flex; flex-direction: column; gap: 14px; }
  .tabs { display: flex; flex-wrap: wrap; gap: 6px; }
  .tabs button { padding: 8px 14px; border-radius: 8px; border: 1px solid var(--line2); background: transparent; color: var(--text); font: inherit; cursor: pointer; }
  .tabs button.on { background: var(--panel2); border-color: var(--blue); color: var(--blue); }
  .list { list-style: none; padding: 0; margin: 8px 0; display: flex; flex-direction: column; }
  .list li { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 14px; padding: 10px 0; border-top: 1px solid var(--line); }
  .grow { flex: 1; min-width: 220px; }
  .tag { font-size: 12px; color: var(--blue); }
  .fine { font-size: 12px; }
  .path { font-family: var(--mono); word-break: break-all; margin: 4px 0; }
  code { font-family: var(--mono); }
  .side { display: flex; flex-direction: column; align-items: flex-end; gap: 4px; font-size: 12px; }
  .lic { font-family: var(--mono); }
  .st { padding: 1px 8px; border-radius: 999px; border: 1px solid var(--line2); }
  .st.ok { color: #7bd88f; border-color: #2f6b3d; }
  .st.later { color: var(--blue); border-color: #2c5b86; }
  .st.no { color: #f08a7a; border-color: #8a3b30; }
</style>
