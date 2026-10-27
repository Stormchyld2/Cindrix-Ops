<script lang="ts">
  // A private daily journal, one text file per day in the person's own folder (user/people/<name>/journal).
  import { onMount } from 'svelte';
  import { journalList, journalSave, isLocked, type JournalEntry } from '../lib/api';
  import PrivateLock from '../lib/PrivateLock.svelte';
  import { who } from '../lib/person.svelte';
  import PersonPick from '../lib/PersonPick.svelte';
  import MonthGrid from '../lib/MonthGrid.svelte';

  function today(): string {
    const d = new Date();
    return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
  }
  let entries: JournalEntry[] = $state([]);
  let day = $state(today());
  let text = $state('');
  let saved = $state('');
  let note = $state('');
  let dirty = $state(false);
  let loadedFor = '';
  let locked = $state(false);

  async function load() {
    note = '';
    locked = false;
    try { entries = await journalList(who.name); } catch (e) { entries = []; if (isLocked(e)) locked = true; else note = String(e); }
    pick(day);
    loadedFor = who.name;
  }
  function pick(d: string) {
    day = d;
    text = entries.find((e) => e.date === d)?.text ?? '';
    saved = text;
    dirty = false;
  }
  async function save() {
    note = '';
    try {
      await journalSave(who.name, day, text);
      saved = text;
      dirty = false;
      entries = await journalList(who.name);
      note = text.trim() ? 'Saved.' : 'This day was cleared. A copy of the old words is kept in your backups folder.';
    } catch (e) { if (isLocked(e)) { locked = true; entries = []; } else note = String(e); }
  }
  function leave(d: string) {
    if (dirty && !confirm('You have words that are not saved. Leave them behind?')) return;
    pick(d);
  }
  onMount(load);
  const first = (t: string) => (t.trim().split('\n')[0] ?? '').slice(0, 70);
  const marks = $derived(Object.fromEntries(entries.filter((e) => e.text.trim()).map((e) => [e.date, 1])));
  const nice = (d: string) => new Date(d + 'T12:00:00').toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric', year: 'numeric' });
</script>

<section>
  <h1>Journal</h1>
  <p class="sub">A place to write down what happened each day. It helps to keep track, and it leaves a record. Other devices on your hotspot can never see it. Without a passcode, anyone using this computer can open the folder. With one, only you can read it.</p>
  <div class="row"><PersonPick onchange={load} /></div>
  <PrivateLock person={who.name} needsUnlock={locked} onchange={load} />
  {#if note}<p class="notice" role="status">{note}</p>{/if}

  {#if !locked}
  <div class="wrap">
    <nav class="days" aria-label="Days">
      <MonthGrid compact selected={day} {marks} onpick={leave} />
      <button class="btn" onclick={() => leave(today())}>Today</button>
      {#each entries as e (e.date)}
        <button class="day" class:on={e.date === day} onclick={() => leave(e.date)}>
          <strong>{nice(e.date)}</strong>
          <span class="muted small">{first(e.text)}</span>
        </button>
      {/each}
      {#if !entries.length}<p class="muted small">No days written yet.</p>{/if}
    </nav>
    <div class="edit panel">
      <div class="label">{nice(day)}</div>
      <textarea class="input" rows="16" bind:value={text} oninput={() => (dirty = text !== saved)} placeholder="What happened today? Who was where? What did you use, make or fix? What do you need tomorrow?" aria-label="Today's words"></textarea>
      <div class="row">
        <button class="btn primary" onclick={save} disabled={!dirty}>Save</button>
        <span class="muted small">{dirty ? 'Not saved yet.' : text.trim() ? 'Saved.' : ''}</span>
      </div>
    </div>
  </div>
  {/if}
</section>

<style>
  .wrap { display: grid; grid-template-columns: 260px 1fr; gap: 14px; margin-top: 12px; align-items: start; }
  .days { display: flex; flex-direction: column; gap: 6px; max-height: 78vh; overflow-y: auto; scrollbar-width: none; }
  .days::-webkit-scrollbar { display: none; }
  .day { text-align: left; padding: 9px 10px; border: 1px solid var(--line); border-left: 3px solid var(--cat-morale); border-radius: 8px; background: var(--panel); display: flex; flex-direction: column; gap: 2px; }
  .day.on { border-color: var(--blue-line); background: var(--blue-soft); }
  .edit { display: flex; flex-direction: column; gap: 10px; border-left: 3px solid var(--cat-morale); }
  textarea { width: 100%; resize: vertical; font-family: inherit; line-height: 1.5; }
  .small { font-size: 13px; }
  .notice { margin: 8px 0 0; }
  @media (max-width: 760px) { .wrap { grid-template-columns: 1fr; } .days { max-height: 220px; } }
</style>
