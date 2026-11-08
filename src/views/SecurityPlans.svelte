<script lang="ts">
  // Security Operations > Planning > written plans (core/src/security_plans.rs). Every save is a new version and old
  // versions stay. "Put into use" pins one exact version to a live incident and keeps a copy of its words.
  import { onMount } from 'svelte';
  import { securityPlansList, securityPlanRead, securityPlanSave, securityPlanActivate, securityIncidentList, type PlanInfo, type SecurityIncident } from '../lib/api';

  let { site = '', me = '' }: { site: string; me: string } = $props();
  let plans: PlanInfo[] = $state([]);
  let incidents: SecurityIncident[] = $state([]);
  let note = $state('');
  let openId: string | null = $state(null); // plan being edited; '' = new
  let pId = $state(''); let pText = $state(''); let pRev = $state(0);
  const who = $derived(me.trim());
  const usable = $derived(incidents.filter((i) => i.site === site && (i.status === 'active' || i.status === 'stabilized')));
  let useInc = $state(''); let useRev = $state(0);

  async function load() {
    try { plans = (await securityPlansList()).plans; incidents = (await securityIncidentList()).incidents; } catch (e) { note = String(e); }
  }
  onMount(load);
  async function act(fn: () => Promise<unknown>, ok = '') { try { await fn(); note = ok; await load(); } catch (e) { note = String(e); } }

  async function open(p: PlanInfo | null, rev = 0) {
    try {
      if (!p) { openId = ''; pId = ''; pText = '# Plan name\n\nWhen to use it:\n\nSteps:\n1. \n'; pRev = 0; return; }
      const r = await securityPlanRead(p.id, rev); openId = p.id; pId = p.id; pText = r.text; pRev = r.revision; useRev = 0; useInc = usable[0]?.id ?? '';
    } catch (e) { note = String(e); }
  }
  const save = () => act(async () => { const n = await securityPlanSave(pId.trim(), pText); openId = pId.trim(); pRev = n; note = `Saved as version ${n}.`; }, '');
  const use = () => act(async () => { await securityPlanActivate(useInc, pId, useRev, who); }, 'The plan is now in force for that incident. A copy of these exact words is kept with it.');
</script>

<div class="panel">
  <h2>Written plans</h2>
  <p class="muted">Evacuation, shelter and other plans. Saving changes makes a new version; old versions are never removed. When trouble starts, put a version into use on the incident, and that exact wording stays with the record even if the plan is changed later.</p>
  {#if note}<p class="note" role="status">{note}</p>{/if}
  {#if plans.length}
    <ul class="plist">
      {#each plans as p (p.id)}
        <li><b>{p.title || p.id}</b> <span class="muted small">{p.id} · version {p.current}{p.revisions.length > 1 ? ` (${p.revisions.length} saved)` : ''}</span> <button class="btn" onclick={() => open(p)}>Open</button></li>
      {/each}
    </ul>
  {:else}<p class="muted">No plans written yet.</p>{/if}
  {#if openId === null}<div class="acts"><button class="btn primary" onclick={() => open(null)}>Write a plan</button></div>{/if}

  {#if openId !== null}
    <div class="edit">
      {#if openId === ''}
        <input class="input" placeholder="Short name, like evacuate-camp" bind:value={pId} aria-label="Plan name" />
      {:else}
        <p class="small muted">Showing version {pRev} of <b>{pId}</b>.
          {#each plans.find((x) => x.id === pId)?.revisions ?? [] as n}<button class="btn small" onclick={() => open(plans.find((x) => x.id === pId)!, n)}>v{n}</button>{/each}</p>
      {/if}
      <textarea class="input text" rows="14" bind:value={pText} aria-label="Plan text"></textarea>
      <div class="acts"><button class="btn primary" disabled={!pId.trim()} onclick={save}>Save as a new version</button> <button class="btn" onclick={() => (openId = null)}>Close</button></div>
      {#if openId !== ''}
        <h3>Put into use</h3>
        {#if usable.length}
          <div class="row ch">
            <select class="input" bind:value={useInc} aria-label="Incident">{#each usable as i}<option value={i.id}>{i.title}</option>{/each}</select>
            <select class="input" bind:value={useRev} aria-label="Version"><option value={0}>Newest version</option>{#each plans.find((x) => x.id === pId)?.revisions ?? [] as n}<option value={n}>Version {n}</option>{/each}</select>
            <button class="btn primary" disabled={!who || !useInc} onclick={use} title={who ? '' : 'Write your name at the top first'}>Put it into use</button>
          </div>
        {:else}<p class="muted small">A plan can only be put into use on an active incident at this site. Open one on the Command tab and make it active first.</p>{/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .plist { list-style: none; padding: 0; margin: 8px 0; display: flex; flex-direction: column; gap: 6px; }
  .edit { margin-top: 12px; display: flex; flex-direction: column; gap: 8px; }
  .text { font-family: var(--mono); width: 100%; box-sizing: border-box; }
  .ch { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
  .acts { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 8px; }
  .small { font-size: 12px; }
  .note { padding: 8px 10px; border-radius: 8px; background: var(--panel); }
</style>
