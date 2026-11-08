<script lang="ts">
  // Security Operations > Personnel > who is available (core/src/security_site.rs). The newest line for a person counts.
  // Someone marked unavailable who is on the watchbill shows under "Needs a look" on Command.
  import { onMount } from 'svelte';
  import { securityAvailabilityList, securityAvailabilitySet, type AvailabilityList } from '../lib/api';

  let { me = '' }: { me: string } = $props();
  let list: AvailabilityList | null = $state(null);
  let note = $state('');
  let person = $state(''); let reason = $state(''); let until = $state('');
  const who = $derived(me.trim());

  async function load() { try { list = await securityAvailabilityList(); } catch (e) { note = String(e); } }
  onMount(load);
  async function set(p: string, status: 'available' | 'unavailable', u = '', r = '') {
    try { await securityAvailabilitySet(p, status, u, r, who); note = 'Saved.'; person = reason = until = ''; await load(); } catch (e) { note = String(e); }
  }
  const when = (iso: string) => { const d = new Date(iso); return isNaN(+d) ? iso : d.toLocaleString([], { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }); };
</script>

<div class="panel">
  <h2>Who is available</h2>
  <p class="muted">Mark someone unavailable when they are sick, hurt, away or resting. Leave the end time empty to mean "until I say otherwise". If they are on the watchbill in that time, Command shows it. Nothing on the watchbill is changed for you.</p>
  {#if note}<p class="note" role="status">{note}</p>{/if}
  {#if list && list.unreadable_lines > 0}<p class="note">Some lines could not be read. They were left exactly as they are, and nothing new is added until they are looked at.</p>{/if}
  {#if list && list.unavailable.length}
    {#each list.unavailable as a (a.id)}
      <div class="row ch"><b>{a.person}</b> <span>{a.reason}</span> <span class="muted small">{a.until ? `until ${when(a.until)}` : 'until said otherwise'}</span>
        <button class="btn" disabled={!who} onclick={() => set(a.person, 'available')} title={who ? '' : 'Write your name at the top first'}>Available again</button></div>
    {/each}
  {:else}<p class="muted">Everyone is available.</p>{/if}
  <div class="row ch">
    <input class="input" placeholder="Who" bind:value={person} aria-label="Who" />
    <input class="input grow" placeholder="Why (sick, away, injured, resting)" bind:value={reason} aria-label="Why" />
    <label class="small">Until <input class="input" type="datetime-local" bind:value={until} /></label>
    <button class="btn primary" disabled={!who || !person.trim() || !reason.trim()} onclick={() => set(person, 'unavailable', until, reason)} title={who ? '' : 'Write your name at the top first'}>Mark unavailable</button>
  </div>
</div>

<style>
  .ch { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; margin: 8px 0; }
  .grow { flex: 1; min-width: 160px; }
  .small { font-size: 12px; }
  .note { padding: 8px 10px; border-radius: 8px; background: var(--panel); }
</style>
