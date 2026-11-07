<script lang="ts">
  // Bible reader for the Canonleaf add-on: one chapter at a time from the owner's own Bible text.
  // Read-only. Ask sources open it at the right verse (a link like "canonleaf:JHN.3.16").
  import { onMount } from 'svelte';
  import { canonleafBooks, canonleafChapter, type BibleBook, type BibleChapter } from '../lib/api';
  import { bibleRequest } from '../lib/chat.svelte';

  let books: BibleBook[] = $state([]);
  let book = $state('JHN');
  let chapter = $state(1);
  let data: BibleChapter | null = $state(null);
  let mark = $state(0);
  let error = $state('');
  let loading = $state(false);

  async function load() {
    error = '';
    loading = true;
    try {
      data = await canonleafChapter(book, chapter);
    } catch (e) {
      data = null;
      error = String(e);
    }
    loading = false;
    if (mark) {
      queueMicrotask(() => document.getElementById('v' + mark)?.scrollIntoView({ block: 'center' }));
    }
  }
  function go(ref: string) {
    // "JHN.3.16"
    const [b, c, v] = ref.split('.');
    if (!b || !Number(c)) return;
    book = b;
    chapter = Number(c);
    mark = Number(v) || 0;
    load();
  }
  onMount(async () => {
    try { books = await canonleafBooks(); } catch (e) { error = String(e); }
    if (bibleRequest.ref) { go(bibleRequest.ref); bibleRequest.ref = ''; } else { load(); }
  });
  $effect(() => {
    // an Ask source opened while this screen was already showing
    if (bibleRequest.ref) { go(bibleRequest.ref); bibleRequest.ref = ''; }
  });
  function pick(code: string) { book = code; chapter = 1; mark = 0; load(); }
  function step(d: number) {
    const n = chapter + d;
    if (n < 1 || (data && n > data.chapters)) return;
    chapter = n;
    mark = 0;
    load();
  }
</script>

<section>
  <h1>Bible</h1>
  <p class="sub">Read from your Canonleaf Bible text. {#if data}{data.edition}.{/if}</p>
  <div class="row">
    <select aria-label="Book" title="Choose a book" value={book} onchange={(e) => pick(e.currentTarget.value)}>
      {#each books as b}<option value={b.code}>{b.name}</option>{/each}
    </select>
    <button class="btn" title="Previous chapter" disabled={chapter <= 1 || loading} onclick={() => step(-1)}>Back</button>
    <span class="chap">{data?.name ?? ''} {chapter}</span>
    <button class="btn" title="Next chapter" disabled={loading || (data !== null && chapter >= data.chapters)} onclick={() => step(1)}>Next</button>
  </div>
  {#if error}
    <p class="error">{error}</p>
  {:else if data}
    <div class="text">
      {#each data.verses as v}
        <p id={'v' + v.number} class:hit={Number(v.number) === mark}><sup>{v.number}</sup> {v.text}</p>
      {/each}
    </div>
  {/if}
</section>

<style>
  .row { display: flex; gap: 8px; align-items: center; margin: 10px 0; flex-wrap: wrap; }
  .chap { min-width: 9rem; text-align: center; font-weight: 600; }
  .text { max-width: 46rem; line-height: 1.6; }
  .text p { margin: 0 0 6px; padding: 2px 6px; border-radius: 6px; }
  .hit { background: color-mix(in srgb, var(--blue) 22%, transparent); }
  sup { color: var(--blue); margin-right: 3px; }
  .error { color: var(--warn); }
</style>
