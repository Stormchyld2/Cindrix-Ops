<script lang="ts">
  // Security Operations > Planning > radios and batteries (core/src/security_equipment.rs, plan COPS-COM-001 section 10).
  // The gear stays in the Inventory. This only records which items are communications gear, when each was tested, and
  // when each failed. A fault stays until a later "repaired" line; the failure count never goes down.
  import { onMount } from 'svelte';
  import { securityEquipmentList, securityEquipmentAdd, type GearList, type GearRow } from '../lib/api';

  let { site = '', me = '' }: { site: string; me: string } = $props();
  let data = $state<GearList | null>(null);
  let note = $state('');
  const who = $derived(me.trim());
  let pick = $state(''); let kind = $state('radio'); let days = $state(30); let here = $state(true);
  let acting = $state<{ uid: string; ev: 'test' | 'fault' | 'repaired' | 'charged' | 'retired' } | null>(null);
  let words = $state('');

  async function load() { try { data = await securityEquipmentList(); } catch (e) { note = String(e); } }
  onMount(load);
  async function act(fn: () => Promise<unknown>, ok: string) { try { await fn(); note = ok; await load(); } catch (e) { note = String(e); } }

  const list = () => act(async () => { await securityEquipmentAdd(pick, 'listed', kind, here ? site : '', Number(days) || 0, who, ''); pick = ''; }, 'Listed. It stays in the Inventory as it was.');
  const write = (g: GearRow) => act(async () => { await securityEquipmentAdd(g.uid, acting!.ev, '', '', 0, who, words); acting = null; words = ''; }, 'Written down.');
  const when = (iso: string) => { const d = new Date(iso); return isNaN(+d) ? 'never' : d.toLocaleDateString([], { month: 'short', day: 'numeric' }); };
  const VERB: Record<string, string> = { test: 'Tested', fault: 'Something is wrong', repaired: 'Fixed or replaced', charged: 'Charged', retired: 'Take out of use' };
  const live = $derived((data?.gear ?? []).filter((g) => !g.retired));
</script>

<div class="panel">
  <h2>Radios and batteries</h2>
  <p class="muted">Pick the radios, batteries and chargers the plan depends on from your Inventory. Test them on a schedule and write down every failure, so you can see which ones to stop trusting. A fault stays on "Needs a look" until someone writes that it was fixed.</p>
  {#if note}<p class="note" role="status">{note}</p>{/if}
  {#if data && data.unreadable_lines > 0}<p class="note">Some lines could not be read. They were left exactly as they are, and nothing new is added until they are looked at.</p>{/if}
  {#if live.length}
    <table class="grid"><thead><tr><th>Item</th><th>Kind</th><th>Last tested</th><th>Failures</th><th></th></tr></thead><tbody>
      {#each live as g (g.uid)}
        <tr class:weak={!!g.open_fault || !g.in_inventory}>
          <td><b>{g.name}</b>{#if g.open_fault}<div class="small">Fault: {g.open_fault}</div>{/if}<div class="small muted">{g.site ? 'One site' : 'Every site'}{g.test_days ? `, test every ${g.test_days} days` : ''}</div></td>
          <td>{data?.kinds.find((k) => k.id === g.kind)?.label ?? g.kind}</td>
          <td>{g.last_test ? when(g.last_test) : 'Never'}</td>
          <td>{g.faults}</td>
          <td class="acts">{#each ['test', 'charged', 'fault', 'repaired', 'retired'] as ev}<button class="btn" onclick={() => { acting = { uid: g.uid, ev: ev as 'test' }; words = ''; }}>{VERB[ev]}</button>{/each}</td>
        </tr>
        {#if acting && acting.uid === g.uid}
          <tr><td colspan="5"><div class="row ch">
            <b>{VERB[acting.ev]}</b>
            <input class="input grow" placeholder={acting.ev === 'fault' ? 'What went wrong' : 'Note (optional)'} bind:value={words} aria-label="Note" />
            <button class="btn primary" disabled={!who || (acting.ev === 'fault' && !words.trim())} onclick={() => write(g)} title={who ? '' : 'Write your name at the top first'}>Write it down</button>
            <button class="btn" onclick={() => (acting = null)}>Cancel</button>
          </div></td></tr>
        {/if}
      {/each}
    </tbody></table>
  {:else}<p class="muted">No gear listed yet.</p>{/if}
  <div class="row ch">
    <select class="input grow" bind:value={pick} aria-label="Inventory item"><option value="">Pick an item from the Inventory…</option>{#each data?.choices ?? [] as c}<option value={c.uid}>{c.name}{c.location ? ` (${c.location})` : ''}</option>{/each}</select>
    <select class="input" bind:value={kind} aria-label="Kind">{#each data?.kinds ?? [] as k}<option value={k.id}>{k.label}</option>{/each}</select>
    <label class="small">Test every <input class="input short" type="number" min="0" max="3650" bind:value={days} /> days</label>
    <label class="small"><input type="checkbox" bind:checked={here} /> this site only</label>
    <button class="btn primary" disabled={!pick || !who} onclick={list} title={who ? '' : 'Write your name at the top first'}>List it as gear</button>
  </div>
</div>

<style>
  .ch { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; margin: 8px 0; }
  .grow { flex: 1; min-width: 160px; }
  .short { width: 80px; }
  .acts { display: flex; gap: 6px; flex-wrap: wrap; }
  .grid { width: 100%; border-collapse: collapse; font-size: 13px; margin-top: 8px; }
  .grid th, .grid td { text-align: left; padding: 5px 8px; border-top: 1px solid var(--line); vertical-align: top; }
  .grid tr.weak td { background: var(--danger-bg, transparent); }
  .small { font-size: 12px; }
  .note { padding: 8px 10px; border-radius: 8px; background: var(--panel); }
</style>
