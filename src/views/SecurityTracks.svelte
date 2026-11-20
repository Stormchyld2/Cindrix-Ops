<script lang="ts">
  // Security Operations > Operations: GPS tracks sent by phones and kept by the host (core/src/tracksheet.rs, docs/45).
  import { onMount, onDestroy } from 'svelte';
  import { securityTracksList, securityTrackExport, securityTrackRecStatus, securityTrackRecStart, securityTrackRecPoint, securityTrackRecStop, securityTrackRecDiscard, type TrackRow } from '../lib/api';

  let tracks = $state<TrackRow[]>([]);
  let note = $state('');
  async function load() { try { tracks = (await securityTracksList()).tracks; } catch (e) { note = String(e); } }
  onMount(() => { load(); refreshRec(); });
  onDestroy(() => { if (watchId !== null) navigator.geolocation.clearWatch(watchId); });

  // Recording a track on this device (core/src/trackrec.rs). Each reading is saved at once, so a crash loses almost nothing.
  let recId = $state('');
  let recPoints = $state(0);
  let recHint = $state<string | null>(null);
  let who = $state('');
  let recNote = $state('');
  let watchId: number | null = null;
  async function refreshRec() {
    try {
      const st = await securityTrackRecStatus();
      if (st.recording) { recId = st.id; recPoints = st.points; recHint = st.hint; if (!who) who = st.person; } else { recId = ''; recPoints = 0; recHint = null; }
    } catch (e) { recNote = String(e); }
  }
  function listen() {
    if (!('geolocation' in navigator)) { recNote = 'This device cannot tell the program where it is. You can still keep the recording, but no points will be added.'; return; }
    watchId = navigator.geolocation.watchPosition(
      async (pos) => {
        if (!recId) return;
        try {
          const c = pos.coords;
          const r = await securityTrackRecPoint(recId, c.latitude, c.longitude, c.altitude ?? null, c.accuracy ?? null, null);
          recNote = r.kept ? '' : r.why;
          await refreshRec();
        } catch (e) { recNote = String(e); }
      },
      () => { recNote = 'The device could not find where it is. Check that location is switched on and allowed for this program.'; },
      { enableHighAccuracy: true, maximumAge: 5000, timeout: 30000 },
    );
  }
  function unlisten() { if (watchId !== null) { navigator.geolocation.clearWatch(watchId); watchId = null; } }
  async function startRec() {
    try { recId = await securityTrackRecStart(who, ''); recNote = ''; listen(); await refreshRec(); } catch (e) { recNote = String(e); }
  }
  async function stopRec() {
    unlisten();
    try { await securityTrackRecStop(recId); note = 'The track is kept in the list below.'; recNote = ''; } catch (e) { recNote = String(e); }
    await refreshRec(); await load();
  }
  async function dropRec() {
    unlisten();
    try { await securityTrackRecDiscard(recId); recNote = 'The recording was thrown away.'; } catch (e) { recNote = String(e); }
    await refreshRec();
  }

  const miles = (m: number) => (m / 1609.344).toFixed(2);
  const feet = (m: number) => Math.round(m * 3.28084);
  const span = (s: number) => { const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60); return h ? `${h} h ${m} min` : `${m} min`; };
  const when = (t: number) => new Date(t * 1000).toLocaleString([], { weekday: 'short', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
  async function copyOut(t: TrackRow) {
    try { note = `Copied to ${await securityTrackExport(t.id)} as a GPX file. It is a plain copy, so keep it somewhere safe.`; } catch (e) { note = String(e); }
  }
</script>

<div class="panel">
  <h2>Record a track</h2>
  <p class="muted small">Records where this device goes. Weak signals are left out, and the line is saved as you go. It needs a device that can tell where it is.</p>
  {#if recId}
    <p><b>Recording</b> for {who || 'someone'}: {recPoints} points so far.</p>
    {#if recHint}<p class="small" role="status">{recHint}</p>{/if}
    <button class="btn" onclick={stopRec} title="Stop and keep this track">Stop and keep</button>
    <button class="btn" onclick={dropRec} title="Throw this recording away">Throw away</button>
    {#if watchId === null}<button class="btn" onclick={listen} title="Start listening for the device's position again after a restart">Carry on recording</button>{/if}
  {:else}
    <label class="small">Who is walking?
      <input type="text" bind:value={who} maxlength="80" placeholder="A name" />
    </label>
    <button class="btn" onclick={startRec} disabled={!who.trim()} title="Begin recording where this device goes">Start recording</button>
  {/if}
  {#if recNote}<p class="small" role="status">{recNote}</p>{/if}
</div>

<div class="panel">
  <h2>Walked tracks</h2>
  <p class="muted small">Tracks recorded by a phone's GPS. They arrive in Command and wait there until you accept them.</p>
  {#if note}<p class="small" role="status">{note}</p>{/if}
  {#if !tracks.length}
    <p class="muted">No tracks kept yet.</p>
  {:else}
    {#each tracks as t (t.id)}
      <div class="trk">
        <div class="grow"><b>{t.person}</b> · {when(t.start)}<br />
          <span class="small">{miles(t.metres)} miles · {span(t.seconds)} · up {feet(t.climb)} ft, down {feet(t.drop)} ft{#if t.high !== null && t.low !== null} · {feet(t.low)} to {feet(t.high)} ft above sea level{/if} · {t.points} points</span></div>
        <button class="btn" onclick={() => copyOut(t)} title="Save this track as a GPX file other map programs can open">Copy out as GPX</button>
      </div>
    {/each}
  {/if}
</div>

<style>
  .trk { display: flex; gap: 10px; flex-wrap: wrap; align-items: center; padding: 8px 0; border-top: 1px solid var(--line); }
  .grow { flex: 1; min-width: 240px; }
</style>
