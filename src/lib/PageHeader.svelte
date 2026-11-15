<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { productInfo } from './api';
  import Icon from './Icon.svelte';
  // The banner at the top of every main screen, as in the mockups: small blue eyebrow with the product name, a big silver
  // title (optionally with a line picture on the left and three slashes after it), a line of words, the buttons on the right,
  // and the painted moon, wolf and mountains behind it (public/ui/banner.jpg).
  let { eyebrow = '', title, tagline = '', icon = '', slashes = true, children }: { eyebrow?: string; title: string; tagline?: string; icon?: string; slashes?: boolean; children?: Snippet } = $props();
  let product = $state('');
  onMount(async () => { try { product = (await productInfo()).name; } catch { /* the plain name is used */ } });
  const eye = $derived((eyebrow && eyebrow !== 'CINDRIX OPS' ? eyebrow : product || 'Cindrix Ops').toUpperCase());
</script>

<header class="ph">
  {#if icon}<span class="bigicon"><Icon name={icon} size={54} /></span>{/if}
  <div class="txt">
    <div class="eyebrow">{eye}</div>
    <h1>{title}{#if slashes && !tagline.includes('•')}<i class="slash" aria-hidden="true"></i>{/if}</h1>
    {#if tagline}<p class="tag" class:dots={tagline.includes('•')}>{tagline}</p>{/if}
  </div>
  {#if children}<div class="acts">{@render children()}</div>{/if}
</header>

<style>
  .ph { position: relative; width: 100%; box-sizing: border-box; display: flex; align-items: center; gap: 16px; min-height: 124px; padding: 14px 22px; border: 1px solid #1d3550; border-radius: 12px; overflow: hidden; background: #060d17 url('/ui/banner.jpg') right center / auto 100% no-repeat; box-shadow: inset 0 -2px 0 #1b6fc2, 0 0 22px rgba(60, 140, 230, .12); }
  .ph::before { content: ''; position: absolute; inset: 0; background: linear-gradient(90deg, #060d17 0%, #060d17 34%, rgba(6, 13, 23, .55) 58%, rgba(6, 13, 23, 0) 80%); pointer-events: none; }
  .txt, .acts, .bigicon { position: relative; z-index: 1; }
  .bigicon { color: #5fb0ff; filter: drop-shadow(0 0 8px rgba(95, 176, 255, .5)); }
  .txt { flex: 1 1 auto; min-width: 0; }
  .eyebrow { font-family: var(--mono); font-size: 13px; font-weight: 700; letter-spacing: .28em; color: #3d9bff; text-shadow: 0 0 10px rgba(61, 155, 255, .45); }
  h1 { margin: 2px 0 4px; font-size: 50px; line-height: 1; font-weight: 800; letter-spacing: .035em; background: linear-gradient(180deg, #ffffff 25%, #aeb8c4 100%); -webkit-background-clip: text; background-clip: text; color: transparent; text-shadow: none; }
  .slash { display: inline-block; width: 64px; height: 16px; margin-left: 22px; vertical-align: middle; background: repeating-linear-gradient(115deg, #2f86e8 0 14px, transparent 14px 20px); filter: drop-shadow(0 0 6px rgba(47, 134, 232, .7)); }
  .tag { margin: 0; max-width: 620px; color: #dfe7f0; font-size: 15px; }
  .tag.dots { font-size: 12.5px; letter-spacing: .2em; text-transform: uppercase; color: #e8eef6; }
  .acts { display: flex; flex-wrap: wrap; gap: 10px; justify-content: flex-end; align-items: center; margin-left: auto; max-width: 62%; }
  @media (max-width: 760px) { .ph { flex-wrap: wrap; } h1 { font-size: 34px; } .acts { max-width: none; margin-left: 0; justify-content: flex-start; } }
</style>
