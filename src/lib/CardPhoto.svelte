<script lang="ts">
  // The picture a card names in its header (`image: morel.jpg`). Shows nothing at all when the card names
  // no picture or the file is not there, so a card without a photo looks exactly as it always did.
  import { cardImage } from './api';

  let { file = '', title = '' }: { file?: string; title?: string } = $props();
  let src: string | null = $state(null);

  $effect(() => {
    const f = file;
    src = null;
    if (!f) return;
    let live = true;
    cardImage(f).then((d) => { if (live) src = d; });
    return () => { live = false; };
  });
</script>

{#if src}
  <figure class="cardphoto"><img {src} alt={title ? `Photo: ${title}` : 'Photo for this card'} /></figure>
{/if}

<style>
  .cardphoto { margin: 8px 0 12px; }
  .cardphoto img { display: block; max-width: 100%; max-height: 340px; object-fit: contain; border-radius: 10px; border: 1px solid var(--line); background: #000; }
  @media print { .cardphoto img { max-height: 240px; border-color: #999; } }
</style>
