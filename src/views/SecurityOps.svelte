<script lang="ts">
  // Security Operations: one screen with six subtabs (docs/44). All six subtabs are built (some parts of each are still waiting, see docs/44 section 17).
  import SecurityCommand from './SecurityCommand.svelte';
  import SecurityOperations from './SecurityOperations.svelte';
  import SecurityComms from './SecurityComms.svelte';
  import SecurityTab from './SecurityTab.svelte';
  import SecurityPlans from './SecurityPlans.svelte';
  import SecurityAvailability from './SecurityAvailability.svelte';

  const tabs = [
    { id: 'Command', c: 'var(--cat-security)', built: true, about: 'Where every site stands, who is on watch, and what needs a look.' },
    { id: 'Security', c: 'var(--cat-intel)', built: true, about: 'Zones, barriers, alarms and inspections for each site.' },
    { id: 'Operations', c: 'var(--cat-ops)', built: true, about: 'The watchbill, patrols and the day\'s tasks.' },
    { id: 'Planning', c: 'var(--cat-academy)', built: true, about: 'The radio channel plan, coverage tests, weather station and written plans.' },
    { id: 'Personnel', c: 'var(--cat-morale)', built: true, about: 'Who is available and the twelve radio skills for each person.' },
    { id: 'Records', c: 'var(--cat-system)', built: true, about: 'The communications log and a readable copy of the records.' },
  ] as const;
  let tab: (typeof tabs)[number]['id'] = $state('Command');
  const current = $derived(tabs.find((t) => t.id === tab)!);
  // The site being looked at and the name written on records are shared by every subtab.
  let site = $state('');
  let me = $state('');
  try { me = localStorage.getItem('securityName') ?? ''; } catch { /* a private window: just ask each time */ }
  function rememberMe() { try { localStorage.setItem('securityName', me); } catch { /* fine */ } }
</script>

<div class="head">
  <h1>Security Operations</h1>
  <p class="sub">Keep watch over your sites, plan for trouble, and keep a clear record. Everything here stays on this computer.</p>
  <div class="row who">
    <label class="label" for="me">Your name</label>
    <input id="me" class="input" placeholder="Written on every record you make" bind:value={me} onchange={rememberMe} />
  </div>
</div>

<div class="subtabs" role="tablist" aria-label="Security Operations">
  {#each tabs as t (t.id)}
    <button role="tab" class:on={tab === t.id} aria-selected={tab === t.id} style={`--c: ${t.c}`} onclick={() => (tab = t.id)}>
      <i aria-hidden="true"></i>{t.id}
    </button>
  {/each}
</div>

{#if tab === 'Command'}
  <SecurityCommand bind:site bind:me />
{:else if tab === 'Security'}
  <SecurityTab bind:site {me} />
{:else if tab === 'Operations'}
  <SecurityOperations bind:site {me} />
{:else if tab === 'Planning'}
  <SecurityComms part="planning" bind:site {me} />
  <SecurityPlans {site} {me} />
{:else if tab === 'Personnel'}
  <SecurityAvailability {me} />
  <SecurityComms part="personnel" bind:site {me} />
{:else if tab === 'Records'}
  <SecurityComms part="records" bind:site {me} />
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
  .who { margin-top: 10px; }
  .who .input { width: 280px; max-width: 100%; }
  .subtabs { display: flex; flex-wrap: wrap; gap: 8px; margin: 12px 0 16px; }
  .subtabs button { display: inline-flex; align-items: center; gap: 8px; padding: 8px 14px; background: var(--panel2); border: 1px solid var(--line); border-radius: 8px; color: var(--muted); font-family: var(--mono); text-transform: uppercase; letter-spacing: .1em; font-size: 12px; font-weight: 500; }
  .subtabs button i { width: 9px; height: 9px; border-radius: 50%; background: var(--c); display: inline-block; }
  .subtabs button:hover { border-color: var(--line2); color: var(--text); }
  .subtabs button.on { background: var(--blue-soft); color: var(--blue); border-color: var(--blue-line); font-weight: 600; box-shadow: inset 0 -2px 0 var(--blue); }
  .soon { max-width: 560px; }
</style>
