<script lang="ts">
  // Security Operations: one screen with six subtabs (docs/44). Only Command is built so far; the others say so.
  import SecurityCommand from './SecurityCommand.svelte';

  const tabs = [
    { id: 'Command', c: 'var(--cat-security)', built: true, about: 'Where every site stands, who is on watch, and what needs a look.' },
    { id: 'Security', c: 'var(--cat-intel)', built: false, about: 'Zones, barriers, alarms and inspections for each site.' },
    { id: 'Operations', c: 'var(--cat-ops)', built: false, about: 'The watchbill, patrols and the day\'s tasks.' },
    { id: 'Planning', c: 'var(--cat-academy)', built: false, about: 'Defensive and Operational Response plans, with their revisions.' },
    { id: 'Personnel', c: 'var(--cat-morale)', built: false, about: 'Who is available, qualifications and rest.' },
    { id: 'Records', c: 'var(--cat-system)', built: false, about: 'Incident history, logs, reviews and exports.' },
  ] as const;
  let tab: (typeof tabs)[number]['id'] = $state('Command');
  const current = $derived(tabs.find((t) => t.id === tab)!);
</script>

<div class="head">
  <h1>Security Operations</h1>
  <p class="sub">Keep watch over your sites, plan for trouble, and keep a clear record. Everything here stays on this computer.</p>
</div>

<div class="subtabs" role="tablist" aria-label="Security Operations">
  {#each tabs as t (t.id)}
    <button role="tab" class:on={tab === t.id} aria-selected={tab === t.id} style={`--c: ${t.c}`} onclick={() => (tab = t.id)}>
      <i aria-hidden="true"></i>{t.id}
    </button>
  {/each}
</div>

{#if current.built}
  <SecurityCommand />
{:else}
  <div class="panel soon">
    <h2>{current.id}</h2>
    <p class="muted">{current.about}</p>
    <p>This part is not built yet. The Command tab already works.</p>
    <button class="btn" onclick={() => (tab = 'Command')}>Back to Command</button>
  </div>
{/if}

<style>
  .head { margin-bottom: 12px; }
  .subtabs { display: flex; flex-wrap: wrap; gap: 8px; margin: 12px 0 16px; }
  .subtabs button { display: inline-flex; align-items: center; gap: 8px; padding: 8px 14px; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; color: var(--muted); font-family: var(--mono); text-transform: uppercase; letter-spacing: .1em; font-size: 12px; font-weight: 500; }
  .subtabs button i { width: 9px; height: 9px; border-radius: 50%; background: var(--c); display: inline-block; }
  .subtabs button:hover { border-color: var(--line2); color: var(--text); }
  .subtabs button.on { background: var(--blue-soft); color: var(--blue); border-color: var(--blue-line); font-weight: 600; box-shadow: inset 0 -2px 0 var(--blue); }
  .soon { max-width: 560px; }
</style>
