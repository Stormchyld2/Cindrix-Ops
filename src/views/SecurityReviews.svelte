<script lang="ts">
  // Security Operations > Records (exercises and after-action reviews) and Personnel (who has practised lately).
  // Core: core/src/security_review.rs. Lines are never changed; an action is done, then checked by someone, or dropped with a reason.
  import { onMount } from 'svelte';
  import { securityReviewList, securityReviewAdd, type ReviewList, type ReviewRow } from '../lib/api';

  let { part = 'records', site = '', me = '' }: { part: 'records' | 'personnel'; site: string; me: string } = $props();
  let data = $state<ReviewList | null>(null);
  let note = $state('');
  const who = $derived(me.trim());
  const today = () => new Date().toISOString().slice(0, 10);

  let exTitle = $state(''); let exDate = $state(today()); let exWords = $state(''); let exPeople = $state('');
  let arTitle = $state(''); let arRef = $state('');
  let draft = $state<Record<string, { kind: 'good' | 'fix'; text: string; what: string; owner: string; due: string; why: string }>>({});
  const blank = () => ({ kind: 'fix' as 'good' | 'fix', text: '', what: '', owner: '', due: '', why: '' });
  const d = (id: string) => draft[id] ?? blank();
  // drafts are made when the list loads, never while the screen is drawing
  function prepare(l: ReviewList) {
    const ids = [...l.exercises.map((e) => e.id), ...l.reviews.flatMap((r) => [r.id, ...r.actions.map((a) => a.id)])];
    for (const id of ids) if (!draft[id]) draft[id] = blank();
  }

  async function load() { try { const l = await securityReviewList(); prepare(l); data = l; } catch (e) { note = String(e); } }
  onMount(load);
  async function act(fn: () => Promise<unknown>, ok: string) { try { await fn(); note = ok; await load(); } catch (e) { note = String(e); } }

  const addExercise = () => act(async () => {
    const id = await securityReviewAdd('exercise', { ref: site, a1: exTitle, a2: exDate, note: exWords, by: who });
    for (const p of exPeople.split(',').map((x) => x.trim()).filter(Boolean)) await securityReviewAdd('attended', { rec: id, a1: p, by: who });
    exTitle = ''; exWords = ''; exPeople = '';
  }, 'Exercise written down.');
  const addPerson = (id: string) => act(async () => { const n = d(id).owner; await securityReviewAdd('attended', { rec: id, a1: n, by: who }); d(id).owner = ''; }, 'Added.');
  const openReview = () => act(async () => { await securityReviewAdd('aar', { ref: arRef, a1: arTitle, by: who }); arTitle = ''; arRef = ''; }, 'Review opened.');
  const finding = (r: ReviewRow) => act(async () => { await securityReviewAdd('finding', { rec: r.id, a1: d(r.id).kind, note: d(r.id).text, by: who }); d(r.id).text = ''; }, 'Written down.');
  const action = (r: ReviewRow) => act(async () => { const x = d(r.id); await securityReviewAdd('action', { rec: r.id, a1: x.owner, a2: x.due, note: x.what, by: who }); x.what = ''; x.owner = ''; x.due = ''; }, 'Action added.');
  const move = (r: ReviewRow, id: string, ev: 'done' | 'verified' | 'dropped') => act(async () => { await securityReviewAdd(ev, { rec: r.id, ref: id, note: d(id).why, by: who }); d(id).why = ''; }, 'Written down.');
  const close = (r: ReviewRow) => act(async () => { await securityReviewAdd('closed', { rec: r.id, by: who }); }, 'Review closed.');
  const refName = (id: string) => data?.incidents.find((i) => i.id === id)?.title ?? data?.exercises.find((e) => e.id === id)?.title ?? '';
  const SAY: Record<string, string> = { open: 'To do', done: 'Done, not checked', verified: 'Checked', dropped: 'Dropped' };
</script>

{#if part === 'personnel'}
<div class="panel">
  <h2>Practice</h2>
  <p class="muted">When each person last took part in an exercise, and how many in the last six months.</p>
  {#if data?.readiness.length}
    <table class="grid"><thead><tr><th>Person</th><th>Last exercise</th><th>Last six months</th></tr></thead><tbody>
      {#each data.readiness as r (r.person)}
        <tr class:weak={r.days_ago === null || r.days_ago > 180}><td>{r.person}</td><td>{r.last ? `${r.last} (${r.days_ago} days ago)` : 'Never'}</td><td>{r.recent}</td></tr>
      {/each}
    </tbody></table>
  {:else}<p class="muted">No one yet. People appear once they are on the watchbill or take part in an exercise.</p>{/if}
</div>
{:else}
<div class="panel">
  <h2>Exercises</h2>
  <p class="muted">Write down practice once it has been held: what you did and who took part. Exercises are never counted as real incidents. About every {data?.every_days ?? 90} days is the aim.</p>
  {#if note}<p class="note" role="status">{note}</p>{/if}
  {#if data && data.unreadable_lines > 0}<p class="note">Some lines could not be read. They were left exactly as they are, and nothing new is added until they are looked at.</p>{/if}
  {#each data?.exercises ?? [] as e (e.id)}
    <div class="item"><b>{e.title}</b> <span class="muted">{e.date}</span>{#if e.note}<div class="small">{e.note}</div>{/if}
      <div class="small">Took part: {e.attendees.length ? e.attendees.join(', ') : 'no one written down'}</div>
      <div class="row ch"><input class="input" placeholder="Add a person" bind:value={draft[e.id].owner} aria-label="Person" /><button class="btn" disabled={!who || !d(e.id).owner.trim()} onclick={() => addPerson(e.id)}>Add</button></div>
    </div>
  {:else}<p class="muted">No exercises yet.</p>{/each}
  <div class="row ch">
    <input class="input grow" placeholder="Short name, like Night alarm drill" bind:value={exTitle} aria-label="Exercise name" />
    <input class="input" type="date" bind:value={exDate} aria-label="Date held" />
    <input class="input grow" placeholder="What you did and how it went" bind:value={exWords} aria-label="What happened" />
    <input class="input grow" placeholder="Who took part (names, commas)" bind:value={exPeople} aria-label="Who took part" />
    <button class="btn primary" disabled={!who || !exTitle.trim()} onclick={addExercise} title={who ? '' : 'Write your name at the top first'}>Write it down</button>
  </div>
</div>

<div class="panel">
  <h2>After-action reviews</h2>
  <p class="muted">Look back at an incident or an exercise: what went well, what to fix, and who will do what by when. A review can be closed once every action is checked or dropped.</p>
  {#each data?.reviews ?? [] as r (r.id)}
    <div class="item" class:muted={r.closed}>
      <b>{r.title}</b> {#if r.ref}<span class="muted">about {refName(r.ref) || r.ref}</span>{/if} {#if r.closed}<span class="small">(closed)</span>{/if}
      {#each r.findings as f}<div class="small">{f.kind === 'good' ? 'Went well' : 'Needs fixing'}: {f.text}</div>{/each}
      {#each r.actions as a (a.id)}
        <div class="small" class:weak={a.late}>{a.text} — {a.owner}, due {a.due}{a.late ? ' (overdue)' : ''} · {SAY[a.state]}{a.note ? `: ${a.note}` : ''}
          {#if !r.closed && (a.state === 'open' || a.state === 'done')}
            <input class="input" placeholder="Note (needed to drop)" bind:value={draft[a.id].why} aria-label="Note" />
            {#if a.state === 'open'}<button class="btn" disabled={!who} onclick={() => move(r, a.id, 'done')}>Done</button>{/if}
            <button class="btn" disabled={!who} onclick={() => move(r, a.id, 'verified')}>Checked</button>
            <button class="btn" disabled={!who || !d(a.id).why.trim()} onclick={() => move(r, a.id, 'dropped')}>Drop</button>
          {/if}
        </div>
      {/each}
      {#if !r.closed}
        <div class="row ch">
          <select class="input" bind:value={draft[r.id].kind} aria-label="Kind"><option value="good">Went well</option><option value="fix">Needs fixing</option></select>
          <input class="input grow" placeholder="What you saw" bind:value={draft[r.id].text} aria-label="Finding" />
          <button class="btn" disabled={!who || !d(r.id).text.trim()} onclick={() => finding(r)}>Add</button>
        </div>
        <div class="row ch">
          <input class="input grow" placeholder="What is to be done" bind:value={draft[r.id].what} aria-label="Action" />
          <input class="input" placeholder="Who" bind:value={draft[r.id].owner} aria-label="Owner" />
          <input class="input" type="date" bind:value={draft[r.id].due} aria-label="Due date" />
          <button class="btn" disabled={!who || !d(r.id).what.trim() || !d(r.id).owner.trim() || !d(r.id).due} onclick={() => action(r)}>Add action</button>
          <button class="btn primary" disabled={!who} onclick={() => close(r)}>Close review</button>
        </div>
      {/if}
    </div>
  {:else}<p class="muted">No reviews yet.</p>{/each}
  <div class="row ch">
    <input class="input grow" placeholder="Short name, like Car at the gate" bind:value={arTitle} aria-label="Review name" />
    <select class="input" bind:value={arRef} aria-label="About"><option value="">About nothing in particular</option>{#each data?.incidents ?? [] as i}<option value={i.id}>{i.exercise ? 'Exercise: ' : ''}{i.title}</option>{/each}{#each data?.exercises ?? [] as e}<option value={e.id}>Practice: {e.title}</option>{/each}</select>
    <button class="btn primary" disabled={!who || !arTitle.trim()} onclick={openReview} title={who ? '' : 'Write your name at the top first'}>Open a review</button>
  </div>
</div>
{/if}
