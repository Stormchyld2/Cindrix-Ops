<script lang="ts">
  // Family pictures: a shared album that other devices on the hotspot can look at, and each person's own album (this computer only).
  import { onMount, onDestroy } from 'svelte';
  import { photosList, photoRead, photoAdd, photoDelete, whoami, inShare, type Photo } from '../lib/api';
  import { who } from '../lib/person.svelte';
  import PersonPick from '../lib/PersonPick.svelte';

  type Scope = 'shared' | 'mine';
  let scope: Scope = $state('shared');
  let photos: Photo[] = $state([]);
  let urls: Record<string, string> = $state({});
  let note = $state('');
  let busy = $state(false);
  let dragging = $state(false);
  let viewing: Photo | null = $state(null);
  let removing: Photo | null = $state(null);
  /** On another device: the family member this browser joined as, if it used an invite. */
  let me: string | null = $state(null);
  const canWrite = $derived(!inShare || !!me);
  let fileInput: HTMLInputElement | undefined = $state();
  const mime = (n: string) => ({ jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png', gif: 'image/gif', webp: 'image/webp' } as Record<string, string>)[n.split('.').pop()?.toLowerCase() ?? ''] ?? 'image/jpeg';

  function forget() {
    for (const u of Object.values(urls)) URL.revokeObjectURL(u);
    urls = {};
  }
  async function load() {
    forget();
    note = '';
    try { photos = await photosList(scope, who.name); } catch (e) { note = String(e); photos = []; }
  }
  async function ensure(p: Photo) {
    if (urls[p.name]) return;
    try {
      const b = await photoRead(scope, who.name, p.name);
      urls[p.name] = URL.createObjectURL(new Blob([b as unknown as BlobPart], { type: mime(p.name) }));
    } catch { /* the thumbnail stays empty */ }
  }
  /** Load a picture only when it scrolls into view, so a big album does not fill memory at once. */
  function visible(node: HTMLElement, p: Photo) {
    const io = new IntersectionObserver((es) => { if (es.some((e) => e.isIntersecting)) { void ensure(p); io.disconnect(); } }, { rootMargin: '200px' });
    io.observe(node);
    return { destroy: () => io.disconnect() };
  }
  async function addFiles(files: FileList | File[] | null) {
    if (!files || busy || !canWrite) return;
    busy = true;
    note = '';
    const failed: string[] = [];
    let n = 0;
    for (const f of Array.from(files)) {
      try { await photoAdd(f, scope, who.name); n++; } catch (e) { failed.push(`${f.name}: ${String(e)}`); }
    }
    await load();
    note = [n ? `Added ${n} picture${n === 1 ? '' : 's'}.` : '', ...failed].filter(Boolean).join(' ');
    busy = false;
    if (fileInput) fileInput.value = '';
  }
  async function confirmRemove() {
    if (!removing) return;
    try { await photoDelete(scope, who.name, removing.name); } catch (e) { note = String(e); }
    removing = null;
    viewing = null;
    await load();
  }
  function pickScope(s: Scope) { scope = s; void load(); }
  onMount(async () => { me = await whoami(); await load(); });
  onDestroy(forget);
</script>

<section
  class:drag={dragging}
  aria-label="Photos"
  ondragover={(e) => { if (canWrite) { e.preventDefault(); dragging = true; } }}
  ondragleave={() => (dragging = false)}
  ondrop={(e) => { e.preventDefault(); dragging = false; void addFiles(e.dataTransfer?.files ?? null); }}
>
  <div class="top">
    <div>
      <h1>Photos</h1>
      <p class="sub">{inShare ? (me ? `Joined as ${me}. You can add pictures to the shared album or your own.` : 'The shared family album.') : 'The shared album can be seen by other devices on your hotspot. Each person’s own album stays on this computer.'}</p>
    </div>
    {#if canWrite}
      <div class="row">
        <input bind:this={fileInput} id="add-photo" class="sr" type="file" accept="image/jpeg,image/png,image/gif,image/webp" multiple onchange={(e) => addFiles(e.currentTarget.files)} />
        <label class="btn primary add" for="add-photo" aria-label="Add pictures">{busy ? 'Adding…' : '+ Add pictures'}</label>
      </div>
    {/if}
  </div>

  {#if canWrite}
    <div class="row tabs">
      <button class="pill" class:on={scope === 'shared'} onclick={() => pickScope('shared')}>Shared album</button>
      <button class="pill" class:on={scope === 'mine'} onclick={() => pickScope('mine')}>{inShare ? 'My album' : 'Own album'}</button>
      {#if scope === 'mine' && !inShare}<PersonPick onchange={load} />{/if}
    </div>
  {/if}
  {#if note}<p class="notice" role="status">{note}</p>{/if}

  {#if photos.length === 0}
    <div class="panel empty">
      <strong>No pictures here yet.</strong>
      <p class="muted">{inShare && !me ? 'Nothing has been shared yet.' : inShare ? 'Use “Add pictures” to put some here.' : 'Drag pictures here, or use “Add pictures”. JPEG, PNG, GIF and WebP pictures work. They are copied onto this drive, and never leave it.'}</p>
    </div>
  {/if}

  <div class="grid">
    {#each photos as p (p.name)}
      <button class="ph" use:visible={p} onclick={() => { viewing = p; void ensure(p); }} aria-label={'Open ' + p.name}>
        {#if urls[p.name]}<img src={urls[p.name]} alt={p.name} />{:else}<span class="muted small">{p.name}</span>{/if}
      </button>
    {/each}
  </div>
</section>

{#if viewing}
  {@const v = viewing}
  <dialog open aria-label={v.name}>
    <div class="dlg">
      <h2>{v.name}</h2>
      {#if urls[v.name]}<img class="big" src={urls[v.name]} alt={v.name} />{:else}<p class="muted">Opening…</p>{/if}
      <div class="row end">
        {#if canWrite}<button class="btn danger" onclick={() => (removing = v)}>Remove</button>{/if}
        <button class="btn" onclick={() => (viewing = null)}>Close</button>
      </div>
    </div>
  </dialog>
{/if}

{#if removing}
  <dialog open aria-label="Remove picture">
    <div class="dlg">
      <h2>Remove this picture?</h2>
      <p>“{removing.name}” will be deleted from this drive.</p>
      <div class="row end">
        <button class="btn" onclick={() => (removing = null)}>Cancel</button>
        <button class="btn danger" onclick={confirmRemove}>Remove</button>
      </div>
    </div>
  </dialog>
{/if}

<svelte:window onkeydown={(e) => { if (e.key === 'Escape') { removing = null; viewing = null; } }} />

<style>
  .top { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 12px; align-items: flex-start; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); }
  .add { cursor: pointer; }
  .tabs { margin: 10px 0; }
  .pill { padding: 6px 14px; border-radius: 999px; border: 1px solid var(--line2); background: transparent; color: var(--muted); font-family: var(--mono); font-size: 12px; text-transform: uppercase; letter-spacing: .08em; }
  .pill.on { color: var(--blue); border-color: var(--blue-line); background: var(--blue-soft); }
  .drag { outline: 2px dashed var(--blue-line); outline-offset: 6px; }
  .empty { margin-top: 12px; max-width: 640px; border-left: 3px solid var(--cat-morale); }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px; margin-top: 12px; }
  .ph { aspect-ratio: 1; padding: 0; overflow: hidden; border: 1px solid var(--line); border-radius: 8px; background: var(--panel); display: flex; align-items: center; justify-content: center; text-align: center; }
  .ph:hover { border-color: var(--blue-line); }
  .ph img { width: 100%; height: 100%; object-fit: cover; }
  .dlg { padding: 18px; display: flex; flex-direction: column; gap: 10px; }
  .dlg h2 { margin: 0; word-break: break-all; }
  .dlg p { margin: 0; }
  .dlg .end { justify-content: flex-end; }
  .big { max-width: 100%; max-height: 62vh; object-fit: contain; border-radius: 6px; }
  dialog { width: min(900px, 94vw); }
  .small { font-size: 12px; padding: 6px; word-break: break-all; }
  .notice { margin: 8px 0 0; }
</style>
