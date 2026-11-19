<script lang="ts">
  // Roles for the people on trusted phones (core/src/permissions.rs). Two exist at the start; add or remove as you like.
  import { onMount } from 'svelte';
  import { rolesGet, rolesAdd, rolesRemove, rolesAssign, type RolesView } from '../lib/api';

  let { people = [] }: { people?: string[] } = $props();

  let view = $state<RolesView | null>(null);
  let error = $state('');
  let newName = $state('');
  let newCaps = $state<string[]>(['log']);

  async function run(f: () => Promise<RolesView>) {
    error = '';
    try { view = await f(); } catch (e) { error = String(e).replace(/^Error:\s*/, ''); }
  }
  onMount(() => run(rolesGet));
  // keep the list of people in step with the devices on screen
  const names = $derived([...new Set([...(view?.people.map((p) => p.person) ?? []), ...people])]);
  const roleOf = (p: string) => view?.people.find((x) => x.person === p)?.role ?? view?.roles[0]?.name ?? '';
  const capText = (k: string) => view?.caps.find((c) => c.key === k)?.text ?? k;
  function toggleCap(k: string) { newCaps = newCaps.includes(k) ? newCaps.filter((c) => c !== k) : [...newCaps, k]; }
  async function add() { await run(() => rolesAdd(newName, newCaps)); if (!error) { newName = ''; newCaps = ['log']; } }
</script>

<div class="roles">
  <div class="small muted pinlabel">What each person's phone may do</div>
  {#if view}
    {#each view.roles as r (r.name)}
      <div class="line">
        <div class="grow"><b>{r.name}</b>
          <div class="small muted">{r.caps.length ? r.caps.map(capText).join('. ') + '.' : 'Can only read.'}</div></div>
        <button class="btn" onclick={() => run(() => rolesRemove(r.name))} title="Take this role away. People who had it get the first role in the list.">Remove</button>
      </div>
    {:else}
      <p class="small muted">There are no roles, so phones can read and nothing else. Add one below.</p>
    {/each}

    {#if names.length}
      <div class="small muted pinlabel">Role for each person</div>
      {#each names as p (p)}
        <div class="line">
          <div class="grow">{p}</div>
          <select class="input" value={roleOf(p)} aria-label="Role for {p}" onchange={(e) => run(() => rolesAssign(p, (e.currentTarget as HTMLSelectElement).value))} disabled={!view.roles.length}>
            {#each view.roles as r (r.name)}<option value={r.name}>{r.name}</option>{/each}
          </select>
        </div>
      {/each}
    {/if}

    <div class="small muted pinlabel">Add a role</div>
    <div class="row">
      <input class="input" placeholder="Role name" aria-label="Role name" bind:value={newName} />
      {#each view.caps as c (c.key)}
        <label class="small"><input type="checkbox" checked={newCaps.includes(c.key)} onchange={() => toggleCap(c.key)} /> {c.text}</label>
      {/each}
      <button class="btn primary" onclick={add} disabled={!newName.trim()}>Add the role</button>
    </div>
    <p class="small muted">Reading the library never needs a role. A phone never changes the host's own files or the security records directly. The most it can do is send reports, and, with the permission to accept or set aside reports, deal with the ones waiting on the host.</p>
  {/if}
  {#if error}<p class="warn" role="alert">{error}</p>{/if}
</div>

<style>
  .roles { margin-top: 10px; display: flex; flex-direction: column; gap: 6px; }
  .line { display: flex; gap: 12px; align-items: center; padding: 6px 0; border-top: 1px solid var(--line); }
  .row { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; }
  .grow { flex: 1; }
  .small { font-size: 12px; }
  .pinlabel { margin-top: 8px; }
</style>
