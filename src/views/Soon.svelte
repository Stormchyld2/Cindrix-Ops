<script lang="ts">
  import PageHeader from '../lib/PageHeader.svelte';
  let { title }: { title: string } = $props();
</script>

<section>
  <PageHeader eyebrow="CINDRIX OPS" title={title} tagline="This part is not built yet. It is planned, and it will show up here when it is ready." />
</section>
