<script lang="ts">
  // Security Operations > Personnel: role qualifications and who has which piece of gear (core/src/security_roles.rs).
  // Nothing is filled in for you. Gear itself lives in the Gear list (top bar: Operations > Gear).
  import { onMount } from 'svelte';
  import { securityRolesList, securityRolesAdd, type RolesList } from '../lib/api';

  let { me = '' }: { me: string } = $props();
  let data = $state<RolesList | null>(null);
  let note = $state('');
  const who = $derived(me.trim());
  const today = () => new Date().toISOString().slice(0, 10);

  let roleName = $state(''); let roleDays = $state('365'); let roleMin = $state('1'); let roleSite = $state(''); let roleNote = $state('');
  let qPerson = $state(''); let qRole = $state(''); let qDate = $state(today()); let qNote = $state('');
  let gGear = $state(''); let gPerson = $state(''); let gNote = $state('');
  let taking = $state(''); let why = $state('');

  async function load() { try { data = await securityRolesList(); } catch (e) { note = String(e); } }
  onMount(load);
  async function act(fn: () => Promise<unknown>, ok: string) { try { await fn(); note = ok; await load(); } catch (e) { note = String(e); } }

  const addRole = () => act(async () => { await securityRolesAdd('role', { a: roleName, b: roleSite, c: roleDays, d: roleMin, note: roleNote, by: who }); roleName = ''; roleNote = ''; }, 'Role saved.');
  const qualify = () => act(async () => { await securityRolesAdd('qualify', { a: qPerson, b: qRole, c: qDate, note: qNote, by: who }); qNote = ''; }, 'Written down.');
  const revoke = (person: string, role: string) => act(async () => { await securityRolesAdd('revoke', { a: person, b: role, note: why, by: who }); taking = ''; why = ''; }, 'Taken away, and the reason is written down.');
  const issue = () => act(async () => { await securityRolesAdd('issue', { a: gGear, b: gPerson, note: gNote, by: who }); gNote = ''; }, 'Handed out.');
  const giveBack = (gear: string, person: string) => act(async () => { await securityRolesAdd('return', { a: gear, b: person, by: who }); }, 'Handed back.');
  const SAY: Record<string, string> = { current: 'Current', 'running out': 'Running out', lapsed: 'Ran out' };
  const day = (iso: string) => { const d = new Date(iso); return isNaN(+d) ? '' : d.toLocaleDateString([], { month: 'short', day: 'numeric', year: 'numeric' }); };
</script>

<div class="panel">
  <h2>Roles and qualifications</h2>
  <p class="muted">A role is something a person has to be qualified for, like radio operator or first aid. Say how long a qualification stays good and how many qualified people you want. Anything running out shows on "Needs a look".</p>
  {#if note}<p class="note" role="status">{note}</p>{/if}
  {#if data && data.unreadable_lines > 0}<p class="note">Some lines could not be read. They were left exactly as they are, and nothing new is added until they are looked at.</p>{/if}
  <datalist id="people">{#each data?.people ?? [] as p}<option value={p}></option>{/each}</datalist>

  {#if data?.roles.length}
    <table class="grid"><thead><tr><th>Role</th><th>Good for</th><th>Qualified</th><th>People</th></tr></thead><tbody>
      {#each data.roles as r (r.name)}
        <tr class:weak={r.min_people > 0 && r.qualified < r.min_people}>
          <td><b>{r.name}</b>{#if r.note}<div class="small">{r.note}</div>{/if}<div class="small muted">{r.site ? (data.sites.find((s) => s.id === r.site)?.name ?? r.site) : 'Every site'}</div></td>
          <td>{r.valid_days ? `${r.valid_days} days` : 'Does not run out'}</td>
          <td>{r.qualified}{r.min_people ? ` of ${r.min_people} wanted` : ''}</td>
          <td class="small">{#each data.quals.filter((x) => x.role === r.name) as x (x.person)}
            <div class:weak={x.standing !== 'current'}>{x.person}: {SAY[x.standing]}{x.until ? ` (to ${day(x.until)})` : ''}
              {#if taking === x.person + '|' + r.name}
                <input class="input" placeholder="Why" bind:value={why} aria-label="Reason" />
                <button class="btn" disabled={!who || !why.trim()} onclick={() => revoke(x.person, r.name)}>Take it away</button>
                <button class="btn" onclick={() => (taking = '')}>Cancel</button>
              {:else}<button class="btn" onclick={() => { taking = x.person + '|' + r.name; why = ''; }}>Take away</button>{/if}
            </div>
          {:else}No one yet{/each}</td>
        </tr>
      {/each}
    </tbody></table>
  {:else}<p class="muted">No roles yet. Add the first one below.</p>{/if}

  <div class="row ch">
    <input class="input grow" placeholder="Role, like Radio operator" bind:value={roleName} aria-label="Role name" />
    <label class="small">Good for <input class="input short" type="number" min="0" max="3650" bind:value={roleDays} /> days (0 = never runs out)</label>
    <label class="small">People wanted <input class="input short" type="number" min="0" max="99" bind:value={roleMin} /></label>
    <select class="input" bind:value={roleSite} aria-label="Site"><option value="">Every site</option>{#each data?.sites ?? [] as s}<option value={s.id}>{s.name}</option>{/each}</select>
    <input class="input grow" placeholder="What it covers (optional)" bind:value={roleNote} aria-label="What it covers" />
    <button class="btn primary" disabled={!who || !roleName.trim()} onclick={addRole} title={who ? '' : 'Write your name at the top first'}>Save role</button>
  </div>
  <div class="row ch">
    <input class="input" list="people" placeholder="Person" bind:value={qPerson} aria-label="Person" />
    <select class="input" bind:value={qRole} aria-label="Role"><option value="">Role…</option>{#each data?.roles ?? [] as r}<option value={r.name}>{r.name}</option>{/each}</select>
    <label class="small">Earned on <input class="input" type="date" bind:value={qDate} /></label>
    <input class="input grow" placeholder="Note (optional)" bind:value={qNote} aria-label="Note" />
    <button class="btn primary" disabled={!who || !qPerson.trim() || !qRole} onclick={qualify} title={who ? '' : 'Write your name at the top first'}>Write it down</button>
  </div>
</div>

<div class="panel">
  <h2>Gear handed out</h2>
  <p class="muted">Who has which piece from the Gear list. Handing a piece back puts it back on the shelf in this record. The Gear list itself is not changed.</p>
  {#if data?.held.length}
    <table class="grid"><thead><tr><th>Gear</th><th>Make, model, serial</th><th>With</th><th>Since</th><th></th></tr></thead><tbody>
      {#each data.held as h (h.gear + h.person)}
        <tr class:weak={h.missing}>
          <td><b>{h.missing ? 'Not in the Gear list any more' : h.name}</b>{#if h.note}<div class="small">{h.note}</div>{/if}</td>
          <td>{[h.make, h.model, h.serial].filter(Boolean).join(' ') || '—'}</td>
          <td>{h.person}</td><td>{day(h.since)}</td>
          <td><button class="btn" disabled={!who} onclick={() => giveBack(h.gear, h.person)}>Handed back</button></td>
        </tr>
      {/each}
    </tbody></table>
  {:else}<p class="muted">Nothing is handed out.</p>{/if}
  <div class="row ch">
    <select class="input grow" bind:value={gGear} aria-label="Gear"><option value="">Pick from the Gear list…</option>{#each data?.gear ?? [] as g}<option value={g.id}>{g.name}{g.detail ? ` (${g.detail})` : ''}{g.qty > 1 ? `, ${g.qty - g.held} of ${g.qty} free` : ''}</option>{/each}</select>
    <input class="input" list="people" placeholder="Person" bind:value={gPerson} aria-label="Person" />
    <input class="input grow" placeholder="Note (optional)" bind:value={gNote} aria-label="Note" />
    <button class="btn primary" disabled={!who || !gGear || !gPerson.trim()} onclick={issue} title={who ? '' : 'Write your name at the top first'}>Hand it out</button>
  </div>
  {#if data && !data.gear.length}<p class="muted">The Gear list is empty. Add your gear there first (Operations > Gear).</p>{/if}
</div>
