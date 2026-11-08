<script lang="ts">
  // Security Operations > Planning, Personnel and Records: the communications side (docs/46, plan COPS-COM-001).
  // Planning: the channel plan and radio coverage tests. Personnel: the twelve radio skills. Records: the communications log.
  // Every line written here stays in the file. A mistake is answered by a later line, never an edit.
  import { onMount } from 'svelte';
  import {
    securityOverview, securityChannelsGet, securityChannelsSave, securityCoverageList, securityCoverageAdd,
    securityTrainingList, securityTrainingRecord, securityCommsList, securityCommsAdd, securityWeatherList, securityWeatherSet, securityExportRecords,
    type WeatherList,
    type ChannelRow, type CoverageList, type TrainingList, type CommsList, type CommsEntry,
  } from '../lib/api';

  let { part, site = $bindable(''), me = '' }: { part: 'planning' | 'personnel' | 'records'; site: string; me: string } = $props();
  let sites: { id: string; name: string }[] = $state([]);
  let note = $state('');
  const who = $derived(me.trim() || 'Base');

  // ---- planning ----
  let channels: ChannelRow[] = $state([]);
  let isDefault = $state(true);
  let cov: CoverageList | null = $state(null);
  let wx: WeatherList | null = $state(null);
  // ---- personnel ----
  let train: TrainingList | null = $state(null);
  // ---- records ----
  let log: CommsList | null = $state(null);

  async function load() {
    try {
      const ov = await securityOverview();
      sites = ov.sites.map((s) => ({ id: s.id, name: s.name }));
      if (!sites.some((s) => s.id === site)) site = sites[0]?.id ?? '';
      if (part === 'planning' && site) {
        const c = await securityChannelsGet(site); channels = c.channels; isDefault = c.is_default;
        cov = await securityCoverageList(site);
        wx = await securityWeatherList();
      }
      if (part === 'personnel') train = await securityTrainingList();
      if (part === 'records') log = await securityCommsList(site);
    } catch (e) { note = String(e); }
  }
  onMount(load);
  $effect(() => { site; part; load(); });
  async function act(fn: () => Promise<unknown>, ok = '') { try { await fn(); note = ok; await load(); } catch (e) { note = String(e); } }

  const when = (iso: string) => { const d = new Date(iso); return isNaN(+d) ? iso : d.toLocaleString([], { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }); };

  // planning: channel plan
  const ROLE: Record<string, string> = { primary: 'Primary net', alternate: 'Alternate net', work: 'Work net', weather: 'Weather (listen only)', other: 'Other' };
  const weatherNow = $derived(channels.find((c) => c.role === 'weather'));
  const pickWeather = (e: Event) => act(() => securityWeatherSet(site, (e.currentTarget as HTMLSelectElement).value), 'Saved. Test it on the radio you will really use.');
  async function exportRecords() { try { const where = await securityExportRecords(); note = `A readable copy was saved in your user folder: ${where}`; } catch (e) { note = String(e); } }
  function addChannel() { channels = [...channels, { channel: '', code: 'off', role: 'other', purpose: '' }]; }
  function dropChannel(i: number) { channels = channels.filter((_, k) => k !== i); }
  const saveChannels = () => act(() => securityChannelsSave(site, channels), 'The channel plan is saved. Write the new channels on the paper card too.');

  // planning: coverage test
  let cPlace = $state(''); let cPi = $state(''); let cPo = $state(''); let cAi = $state(''); let cAo = $state(''); let cNote = $state('');
  const addTest = () => act(async () => {
    await securityCoverageAdd(site, cPlace, cPi, cPo, cAi, cAo, who, cNote);
    cPlace = cPi = cPo = cAi = cAo = cNote = '';
  }, 'Saved. A weak place also shows under "Needs a look" on Command.');
  const cell = (v: number | null) => (v === null ? '–' : String(v));
  const SCALE = ['0 no usable contact', '1 barely understandable', '2 unreliable, many repeats', '3 understandable with some repeats', '4 minor noise', '5 clear'];

  // personnel: training
  let tPerson = $state(''); let tSkill = $state(1); let tResult: 'pass' | 'practice' = $state('pass'); let tBy = $state(''); let tNote = $state('');
  const addTraining = () => act(async () => { await securityTrainingRecord(tPerson, tSkill, tResult, tBy, tNote); tNote = ''; }, 'Saved.');

  // records: log
  let lCat = $state('watch-turnover'); let lStation = $state(''); let lEvent = $state(''); let lAction = $state(''); let lStatus: 'open' | 'complete' = $state('complete');
  const addLog = () => act(async () => { await securityCommsAdd(site, lCat, lStation, lEvent, lAction, lStatus, '', who); lEvent = lAction = ''; }, 'Written in the log.');
  let closing: string | null = $state(null); let closeText = $state('');
  const closeEntry = (e: CommsEntry) => act(async () => { await securityCommsAdd(site, e.category, e.station, closeText, '', 'closed', e.id, who); closing = null; closeText = ''; }, 'Closed.');
  const label = (id: string) => log?.categories.find((c) => c.id === id)?.label ?? id;
  const isOpen = (e: CommsEntry) => !!log?.open.includes(e.id);
</script>

{#if note}<p class="note" role="status">{note}</p>{/if}

{#if part !== 'personnel'}
  <div class="row pick">
    <label class="label" for="cs">Site</label>
    <select id="cs" class="input" bind:value={site}>{#each sites as s}<option value={s.id}>{s.name}</option>{/each}</select>
  </div>
{/if}
{#if part !== 'personnel' && !site}
  <div class="panel"><p class="muted">Add a site on the Command tab first.</p></div>
{:else if part === 'planning'}
  <div class="panel">
    <h2>Radio channels</h2>
    <p class="muted">{isDefault ? 'These are the standard plan\'s channels. Save them to make them this site\'s own.' : 'This site\'s channel plan.'} Privacy codes stay off on the primary net. A privacy code is not encryption.</p>
    {#each channels as c, i}
      <div class="row ch">
        <select class="input" bind:value={c.role}>{#each Object.entries(ROLE) as [k, v]}<option value={k}>{v}</option>{/each}</select>
        <input class="input short" placeholder="Channel" bind:value={c.channel} aria-label="Channel" />
        <input class="input short" placeholder="Code" bind:value={c.code} aria-label="Privacy code" />
        <input class="input grow" placeholder="What it is for" bind:value={c.purpose} aria-label="Purpose" />
        <button class="btn" onclick={() => dropChannel(i)} title="Take this channel out of the plan">Remove</button>
      </div>
    {/each}
    <div class="acts"><button class="btn" onclick={addChannel}>Add a channel</button> <button class="btn primary" onclick={saveChannels}>Save the channel plan</button></div>
  </div>

  <div class="panel">
    <h2>Weather station</h2>
    <p class="muted">NOAA Weather Radio can be listened to on most hand radios and costs no battery to transmit. Pick the station for this site.</p>
    <select class="input" onchange={pickWeather} aria-label="Weather station" value={weatherNow ? (wx?.stations.find((x) => x.mhz === weatherNow.channel)?.call_sign ?? '') : ''}>
      <option value="">None chosen</option>
      {#each wx?.stations ?? [] as st}<option value={st.call_sign}>{st.place} · {st.call_sign} · {st.mhz} MHz</option>{/each}
    </select>
    {#if wx}<p class="small muted">{wx.note}</p>{/if}
  </div>

  <div class="panel">
    <h2>Radio coverage tests</h2>
    <p class="muted">Rate how well you could hear and be heard, from 0 to 5. Where radio is weak (2 or less), people there need another way to check in. Do not count on the radio working in an emergency.</p>
    <p class="small muted">{SCALE.join(' · ')}</p>
    {#if cov && cov.places.length}
      <table class="grid"><thead><tr><th>Place</th><th>Primary heard</th><th>Primary sent</th><th>Alternate heard</th><th>Alternate sent</th><th>Tested</th><th>Note</th></tr></thead><tbody>
        {#each cov.places as p}
          <tr class:weak={[p.p_in, p.p_out, p.a_in, p.a_out].some((v) => v !== null && v <= 2)}><td>{p.place}</td><td>{cell(p.p_in)}</td><td>{cell(p.p_out)}</td><td>{cell(p.a_in)}</td><td>{cell(p.a_out)}</td><td>{when(p.at)} by {p.by}</td><td>{p.note}</td></tr>
        {/each}
      </tbody></table>
      <p class="small muted">The newest test of each place is shown. Older tests stay in the file.</p>
    {:else}<p class="muted">No tests yet. Try the base, each boundary, the work area and the vehicles.</p>{/if}
    <div class="row ch">
      <input class="input grow" placeholder="Place (North boundary)" bind:value={cPlace} aria-label="Place" />
      <input class="input short" placeholder="Pri heard" bind:value={cPi} aria-label="Primary heard" inputmode="numeric" />
      <input class="input short" placeholder="Pri sent" bind:value={cPo} aria-label="Primary sent" inputmode="numeric" />
      <input class="input short" placeholder="Alt heard" bind:value={cAi} aria-label="Alternate heard" inputmode="numeric" />
      <input class="input short" placeholder="Alt sent" bind:value={cAo} aria-label="Alternate sent" inputmode="numeric" />
      <input class="input grow" placeholder="Note" bind:value={cNote} aria-label="Note" />
      <button class="btn primary" disabled={!cPlace.trim()} onclick={addTest}>Save the test</button>
    </div>
  </div>
{:else if part === 'personnel'}
  <div class="panel">
    <h2>Radio skills</h2>
    <p class="muted">Each person shows these twelve skills to someone else, who signs them off. Practise with a flat battery, a busy channel and poor reception. The newest line for a skill counts, so a skill that goes badly can be marked "needs practice" and passed later.</p>
    {#if train && train.people.length}
      <div class="scroll"><table class="grid"><thead><tr><th>Person</th>{#each train.skills as s, i}<th title={s}>{i + 1}</th>{/each}<th>All twelve</th></tr></thead><tbody>
        {#each train.people as p}
          <tr><td>{p.person}</td>{#each p.skills as v}<td class:ok={v === true} class:weak={v === false}>{v === true ? '✓' : v === false ? '!' : ''}</td>{/each}<td>{p.qualified ? 'Yes' : 'Not yet'}</td></tr>
        {/each}
      </tbody></table></div>
      <ol class="small muted">{#each train.skills as s}<li>{s}</li>{/each}</ol>
    {:else}<p class="muted">Nobody is tracked yet. The program only mentions people it has a record for.</p>{/if}
    <div class="row ch">
      <input class="input" placeholder="Whose skill" bind:value={tPerson} aria-label="Person" />
      <select class="input" bind:value={tSkill} aria-label="Skill">{#each train?.skills ?? [] as s, i}<option value={i + 1}>{i + 1}. {s}</option>{/each}</select>
      <select class="input" bind:value={tResult} aria-label="Result"><option value="pass">Showed it</option><option value="practice">Needs practice</option></select>
      <input class="input" placeholder="Who watched (not the same person)" bind:value={tBy} aria-label="Signed off by" />
      <input class="input grow" placeholder="Note" bind:value={tNote} aria-label="Note" />
      <button class="btn primary" disabled={!tPerson.trim() || !tBy.trim()} onclick={addTraining}>Sign off</button>
    </div>
  </div>
{:else}
  <div class="panel">
    <h2>Copy of the records</h2>
    <p class="muted">Writes incidents, outings and patrols, the communications log, coverage tests, inspections and availability into one readable text file you can print or hand over. The logs themselves are not changed.</p>
    <button class="btn" onclick={exportRecords}>Make a readable copy</button>
  </div>
  <div class="panel">
    <h2>Communications log</h2>
    <p class="muted">Write down significant calls and problems, not every ordinary conversation. An entry marked open stays under "Needs a look" until someone closes it with a line saying what happened.</p>
    <div class="row ch">
      <select class="input" bind:value={lCat} aria-label="Category">{#each log?.categories ?? [] as c}<option value={c.id}>{c.label}</option>{/each}</select>
      <input class="input short" placeholder="Station" bind:value={lStation} aria-label="Station" />
      <input class="input grow" placeholder="What happened" bind:value={lEvent} aria-label="Event" />
      <input class="input grow" placeholder="What was done" bind:value={lAction} aria-label="Action" />
      <select class="input" bind:value={lStatus} aria-label="Status"><option value="complete">Done</option><option value="open">Still open</option></select>
      <button class="btn primary" disabled={!lEvent.trim()} onclick={addLog}>Write it down</button>
    </div>
    {#if log && log.unreadable_lines > 0}<p class="note">Some lines in the log could not be read. They were left exactly as they are.</p>{/if}
    {#if log && log.entries.length}
      <table class="grid"><thead><tr><th>Time</th><th>Station</th><th>Type</th><th>Event</th><th>Action</th><th>Status</th><th></th></tr></thead><tbody>
        {#each log.entries as e (e.id)}
          <tr class:weak={isOpen(e)}><td>{when(e.at)}</td><td>{e.station}</td><td>{label(e.category)}</td><td>{e.event}</td><td>{e.action}</td>
            <td>{e.status === 'closed' ? `Closes ${e.closes}` : isOpen(e) ? 'Open' : e.status === 'open' ? 'Closed later' : 'Done'}</td>
            <td>{#if isOpen(e)}<button class="btn" onclick={() => { closing = e.id; closeText = ''; }}>Close</button>{/if}</td></tr>
          {#if closing === e.id}
            <tr><td colspan="7"><div class="row"><input class="input grow" placeholder="How it ended" bind:value={closeText} aria-label="How it ended" /> <button class="btn primary" disabled={!closeText.trim()} onclick={() => closeEntry(e)}>Close it</button> <button class="btn" onclick={() => (closing = null)}>Cancel</button></div></td></tr>
          {/if}
        {/each}
      </tbody></table>
    {:else}<p class="muted">Nothing written yet.</p>{/if}
  </div>
{/if}

<style>
  .pick { margin-bottom: 10px; align-items: center; gap: 8px; }
  .ch { gap: 8px; flex-wrap: wrap; align-items: center; margin: 8px 0; }
  .short { width: 90px; }
  .grow { flex: 1; min-width: 160px; }
  .acts { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 8px; }
  .grid { width: 100%; border-collapse: collapse; font-size: 13px; margin-top: 8px; }
  .grid th, .grid td { text-align: left; padding: 5px 8px; border-top: 1px solid var(--line); vertical-align: top; }
  .grid tr.weak td { background: var(--danger-bg, transparent); }
  .grid td.ok { color: var(--ok-text, var(--ok)); text-align: center; }
  .grid td.weak { text-align: center; }
  .scroll { overflow-x: auto; }
  .small { font-size: 12px; }
  .note { padding: 8px 10px; border-radius: 8px; background: var(--panel); }
</style>
