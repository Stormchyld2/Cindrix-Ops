<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { mapsList, mapLoad, mapSave, mapDelete, mapExportGpx, routeStats, mapPacks, tileUrl, type MapMeta, type LatLon, type RouteStats, type Pack } from '../lib/api';
  import type { Map as MLMap, Marker, StyleSpecification, GeoJSONSource } from 'maplibre-gl';
  import { settings } from '../lib/settings.svelte';

  type Pin = { id: number; label: string; kind: 'pin' | 'rendezvous'; lat: number; lon: number };
  type Area = { label: string; points: LatLon[] };
  type MapFile = { version: 1; name: string; pins: Pin[]; route: number[]; nogo: Area[] };

  let saved: MapMeta[] = $state([]);
  let current: string | null = $state(null); // id of the saved map being edited, null for a new one
  let name = $state('');
  let notes = $state('');
  let pins: Pin[] = $state([]);
  let route: number[] = $state([]); // pin ids in order
  let nogo: Area[] = $state([]);
  let draft: LatLon[] = $state([]); // no-go area being drawn
  let draftLabel = $state('');
  let message = $state('');
  let error = $state('');
  /** What the open map file covers, read from its header. */
  let packInfo = $state('');
  let stats: RouteStats | null = $state(null);
  let confirmDelete = $state(false);

  // form fields
  let pLabel = $state('');
  let pKind: 'pin' | 'rendezvous' = $state('pin');
  let pLat = $state('');
  let pLon = $state('');
  let aLat = $state('');
  let aLon = $state('');

  // ---------- the tile map (only when a street pack is on the drive) ----------
  let packs: Pack[] = $state([]);
  let street: Pack | undefined = $derived(packs.find((p) => p.kind === 'street'));
  let mapEl: HTMLDivElement | undefined = $state();
  let mapReady = $state(false);
  let tileError = $state('');
  let addMode: 'off' | 'pin' | 'rendezvous' | 'corner' = $state('off');
  let map: MLMap | null = null;
  let markers: Marker[] = [];
  let ml: typeof import('maplibre-gl') | null = null;
  let protocol: import('pmtiles').Protocol | null = null;

  onMount(async () => {
    await refresh();
    try {
      packs = await mapPacks();
    } catch (e) {
      tileError = String(e);
    }
  });
  onDestroy(() => {
    map?.remove();
    map = null;
  });

  $effect(() => {
    if (street && mapEl && !map) void initMap(street, mapEl);
  });

  async function initMap(pack: Pack, el: HTMLDivElement) {
    try {
      const [maplibregl, { Protocol, PMTiles }, { layers, namedFlavor }] = await Promise.all([
        import('maplibre-gl'),
        import('pmtiles'),
        import('@protomaps/basemaps'),
      ]);
      await import('maplibre-gl/dist/maplibre-gl.css');
      ml = maplibregl;
      if (!protocol) {
        protocol = new Protocol();
        // The cast keeps this compiling whichever handler signature the two libraries agree on.
        maplibregl.addProtocol('pmtiles', protocol.tile as never);
      }
      const url = await tileUrl(pack.rel);
      // Read the file's header first: it says which area and zoom levels the pack really holds.
      // A pack cut for one town has nothing to show at the whole-country view the map used to open on.
      const archive = new PMTiles(url);
      protocol.add(archive);
      let area: { w: number; s: number; e: number; n: number; minZoom: number; maxZoom: number } | null = null;
      try {
        const h = await archive.getHeader();
        if ([h.minLon, h.minLat, h.maxLon, h.maxLat].every((v) => Number.isFinite(v)) && h.maxLon > h.minLon && h.maxLat > h.minLat) {
          area = { w: h.minLon, s: h.minLat, e: h.maxLon, n: h.maxLat, minZoom: h.minZoom, maxZoom: h.maxZoom };
          packInfo = `${pack.name}: covers ${h.minLat.toFixed(2)}, ${h.minLon.toFixed(2)} to ${h.maxLat.toFixed(2)}, ${h.maxLon.toFixed(2)}; zoom ${h.minZoom} to ${h.maxZoom}.`;
        }
      } catch (e) {
        tileError = 'The map file could not be read: ' + String((e as Error)?.message ?? e) + ' The file may be damaged or incomplete; cut it again with scripts\\get-maps.ps1.';
        return;
      }
      // No glyph (font) files ship yet, so the text layers are left out: roads, water, land and
      // buildings are drawn, place names are not. Pin labels are drawn by this screen instead.
      const style = {
        version: 8,
        sources: { protomaps: { type: 'vector', url: 'pmtiles://' + url, attribution: '© OpenStreetMap contributors' } },
        layers: layers('protomaps', namedFlavor('dark'), { lang: 'en' }).filter((l) => l.type !== 'symbol'),
      } as unknown as StyleSpecification;
      const m = new maplibregl.Map({
        container: el,
        style,
        center: area ? [(area.w + area.e) / 2, (area.s + area.n) / 2] : [-98.5, 39.8],
        zoom: area ? Math.max(area.minZoom, 3) : 3.5,
        attributionControl: { compact: true },
      });
      // Open on the area the pack covers (the whole US for a US pack, one town for a small test pack).
      if (area) m.fitBounds([[area.w, area.s], [area.e, area.n]], { padding: 20, duration: 0, maxZoom: Math.min(area.maxZoom, 15) });
      m.addControl(new maplibregl.NavigationControl({ showCompass: true }), 'top-right');
      m.addControl(new maplibregl.ScaleControl({ unit: settings.values.mapUnits === 'metric' ? 'metric' : 'imperial' }), 'bottom-left');
      m.on('error', (e) => {
        tileError = 'The map could not read its tiles: ' + String((e as { error?: Error }).error?.message ?? e);
      });
      m.on('load', () => {
        for (const id of ['route', 'nogo', 'draft']) m.addSource(id, { type: 'geojson', data: empty() });
        m.addLayer({ id: 'nogo-fill', type: 'fill', source: 'nogo', paint: { 'fill-color': '#e5533d', 'fill-opacity': 0.25 } });
        m.addLayer({ id: 'nogo-line', type: 'line', source: 'nogo', paint: { 'line-color': '#e5533d', 'line-width': 2, 'line-dasharray': [2, 2] } });
        m.addLayer({ id: 'draft-line', type: 'line', source: 'draft', paint: { 'line-color': '#e5533d', 'line-width': 2, 'line-dasharray': [1, 2] } });
        m.addLayer({ id: 'route-line', type: 'line', source: 'route', paint: { 'line-color': '#ff9a1f', 'line-width': 4 } });
        mapReady = true;
        zoomToPoints();
      });
      m.on('click', (e) => onMapClick(e.lngLat.lat, e.lngLat.lng));
      map = m;
    } catch (e) {
      tileError = 'The map viewer could not start: ' + String(e);
    }
  }

  const empty = () => ({ type: 'FeatureCollection' as const, features: [] });
  const ring = (pts: LatLon[]) => [...pts, pts[0]].map(([la, lo]) => [lo, la]);

  function onMapClick(lat: number, lon: number) {
    const p: LatLon = [Math.round(lat * 1e5) / 1e5, Math.round(lon * 1e5) / 1e5];
    if (addMode === 'pin' || addMode === 'rendezvous') {
      const id = pins.reduce((m, x) => Math.max(m, x.id), 0) + 1;
      pins = [...pins, { id, label: (addMode === 'rendezvous' ? 'Rendezvous ' : 'Pin ') + id, kind: addMode, lat: p[0], lon: p[1] }];
    } else if (addMode === 'corner') {
      draft = [...draft, p];
    }
  }

  function zoomToPoints() {
    if (!map || !ml) return;
    const all: LatLon[] = [...pins.map((p) => [p.lat, p.lon] as LatLon), ...nogo.flatMap((a) => a.points)];
    if (all.length === 0) return;
    const b = new ml.LngLatBounds([all[0][1], all[0][0]], [all[0][1], all[0][0]]);
    for (const [la, lo] of all) b.extend([lo, la]);
    map.fitBounds(b, { padding: 60, maxZoom: 15, duration: 0 });
  }

  // Keep the drawn layers and the pin markers in step with what is on screen.
  $effect(() => {
    if (!map || !ml || !mapReady) return;
    const m = map;
    const rp = routePoints;
    (m.getSource('route') as GeoJSONSource).setData(
      rp.length > 1
        ? { type: 'Feature', properties: {}, geometry: { type: 'LineString', coordinates: rp.map(([la, lo]) => [lo, la]) } }
        : empty(),
    );
    (m.getSource('nogo') as GeoJSONSource).setData({
      type: 'FeatureCollection',
      features: nogo.filter((a) => a.points.length >= 3).map((a) => ({ type: 'Feature' as const, properties: {}, geometry: { type: 'Polygon' as const, coordinates: [ring(a.points)] } })),
    });
    (m.getSource('draft') as GeoJSONSource).setData(
      draft.length > 1
        ? { type: 'Feature', properties: {}, geometry: { type: 'LineString', coordinates: draft.map(([la, lo]) => [lo, la]) } }
        : empty(),
    );
    for (const k of markers) k.remove();
    markers = pins.map((p) => {
      const el = document.createElement('div');
      el.className = 'pinmark';
      el.innerHTML = '<span class="pdot"></span><span class="plab"></span>';
      (el.querySelector('.pdot') as HTMLElement).style.background = pinColor(p.kind);
      (el.querySelector('.plab') as HTMLElement).textContent = p.label; // text, never markup
      return new ml!.Marker({ element: el, anchor: 'left' }).setLngLat([p.lon, p.lat]).addTo(m);
    });
  });

  async function refresh() {
    try {
      saved = await mapsList();
    } catch (e) {
      error = String(e);
    }
  }

  function parsePoint(lat: string, lon: string): LatLon | null {
    const a = Number(lat.trim());
    const b = Number(lon.trim());
    if (lat.trim() === '' || lon.trim() === '' || !Number.isFinite(a) || !Number.isFinite(b)) return null;
    if (a < -90 || a > 90 || b < -180 || b > 180) return null;
    return [a, b];
  }

  function addPin() {
    error = '';
    const p = parsePoint(pLat, pLon);
    if (!p) {
      error = 'Enter a latitude between -90 and 90 and a longitude between -180 and 180, as numbers (for example 39.4 and -80.1).';
      return;
    }
    const id = pins.reduce((m, x) => Math.max(m, x.id), 0) + 1;
    pins = [...pins, { id, label: pLabel.trim() || (pKind === 'rendezvous' ? 'Rendezvous ' + id : 'Pin ' + id), kind: pKind, lat: p[0], lon: p[1] }];
    pLabel = pLat = pLon = '';
  }
  function removePin(id: number) {
    pins = pins.filter((p) => p.id !== id);
    route = route.filter((r) => r !== id);
  }
  function toRoute(id: number) {
    route = [...route, id];
  }
  function addDraftPoint() {
    error = '';
    const p = parsePoint(aLat, aLon);
    if (!p) {
      error = 'Enter the corner as a latitude and a longitude.';
      return;
    }
    draft = [...draft, p];
    aLat = aLon = '';
  }
  function finishArea() {
    if (draft.length < 3) {
      error = 'An area needs at least three corners.';
      return;
    }
    nogo = [...nogo, { label: draftLabel.trim() || 'No-go area ' + (nogo.length + 1), points: draft }];
    draft = [];
    draftLabel = '';
    error = '';
  }

  let routePoints: LatLon[] = $derived(
    route.map((id) => pins.find((p) => p.id === id)).filter((p): p is Pin => !!p).map((p) => [p.lat, p.lon] as LatLon),
  );
  $effect(() => {
    const pts = routePoints;
    if (pts.length < 2) {
      stats = null;
      return;
    }
    routeStats(pts).then((s) => (stats = s)).catch(() => (stats = null));
  });

  function newMap() {
    current = null;
    name = notes = '';
    pins = [];
    route = [];
    nogo = [];
    draft = [];
    message = error = '';
    confirmDelete = false;
  }

  async function open(id: string) {
    try {
      const m = await mapLoad(id);
      const f = JSON.parse(m.json) as Partial<MapFile>;
      current = id;
      name = f.name ?? '';
      pins = f.pins ?? [];
      route = f.route ?? [];
      nogo = f.nogo ?? [];
      notes = m.notes;
      draft = [];
      message = error = '';
      confirmDelete = false;
      queueMicrotask(zoomToPoints);
    } catch (e) {
      error = 'Could not open that map: ' + String(e);
    }
  }

  async function save() {
    error = message = '';
    if (!name.trim()) {
      error = 'Give the map a name first.';
      return;
    }
    const file: MapFile = { version: 1, name: name.trim(), pins, route, nogo };
    try {
      current = await mapSave(current, name.trim(), JSON.stringify(file, null, 2), notes);
      message = 'Saved.';
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function exportGpx() {
    error = message = '';
    if (!current) {
      error = 'Save the map first, then export its route.';
      return;
    }
    try {
      const where = await mapExportGpx(current, name.trim() || 'Route', routePoints);
      message = 'Route written to ' + where;
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function remove() {
    if (!current) return;
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    try {
      await mapDelete(current);
      newMap();
      await refresh();
      message = 'Map deleted.';
    } catch (e) {
      error = String(e);
    }
  }

  // ---------- plan view: your points, drawn to scale, with no map underneath ----------
  const W = 640;
  const H = 400;
  const PAD = 36;

  let view = $derived.by(() => {
    const all: LatLon[] = [...pins.map((p) => [p.lat, p.lon] as LatLon), ...nogo.flatMap((a) => a.points), ...draft];
    if (all.length === 0) return null;
    const lats = all.map((p) => p[0]);
    const lons = all.map((p) => p[1]);
    const minLat = Math.min(...lats), maxLat = Math.max(...lats);
    const minLon = Math.min(...lons), maxLon = Math.max(...lons);
    const midLat = (minLat + maxLat) / 2;
    const kx = Math.cos((midLat * Math.PI) / 180); // a degree of longitude is shorter away from the equator
    const spanX = Math.max((maxLon - minLon) * kx, 1e-4);
    const spanY = Math.max(maxLat - minLat, 1e-4);
    const scale = Math.min((W - 2 * PAD) / spanX, (H - 2 * PAD) / spanY);
    const offX = (W - spanX * scale) / 2;
    const offY = (H - spanY * scale) / 2;
    const x = (lon: number) => offX + (lon - minLon) * kx * scale;
    const y = (lat: number) => H - offY - (lat - minLat) * scale;
    // scale bar: pixels per kilometre, then a round length that fits
    const kmPerDeg = 111.195;
    const pxPerKm = scale / kmPerDeg;
    const options = [0.1, 0.2, 0.5, 1, 2, 5, 10, 20, 50, 100, 200, 500, 1000];
    const km = [...options].reverse().find((k) => k * pxPerKm <= 150) ?? options[0];
    return { x, y, barPx: km * pxPerKm, barKm: km };
  });

  const pinColor = (k: string) => (k === 'rendezvous' ? '#6fbf73' : '#5aa9e6');
</script>

<section>
  <h1>Maps</h1>
  <p class="sub">Plan with your own points: places to meet, routes between them, and areas to avoid. Save the plan, print it, and export the route for a phone map app.</p>
</section>

{#if street}
  <div class="warn">
    <strong>Map: {street.name}</strong> ({street.mb} MB, from this drive).{#if packInfo} This pack covers {packInfo.replace(street.name + ': covers ', '')}{/if} Roads, water, land and buildings are drawn; place names are not yet. Positions you place are approximate, and routes are straight lines between your points: this app does not plan roads and does not yet steer routes around no-go areas.
  </div>
{:else}
  <div class="warn">
    <strong>No map tiles on this drive yet.</strong> Street and topo maps come from map packs (see doc 7), so for now this is a plan view of the points you enter, drawn to scale with nothing underneath. Positions are typed in by hand and are approximate. Routes are straight lines between your points; this app does not plan roads and does not yet steer routes around no-go areas.
  </div>
{/if}
{#if tileError}<div class="warn err" role="alert">{tileError}</div>{/if}

{#if error}<div class="warn err" role="alert">{error}</div>{/if}
{#if message}<div class="ok-note" role="status">{message}</div>{/if}

<div class="cols">
  <div class="panel side">
    <div class="label">My saved maps</div>
    <div class="list">
      {#each saved as m}
        <button class="item" class:on={m.id === current} onclick={() => open(m.id)}>
          {m.name}{#if m.has_route} <span class="muted">· route</span>{/if}
        </button>
      {:else}
        <span class="muted">Nothing saved yet.</span>
      {/each}
    </div>
    <button class="btn" onclick={newMap}>New map</button>
  </div>

  <div class="panel main">
    <div class="row">
      <input class="input grow" placeholder="Map name (for example: Home to the lake cabin)" bind:value={name} aria-label="Map name" />
      <button class="btn primary" onclick={save}>Save</button>
      <button class="btn" onclick={exportGpx} disabled={routePoints.length < 2}>Export route (GPX)</button>
      {#if current}<button class="btn danger" onclick={remove}>{confirmDelete ? 'Really delete?' : 'Delete'}</button>{/if}
    </div>

    {#if street}
      <div class="row">
        <span class="label">Click the map to add:</span>
        {#each [['off', 'Nothing'], ['pin', 'Pins'], ['rendezvous', 'Rendezvous points'], ['corner', 'No-go corners']] as [k, t]}
          <button class="btn" class:primary={addMode === k} onclick={() => (addMode = k as typeof addMode)}>{t}</button>
        {/each}
        <button class="btn" onclick={zoomToPoints} disabled={pins.length === 0 && nogo.length === 0}>Zoom to my points</button>
      </div>
      <div class="tilemap" bind:this={mapEl} role="application" aria-label="Map"></div>
    {:else}
      <svg viewBox="0 0 {W} {H}" role="img" aria-label="Plan view of your points" class="plan">
        <rect width={W} height={H} fill="#0f1110" />
        {#if view}
          {#each nogo as a}
            <polygon points={a.points.map((p) => view.x(p[1]) + ',' + view.y(p[0])).join(' ')} fill="#e5533d33" stroke="#e5533d" stroke-width="1.5" stroke-dasharray="5 4" />
          {/each}
          {#if draft.length > 0}
            <polyline points={draft.map((p) => view.x(p[1]) + ',' + view.y(p[0])).join(' ')} fill="none" stroke="#e5533d" stroke-width="1.5" stroke-dasharray="2 3" />
          {/if}
          {#if routePoints.length > 1}
            <polyline points={routePoints.map((p) => view.x(p[1]) + ',' + view.y(p[0])).join(' ')} fill="none" stroke="#ff9a1f" stroke-width="2.5" />
          {/if}
          {#each pins as p}
            <circle cx={view.x(p.lon)} cy={view.y(p.lat)} r="6" fill={pinColor(p.kind)} stroke="#0f1110" stroke-width="2" />
            <text x={view.x(p.lon) + 10} y={view.y(p.lat) + 4} fill="#ece7de" font-size="12">{p.label}</text>
          {/each}
          <line x1={PAD} y1={H - 14} x2={PAD + view.barPx} y2={H - 14} stroke="#b5ada0" stroke-width="2" />
          <text x={PAD + view.barPx + 8} y={H - 10} fill="#b5ada0" font-size="11">{settings.values.mapUnits === 'metric' ? `${view.barKm} km` : `${(view.barKm * 0.621371).toFixed(view.barKm < 1 ? 2 : 1)} mi`}</text>
          <text x={W - 24} y="22" fill="#b5ada0" font-size="12" text-anchor="middle">N ↑</text>
        {:else}
          <text x={W / 2} y={H / 2} fill="#9a9388" font-size="14" text-anchor="middle">Add a point below to start your plan.</text>
        {/if}
      </svg>
    {/if}

    {#if stats}
      <p class="stat"><strong>Route:</strong> {settings.values.mapUnits === 'metric' ? `${stats.km.toFixed(1)} km` : `${stats.miles.toFixed(1)} miles`} in a straight line through {routePoints.length} points. Real roads will be longer.</p>
    {/if}

    <div class="grid2">
      <div>
        <div class="label">Add a place</div>
        <div class="row">
          <input class="input" placeholder="Label" bind:value={pLabel} aria-label="Place label" />
          <select class="input" bind:value={pKind} aria-label="Kind of place">
            <option value="pin">Pin</option>
            <option value="rendezvous">Rendezvous point</option>
          </select>
        </div>
        <div class="row">
          <input class="input" placeholder="Latitude, e.g. 39.4" bind:value={pLat} aria-label="Latitude" inputmode="decimal" />
          <input class="input" placeholder="Longitude, e.g. -80.1" bind:value={pLon} aria-label="Longitude" inputmode="decimal" />
          <button class="btn" onclick={addPin}>Add</button>
        </div>
        <ul class="plain">
          {#each pins as p}
            <li>
              <span class="dot" style="background:{pinColor(p.kind)}"></span>
              {p.label} <span class="muted">{p.lat.toFixed(4)}, {p.lon.toFixed(4)}</span>
              <button class="link" onclick={() => toRoute(p.id)}>add to route</button>
              <button class="link" onclick={() => removePin(p.id)}>remove</button>
            </li>
          {/each}
        </ul>
        {#if route.length > 0}
          <p class="muted">Route order: {route.map((id) => pins.find((p) => p.id === id)?.label ?? '?').join(' → ')}
            <button class="link" onclick={() => (route = [])}>clear route</button></p>
        {/if}
      </div>

      <div>
        <div class="label">No-go areas</div>
        <p class="muted small">Corners of a flooded, blocked or unsafe area, in order around its edge.</p>
        <div class="row">
          <input class="input" placeholder="Area label" bind:value={draftLabel} aria-label="No-go area label" />
        </div>
        <div class="row">
          <input class="input" placeholder="Latitude" bind:value={aLat} aria-label="Corner latitude" inputmode="decimal" />
          <input class="input" placeholder="Longitude" bind:value={aLon} aria-label="Corner longitude" inputmode="decimal" />
          <button class="btn" onclick={addDraftPoint}>Add corner</button>
        </div>
        <div class="row">
          <span class="muted">{draft.length} corner{draft.length === 1 ? '' : 's'} so far</span>
          <button class="btn" onclick={finishArea} disabled={draft.length < 3}>Finish area</button>
          {#if draft.length > 0}<button class="link" onclick={() => (draft = [])}>start over</button>{/if}
        </div>
        <ul class="plain">
          {#each nogo as a, i}
            <li>{a.label} <span class="muted">{a.points.length} corners</span>
              <button class="link" onclick={() => (nogo = nogo.filter((_, j) => j !== i))}>remove</button></li>
          {/each}
        </ul>
      </div>
    </div>

    <div class="label notes-label">Notes</div>
    <textarea class="input" bind:value={notes} placeholder="Written notes for this map: gates, water on the way, who meets where." aria-label="Notes"></textarea>
    <p class="muted small">Map data comes from © OpenStreetMap contributors. Saved maps are plain files in the user/saved-maps folder.</p>
  </div>
</div>

<style>
  .cols { display: flex; flex-wrap: wrap; gap: 14px; align-items: flex-start; }
  .side { flex: 1 1 220px; max-width: 280px; display: flex; flex-direction: column; gap: 10px; }
  .main { flex: 4 1 480px; display: flex; flex-direction: column; gap: 12px; min-width: 0; }
  .list { display: flex; flex-direction: column; gap: 4px; }
  .item { text-align: left; padding: 8px 10px; border-radius: 8px; border: 1px solid transparent; background: transparent; color: var(--text); }
  .item:hover { border-color: var(--line2); }
  .item.on { border-color: var(--blue-line); color: var(--blue); }
  .grow { flex: 1 1 220px; min-width: 0; }
  .plan { width: 100%; height: auto; border: 1px solid var(--line); border-radius: 10px; }
  .grid2 { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 18px; }
  .plain { list-style: none; margin: 8px 0 0; padding: 0; display: flex; flex-direction: column; gap: 6px; font-size: 13px; }
  .dot { display: inline-block; width: 9px; height: 9px; border-radius: 50%; margin-right: 6px; }
  .link { background: none; border: 0; padding: 0 4px; color: var(--blue); text-decoration: underline; font-size: 12px; }
  .small { font-size: 12px; margin: 4px 0; }
  .stat { margin: 0; }
  .err { border-color: #7a2e22; background: #2a1511; color: #ffb4a6; }
  .ok-note { padding: 10px 12px; border-radius: 8px; background: #122117; border: 1px solid #2c5a3a; font-size: 13px; color: #b8e3c0; }
  .notes-label { margin-top: 4px; }
  input.input { min-width: 0; }
  .tilemap { width: 100%; height: 460px; border: 1px solid var(--line); border-radius: 10px; overflow: hidden; background: #0f1110; }
  :global(.pinmark) { display: flex; align-items: center; gap: 6px; pointer-events: none; }
  :global(.pinmark .pdot) { width: 12px; height: 12px; border-radius: 50%; border: 2px solid #0f1110; display: inline-block; }
  :global(.pinmark .plab) { color: #ece7de; font-size: 12px; text-shadow: 0 0 3px #000, 0 0 3px #000; white-space: nowrap; }
</style>
