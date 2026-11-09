<script lang="ts">
  // Security Operations > Command (docs/44, mockups/SecurityCommand.html). Reads one summary from the program;
  // the posture shown is always one a person set. "Unknown" is where a site starts and never means safe.
  import { onMount, onDestroy } from 'svelte';
  import {
    securityOverview, securityWatchStrip, securitySiteSave, securityPostureSet,
    securityIncidentOpen, securityIncidentEntry, securityIncidentList,
    securityPocketList, securityPocketAccept, securityPocketSetAside, securityPocketSheet, type PocketReport, type CountSheet,
    type SecurityOverview, type WatchStrip, type SecurityIncident, type SecuritySite,
  } from '../lib/api';

  let { site = $bindable(''), me = $bindable('') }: { site: string; me: string } = $props();
  let ov = $state<SecurityOverview | null>(null);
  let strip = $state<WatchStrip | null>(null);
  let incidents: SecurityIncident[] = $state([]);
  let note = $state('');
  let reports: PocketReport[] = $state([]);
  let sheets: Record<string, CountSheet> = $state({}); // count sheets the host has opened to look at
  let timer: ReturnType<typeof setInterval> | undefined;

  function rememberMe() { try { localStorage.setItem('securityName', me); } catch { /* a private window: just ask each time */ } }

  async function refresh() {
    try {
      ov = await securityOverview();
      if (!ov.sites.some((s) => s.id === site)) site = ov.sites[0]?.id ?? '';
      strip = site ? await securityWatchStrip(site) : null;
      incidents = (await securityIncidentList()).incidents;
      reports = ov.pocket_waiting > 0 ? await securityPocketList() : [];
    } catch (e) { note = String(e); }
  }
  onMount(() => { refresh(); timer = setInterval(refresh, 30000); });
  onDestroy(() => { if (timer) clearInterval(timer); });
  async function pickSite(id: string) { site = id; strip = id ? await securityWatchStrip(id) : null; }

  const cur: SecuritySite | undefined = $derived(ov?.sites.find((s) => s.id === site));
  const siteIncidents = $derived(incidents.filter((i) => i.site === site));
  const liveHere = $derived(siteIncidents.filter((i) => !i.exercise && i.status !== 'closed'));

  const WORDS: Record<string, string> = { normal: 'Normal', elevated: 'Elevated', emergency: 'Emergency', unknown: 'Unknown' };
  const MEANS: Record<string, string> = {
    normal: 'Someone set this site to routine. That is a decision, not a promise that nothing is wrong.',
    elevated: 'Extra attention: more checks than usual.',
    emergency: 'A serious situation is being handled.',
    unknown: 'No one has set this yet, or it could not be checked. Unknown does not mean safe.',
  };
  const time = (iso: string) => { const d = new Date(iso); return isNaN(+d) ? '' : d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' }); };
  const when = (iso: string) => { const d = new Date(iso); return isNaN(+d) ? '' : d.toLocaleString([], { weekday: 'short', hour: 'numeric', minute: '2-digit' }); };

  // What needs a look comes from the program itself, so this list and the Dashboard count always agree.
  const looks = $derived(cur?.looks ?? []);
  const extra = $derived([...(ov && ov.unreadable_lines > 0 ? ['Some lines in the security files could not be read. They were left exactly as they are.'] : []), ...(ov && ov.pocket_waiting > 0 ? [`${ov.pocket_waiting} report${ov.pocket_waiting === 1 ? '' : 's'} from phones ${ov.pocket_waiting === 1 ? 'is' : 'are'} waiting for you.`] : [])]);

  // ---- reports waiting from Pocket phones ----
  async function acceptReport(r: PocketReport) {
    try { const n = await securityPocketAccept(r.name); note = n ? `Added ${n} line${n === 1 ? '' : 's'} from ${r.person}'s phone to the records.` : 'Those lines were already in the records.'; await refresh(); } catch (e) { note = String(e); }
  }
  async function lookAtSheet(r: PocketReport) {
    if (sheets[r.name]) { const { [r.name]: _gone, ...rest } = sheets; sheets = rest; return; }
    try { sheets = { ...sheets, [r.name]: await securityPocketSheet(r.name) }; } catch (e) { note = String(e); }
  }
  async function acceptSheet(r: PocketReport) {
    try {
      const n = await securityPocketAccept(r.name, r.changed > 0);
      note = n ? `Changed ${n} number${n === 1 ? '' : 's'} in the Inventory from ${r.person}'s count.` : 'The Inventory already had those numbers.';
      await refresh();
    } catch (e) { note = String(e); }
  }
  async function asideReport(r: PocketReport) {
    try { await securityPocketSetAside(r.name); note = 'Set aside. It is kept in the set-aside folder, not deleted.'; await refresh(); } catch (e) { note = String(e); }
  }

  // ---- adding a site ----
  let newName = $state('');
  async function addSite() {
    try { const id = await securitySiteSave({ name: newName }); newName = ''; note = ''; site = id; await refresh(); } catch (e) { note = String(e); }
  }

  // ---- setting the posture ----
  let postureDlg: HTMLDialogElement | undefined = $state();
  let pPosture: SecuritySite['posture'] = $state('elevated');
  let pReason = $state('');
  let pReview = $state(4);
  function openPosture() { pPosture = cur && cur.posture !== 'unknown' ? cur.posture : 'normal'; pReason = ''; pReview = 4; postureDlg?.showModal(); }
  async function savePosture() {
    if (!me.trim()) { note = 'Type your name first, so the record says who set it.'; return; }
    if (pPosture === 'emergency' && !confirm('Set this site to Emergency?')) return;
    try { rememberMe(); await securityPostureSet(site, pPosture, me, pReason, pReview); postureDlg?.close(); note = ''; await refresh(); } catch (e) { note = String(e); }
  }

  // ---- incidents ----
  let incDlg: HTMLDialogElement | undefined = $state();
  let iTitle = $state('');
  let iPriority = $state(2);
  let iExercise = $state(false);
  async function openIncident() {
    if (!me.trim()) { note = 'Type your name first, so the record says who wrote it.'; return; }
    try { rememberMe(); await securityIncidentOpen(site, iTitle, iPriority, iExercise, me); incDlg?.close(); iTitle = ''; iExercise = false; note = ''; await refresh(); } catch (e) { note = String(e); }
  }
  let shown = $state<string | null>(null);
  let words = $state('');
  let reason = $state('');
  const NEXT: Record<string, string[]> = { draft: ['active', 'closed'], active: ['stabilized', 'closed'], stabilized: ['active', 'closed'], closed: ['reopen'] };
  async function addNote(i: SecurityIncident) {
    if (!me.trim()) { note = 'Type your name first.'; return; }
    try { rememberMe(); await securityIncidentEntry({ incident: i.id, kind: 'note', text: words, source: 'observed', by: me }); words = ''; note = ''; await refresh(); } catch (e) { note = String(e); }
  }
  async function move(i: SecurityIncident, to: string) {
    if (!me.trim()) { note = 'Type your name first.'; return; }
    try { rememberMe(); await securityIncidentEntry({ incident: i.id, kind: 'status', arg1: to, text: reason, source: 'observed', by: me }); reason = ''; note = ''; await refresh(); } catch (e) { note = String(e); }
  }
  const SOURCE: Record<string, string> = { observed: 'seen', reported: 'told', imported: 'imported', corrected: 'fix' };
</script>

{#if note}<div class="warn" role="status">{note} <button class="btn" onclick={() => (note = '')}>OK</button></div>{/if}

{#if ov && ov.sites.length === 0}
  <div class="panel first">
    <h2>Add your first site</h2>
    <p class="muted">A site is a place you keep watch over: Home, a campsite, a place you would go to. Each one has its own posture, watch and incidents. Until you set a posture, a site shows <b>Unknown</b>, which does not mean safe.</p>
    <div class="row">
      <input class="input" placeholder="Name, for example Home" bind:value={newName} onkeydown={(e) => e.key === 'Enter' && addSite()} />
      <button class="btn primary" onclick={addSite}>Add site</button>
    </div>
  </div>
{:else if ov && cur}
  <div class="top row">
    <label class="label" for="site">Site</label>
    <select id="site" class="input" value={site} onchange={(e) => pickSite(e.currentTarget.value)}>
      {#each ov.sites as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
    </select>
    <span class="grow"></span>
    <details class="addsite"><summary class="btn">Add a site</summary>
      <div class="row"><input class="input" placeholder="Name" bind:value={newName} onkeydown={(e) => e.key === 'Enter' && addSite()} /><button class="btn" onclick={addSite}>Add</button></div>
    </details>
  </div>

  <section class={'banner ' + cur.posture} aria-live="polite">
    <div>
      <div class="label">{cur.name} posture</div>
      <div class="big">{WORDS[cur.posture]}{#if cur.check_due}<span class="badge due">Check due</span>{/if}</div>
      <p class="means">{MEANS[cur.posture]}</p>
      {#if cur.set_by}<p class="by">Set by {cur.set_by} at {when(cur.at)}{cur.reason ? ': ' + cur.reason : ''}</p>{/if}
    </div>
    <button class="btn primary" onclick={openPosture}>Set posture</button>
  </section>

  <div class="tiles">
    <div class="tile" style="--c: var(--cat-security)"><div class="label">On watch now</div><div class="num">{cur.on_watch.length ? cur.on_watch.join(', ') : 'No one'}</div><div class="small muted">{cur.on_watch.length ? 'Acknowledged and on duty' : 'Nobody has acknowledged a watch right now'}</div></div>
    <div class="tile" style="--c: var(--cat-ops)"><div class="label">Needs a look</div><div class="num">{looks.length + extra.length}</div><div class="small muted">{looks.length + extra.length ? 'See the list below' : 'Nothing waiting'}</div></div>
    <div class="tile" style="--c: var(--cat-comms)"><div class="label">Live incidents here</div><div class="num">{liveHere.length}</div><div class="small muted">Practice drills are not counted</div></div>
    <div class="tile" style="--c: var(--cat-intel)"><div class="label">All sites</div><div class="num">{WORDS[ov.headline]}</div><div class="small muted">{ov.all_normal_confirmed ? 'Every site is set to normal' : ov.unknown ? `${ov.unknown} not set` : 'Worst case across sites'}</div></div>
  </div>

  <div class="panel">
    <h2>Watch today</h2>
    {#if strip && strip.segments.length}
      {@const t0 = Date.parse(strip.from)}
      {@const span = Date.parse(strip.to) - t0}
      <div class="strip" role="img" aria-label="Today's watch coverage">
        {#each strip.segments as s}
          <div class={'seg ' + s.cover} style={`left:${((Date.parse(s.from) - t0) / span) * 100}%;width:${((Date.parse(s.to) - Date.parse(s.from)) / span) * 100}%`} title={`${time(s.from)} to ${time(s.to)}: ${s.cover === 'gap' ? 'nobody' : s.cover}`}></div>
        {/each}
      </div>
      <div class="axis"><span>12 am</span><span>6 am</span><span>12 pm</span><span>6 pm</span><span>12 am</span></div>
      <div class="key"><span><i class="k confirmed"></i>Acknowledged</span><span><i class="k planned"></i>Scheduled, not yet acknowledged</span><span><i class="k gap"></i>Nobody</span></div>
      {#if strip.watches.length}
        <ul class="plain">{#each strip.watches as w}<li>{time(w.start)} to {time(w.end)}: <b>{w.person}</b> <span class="muted">({w.state})</span></li>{/each}</ul>
      {:else}
        <p class="muted">No watches are written down for this site today. Planning the watchbill comes with the Operations tab.</p>
      {/if}
    {/if}
  </div>

  {#if reports.length}
    <div class="panel reports">
      <h2>Waiting for connection</h2>
      <p class="muted">Sent by a Pocket phone when it reached this computer. Nothing goes into the records until you accept it. Reports can add notes and new incidents, never change an incident's status. A count sheet only sets the numbers on items you already have; it never adds or deletes one. Check-ins only add to someone who is already out; a phone cannot start, end or cancel an outing.</p>
      {#each reports as r (r.name)}
        <div class="rep">
          {#if r.problem}
            <div><b>{r.name}</b> <span class="badge due">Cannot be used</span><div class="muted small">{r.problem}</div></div>
            <button class="btn" onclick={() => asideReport(r)}>Set aside</button>
          {:else if r.kind === 'count-sheet'}
            <div><b>{r.person}</b>'s phone, <b>inventory count</b>, {r.entries} item{r.entries === 1 ? '' : 's'}, counted {when(r.created)}
              {#if r.changed > 0}<div class="small"><span class="badge due">{r.changed} number{r.changed === 1 ? '' : 's'} changed here since the phone looked</span></div>{/if}</div>
            <div class="acts">
              <button class="btn" onclick={() => lookAtSheet(r)}>{sheets[r.name] ? 'Hide the numbers' : 'Look at the numbers'}</button>
              <button class="btn primary" onclick={() => acceptSheet(r)} title="Change the Inventory to the counted numbers. Nothing is deleted.">{r.changed > 0 ? 'Accept anyway' : 'Accept the counts'}</button>
              <button class="btn" onclick={() => asideReport(r)}>Set aside</button>
            </div>
            {#if sheets[r.name]}
              <table class="sheet"><thead><tr><th>Item</th><th>Where</th><th>Phone saw</th><th>Here now</th><th>Counted</th></tr></thead><tbody>
                {#each sheets[r.name].lines as ln}
                  <tr class:moved={ln.standing === 'moved'}><td>{ln.name || '(not in the Inventory)'}</td><td>{ln.location}</td><td>{ln.was}</td><td>{ln.now} {ln.unit}</td><td><b>{ln.counted}</b></td></tr>
                {/each}
              </tbody></table>
            {/if}
          {:else if r.kind === 'checkin-sheet'}
            <div><b>{r.person}</b>'s phone, <b>check-ins</b> for someone who is out, {r.entries} line{r.entries === 1 ? '' : 's'}, written {when(r.created)}</div>
            <div class="acts"><button class="btn primary" onclick={() => acceptReport(r)} title="Add these to the outing record as reported by the phone">Accept</button> <button class="btn" onclick={() => asideReport(r)}>Set aside</button></div>
          {:else}
            <div><b>{r.person}</b>'s phone, {r.entries} line{r.entries === 1 ? '' : 's'}, written {when(r.created)}</div>
            <div class="acts"><button class="btn primary" onclick={() => acceptReport(r)}>Accept</button> <button class="btn" onclick={() => asideReport(r)}>Set aside</button></div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  <div class="panel">
    <h2>Needs a look</h2>
    {#if looks.length || extra.length}<ul class="plain look">{#each [...looks, ...extra] as l}<li>{l}</li>{/each}</ul>{:else}<p class="muted">Nothing is waiting at this site.</p>{/if}
  </div>

  <div class="panel">
    <div class="row"><h2>Incidents</h2><span class="grow"></span><button class="btn" onclick={() => incDlg?.showModal()}>Open an incident</button></div>
    {#if siteIncidents.length === 0}<p class="muted">No incidents at this site.</p>{/if}
    {#each siteIncidents as i (i.id)}
      <div class="inc">
        <button class="head" onclick={() => (shown = shown === i.id ? null : i.id)} aria-expanded={shown === i.id}>
          <span class="badge st {i.status}">{i.status}</span>
          {#if i.exercise}<span class="badge ex">Practice</span>{/if}
          <b>{i.title}</b>
          <span class="muted">priority {i.priority}, opened {when(i.opened_at)} by {i.opened_by}</span>
        </button>
        {#if shown === i.id}
          <ol class="tl">
            {#each i.timeline as l}
              <li class:old={l.superseded_by}>
                <span class="muted">{when(l.happened_at)}{l.recorded_at !== l.happened_at ? ` (written ${when(l.recorded_at)})` : ''}</span>
                <span class="badge src">{SOURCE[l.source] ?? l.source}</span>
                <b>{l.kind === 'open' ? 'Opened' : l.kind === 'status' ? 'Now ' + l.arg1 : l.kind === 'plan' ? 'Plan activated' : l.kind}</b>
                {l.text}
                {#if l.superseded_by}<em class="muted"> (replaced by a later entry; kept)</em>{/if}
                <span class="muted"> {l.by}</span>
              </li>
            {/each}
          </ol>
          {#if i.plans.length}<p class="small muted">Plans followed: {i.plans.map((p) => `${p.plan} revision ${p.revision}`).join(', ')}</p>{/if}
          <div class="row">
            <input class="input grow" placeholder="Add a note to the timeline" bind:value={words} onkeydown={(e) => e.key === 'Enter' && addNote(i)} />
            <button class="btn" onclick={() => addNote(i)}>Add note</button>
          </div>
          <div class="row">
            <input class="input grow" placeholder={i.status === 'closed' ? 'Why is it being reopened?' : 'Reason (needed to close: what shows it is safe)'} bind:value={reason} />
            {#each NEXT[i.status] as to}<button class="btn" onclick={() => move(i, to)}>{to === 'reopen' ? 'Reopen' : 'Mark ' + to}</button>{/each}
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<dialog bind:this={postureDlg}>
  <div class="dlg">
    <h2>Set posture</h2>
    <p class="muted">This is your decision. It stays until someone sets it again; the program never lowers it by itself.</p>
    <label class="label" for="pp">Posture</label>
    <select id="pp" class="input" bind:value={pPosture}>
      <option value="normal">Normal</option><option value="elevated">Elevated</option><option value="emergency">Emergency</option><option value="unknown">Unknown</option>
    </select>
    <label class="label" for="pr">Why (needed for elevated and emergency)</label>
    <textarea id="pr" class="input" bind:value={pReason}></textarea>
    <label class="label" for="pv">Look at it again in</label>
    <select id="pv" class="input" bind:value={pReview}>
      <option value={0}>No reminder</option><option value={2}>2 hours</option><option value={4}>4 hours</option><option value={8}>8 hours</option><option value={24}>24 hours</option>
    </select>
    <div class="row"><button class="btn primary" onclick={savePosture}>Save</button><button class="btn" onclick={() => postureDlg?.close()}>Cancel</button></div>
  </div>
</dialog>

<dialog bind:this={incDlg}>
  <div class="dlg">
    <h2>Open an incident</h2>
    <p class="muted">Write down what is happening. It starts as a draft; you decide when it becomes active.</p>
    <label class="label" for="it">What is happening</label>
    <input id="it" class="input" bind:value={iTitle} placeholder="For example: Car stopped at the gate" />
    <label class="label" for="ip">Priority (1 is most serious)</label>
    <select id="ip" class="input" bind:value={iPriority}><option value={1}>1</option><option value={2}>2</option><option value={3}>3</option><option value={4}>4</option></select>
    <label class="check-row"><input type="checkbox" bind:checked={iExercise} /> This is a practice drill, not real</label>
    <div class="row"><button class="btn primary" onclick={openIncident}>Open</button><button class="btn" onclick={() => incDlg?.close()}>Cancel</button></div>
  </div>
</dialog>

<style>
  .panel { margin-bottom: 14px; }
  .panel h2 { font-size: 15px; margin-bottom: 8px; }
  .top { margin-bottom: 14px; }
  .grow { flex: 1; }
  .who { width: 160px; }
  .addsite summary { list-style: none; cursor: pointer; }
  .banner { display: flex; align-items: center; justify-content: space-between; gap: 16px; flex-wrap: wrap; padding: 18px 20px; border-radius: 12px; border: 1px solid var(--line2); border-left: 5px solid var(--muted); background: var(--panel); margin-bottom: 14px; }
  .banner.normal { border-left-color: var(--ok); background: var(--ok-bg); border-color: var(--ok-line); }
  .banner.elevated { border-left-color: var(--warn); background: var(--warn-soft); border-color: var(--warn-line); }
  .banner.emergency { border-left-color: var(--danger); background: var(--danger-bg); border-color: var(--danger-line); }
  .banner.unknown { border-left-style: dashed; }
  .banner .big { font-family: var(--display); text-transform: uppercase; letter-spacing: .12em; font-weight: 700; font-size: 30px; }
  .means, .by { margin: 4px 0 0; max-width: 620px; }
  .by { color: var(--muted); font-size: 13px; }
  .badge.due { margin-left: 12px; vertical-align: middle; }
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); gap: 12px; margin-bottom: 14px; }
  .tile { padding: 14px; border: 1px solid var(--line); border-left: 4px solid var(--c, var(--line)); border-radius: 12px; background: linear-gradient(180deg, color-mix(in srgb, var(--c, var(--line)) 9%, var(--panel)), var(--panel) 70%); }
  .tile .num { font-size: 22px; font-weight: 700; margin: 2px 0; overflow-wrap: anywhere; }
  .small { font-size: 12px; }
  .strip { position: relative; height: 26px; border-radius: 6px; background: var(--panel2); border: 1px solid var(--line); overflow: hidden; }
  .seg { position: absolute; top: 0; bottom: 0; }
  .seg.confirmed { background: var(--ok); }
  .seg.planned { background: repeating-linear-gradient(45deg, var(--warn) 0 6px, transparent 6px 12px); }
  .seg.gap { background: transparent; box-shadow: inset 0 0 0 1px var(--danger-line); }
  .axis { display: flex; justify-content: space-between; font-family: var(--mono); font-size: 11px; color: var(--muted); margin-top: 4px; }
  .key { display: flex; gap: 16px; flex-wrap: wrap; font-size: 12px; color: var(--muted); margin: 8px 0; }
  .key .k { display: inline-block; width: 12px; height: 12px; border-radius: 3px; margin-right: 6px; vertical-align: -2px; }
  .k.confirmed { background: var(--ok); } .k.planned { background: repeating-linear-gradient(45deg, var(--warn) 0 3px, transparent 3px 6px); } .k.gap { box-shadow: inset 0 0 0 1px var(--danger-line); }
  ul.plain { margin: 6px 0 0; padding-left: 18px; }
  ul.look li { color: var(--warn); margin: 3px 0; }
  .inc { border-top: 1px solid var(--line); padding: 8px 0; }
  .inc .head { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; width: 100%; background: none; border: 0; text-align: left; padding: 4px 0; }
  .badge.st.active { color: var(--danger-text); border-color: var(--danger-line); background: var(--danger-bg); }
  .badge.st.stabilized { color: var(--warn); }
  .badge.st.closed { color: var(--ok-text); border-color: var(--ok-line); background: var(--ok-bg); }
  .badge.ex, .badge.src { color: var(--muted); border-color: var(--line2); background: transparent; }
  .tl { list-style: none; margin: 8px 0; padding: 0; }
  .tl li { padding: 4px 0 4px 10px; border-left: 2px solid var(--line2); margin-bottom: 4px; }
  .tl li.old { opacity: .6; text-decoration: line-through; }
  .tl li.old em { text-decoration: none; }
  .inc .row { margin-top: 6px; }
  .dlg { padding: 18px; display: flex; flex-direction: column; gap: 8px; }
  .check-row { display: flex; gap: 8px; align-items: center; }
  .first { max-width: 640px; }
  .rep { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; padding: 8px 0; border-top: 1px solid var(--line); }
  .sheet { width: 100%; border-collapse: collapse; margin-top: 8px; font-size: 13px; }
  .sheet th, .sheet td { text-align: left; padding: 4px 8px; border-top: 1px solid var(--line); }
  .sheet tr.moved td { background: var(--danger-bg, transparent); }
</style>
