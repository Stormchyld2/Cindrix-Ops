<script lang="ts">
  // Security Operations > Operations: GPS tracks sent by phones and kept by the host (core/src/tracksheet.rs, docs/45).
  import { onMount } from 'svelte';
  import { securityTracksList, securityTrackExport, type TrackRow } from '../lib/api';

  let tracks = $state<TrackRow[]>([]);
  let note = $state('');
  async function load() { try { tracks = (await securityTracksList()).tracks; } catch (e) { note = String(e); } }
  onMount(load);

  const miles = (m: number) => (m / 1609.344).toFixed(2);
  const feet = (m: number) => Math.round(m * 3.28084);
  const span = (s: number) => { const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60); return h ? `${h} h ${m} min` : `${m} min`; };
  const when = (t: number) => new Date(t * 1000).toLocaleString([], { weekday: 'short', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
  async function copyOut(t: TrackRow) {
    try { note = `Copied to ${await securityTrackExport(t.id)} as a GPX file. It is a plain copy, so keep it somewhere safe.`; } catch (e) { note = String(e); }
  }
</script>

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
