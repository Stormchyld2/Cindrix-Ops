<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { libraryStatus, libraryShare, type LibraryStatus } from '../lib/api';

  let st: LibraryStatus = $state({ archives: [], ready: false, url: null, shared: false, lan_url: null });
  let timer: ReturnType<typeof setInterval> | undefined;
  let working = $state(false);
  let confirmShare = $state(false);

  async function refresh() {
    st = await libraryStatus();
  }
  onMount(() => { refresh(); timer = setInterval(refresh, 2500); });
  onDestroy(() => { if (timer) clearInterval(timer); });

  async function setShare(on: boolean) {
    working = true;
    confirmShare = false;
    await libraryShare(on);
    // the library server restarts in the background; wait for it to come back
    for (let i = 0; i < 40; i++) {
      await new Promise((r) => setTimeout(r, 750));
      await refresh();
      if (st.ready && st.shared === on) break;
    }
    working = false;
  }
</script>

<section class="wrap">
  <div class="top">
    <div>
      <h1>Archives</h1>
      <p class="sub">The big offline library: Wikipedia, medical guides, books and more, stored on this drive as .zim files. Read them here, no internet needed.</p>
    </div>
  </div>

  {#if st.ready && st.url}
    <div class="frame panel">
      <iframe title="Offline library" src={st.url} sandbox="allow-scripts allow-same-origin allow-forms allow-popups"></iframe>
    </div>
  {:else if st.archives.length > 0}
    <div class="panel"><strong>Starting the library…</strong><p class="muted">{st.archives.length} archive{st.archives.length === 1 ? '' : 's'} found. This can take a few seconds.</p></div>
  {:else}
    <div class="panel">
      <strong>No archives on this drive yet.</strong>
      <p class="muted">Put .zim files in the <code>library</code> folder next to the program, then restart. You can download them from the Kiwix library while you have internet, or run the drive build script. Only add archives whose license lets you keep a copy.</p>
    </div>
  {/if}

  {#if st.ready}
    <div class="panel share">
      <div class="head">
        <div>
          <div class="label">SHARE WITH OTHER DEVICES</div>
          <strong>{st.shared ? 'Sharing is on' : 'Sharing is off'}</strong>
        </div>
        {#if st.shared}
          <button class="btn" onclick={() => setShare(false)} disabled={working}>Turn off</button>
        {:else}
          <button class="btn primary" onclick={() => (confirmShare = true)} disabled={working}>Turn on</button>
        {/if}
      </div>
      {#if st.shared}
        <p>Phones, tablets and laptops on the same Wi-Fi or hotspot can read this library. Open this address on the other device:</p>
        <p class="addr">{st.lan_url ?? 'Your computer’s address could not be found. Look in the Wi-Fi settings for its IP address and use it with port 8787.'}</p>
        <p class="muted fine">They can only read. They cannot change anything, and cannot see your inventory, family page or books. Anyone on this network can open the library while sharing is on.</p>
      {:else}
        <p class="muted">Off, the library can only be opened on this computer. Turn it on to use this drive as a small off-grid library server for the family: put it on a hotspot or router with no internet, and phones can browse it.</p>
      {/if}
      {#if working}<p class="muted fine">Restarting the library…</p>{/if}
    </div>
  {/if}

  {#if confirmShare}
    <dialog open aria-label="Share the library">
      <div class="dlg">
        <h2>Share the library on this network?</h2>
        <p>Other devices on the same Wi-Fi will be able to read the library while this is on. Windows may ask to allow the library program through the firewall: choose private networks only. Do not turn this on in public places.</p>
        <p class="muted">Everything else on this drive stays private and the assistant does not move onto the network.</p>
        <div class="row end">
          <button class="btn" onclick={() => (confirmShare = false)}>Cancel</button>
          <button class="btn primary" onclick={() => setShare(true)}>Share</button>
        </div>
      </div>
    </dialog>
  {/if}
</section>

<style>
  .wrap { display: flex; flex-direction: column; gap: 14px; flex: 1; }
  .frame { padding: 0; overflow: hidden; height: min(70vh, 720px); min-height: 360px; }
  iframe { width: 100%; height: 100%; border: 0; background: #fff; }
  .head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 10px; }
  .share p { margin: 8px 0 0; }
  .addr { font-family: var(--mono); font-size: 16px; color: var(--blue); word-break: break-all; }
  .fine { font-size: 12px; }
  code { font-family: var(--mono); }
  .dlg { padding: 18px; display: flex; flex-direction: column; gap: 10px; }
  .end { justify-content: flex-end; }
</style>
