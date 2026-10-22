<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import { mapsList, mapLoad, mapSave, mapDelete, mapExportGpx, routeStats, mapPacks, tileUrl, placeSearch, directions as askDirections, type Directions, type DirRoute, type PlaceHit, type MapMeta, type LatLon, type RouteStats, type Pack } from '../lib/api';
  import type { Map as MLMap, Marker, StyleSpecification, GeoJSONSource } from 'maplibre-gl';
  import { settings } from '../lib/settings.svelte';
  import { POI_GROUPS, kindName, groupOf, distanceKm, compass } from '../lib/mapkinds';
  import { DRAW_COLORS, DRAW_WIDTHS, DEFAULT_COLOR, drawingFeatures, previewFeatures, handlesOf, labelPoint, measureOf, pathKm, areaKm2, fmtLength, fmtArea, type Drawing, type DrawKind } from '../lib/mapdraw';

  type Pin = { id: number; label: string; kind: 'pin' | 'rendezvous'; lat: number; lon: number };
  type Area = { label: string; points: LatLon[] };
  type MapFile = { version: 1; name: string; pins: Pin[]; route: number[]; nogo: Area[]; drawings?: Drawing[] };

  let saved: MapMeta[] = $state([]);
  let current: string | null = $state(null); // id of the saved map being edited, null for a new one
  let name = $state('');
  let notes = $state('');
  let pins: Pin[] = $state([]);
  let route: number[] = $state([]); // pin ids in order
  let nogo: Area[] = $state([]);
  let drawings: Drawing[] = $state([]);
  let draft: LatLon[] = $state([]); // no-go area being drawn
  let draftLabel = $state('');
  let message = $state('');
  let error = $state('');
  /** What the open map file covers, read from its header. */
  let packInfo = $state('');
  /** True when the map file covers only part of the country: the grey beyond its edge is simply outside it. */
  let partial = $state(false);
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
  let streetPacks: Pack[] = $derived(packs.filter((p) => p.kind === 'street'));
  let street: Pack | undefined = $derived(streetPacks[0]);
  // pictures: USGS topo maps (maps/us/topo) and aerial photos (maps/satellite); more than one file can be on the drive,
  // for example the whole country at a wide view and one county sharp
  let topoPacks: Pack[] = $derived(packs.filter((p) => p.kind === 'topo'));
  let aerialPacks: Pack[] = $derived(packs.filter((p) => p.kind === 'satellite'));
  // the colours of the street map: light (the default), dark or black; the choice is remembered on this computer
  type Look = 'light' | 'dark' | 'black';
  function loadLook(): Look {
    try { const v = localStorage.getItem('cindrix-map-look'); if (v === 'light' || v === 'dark' || v === 'black') return v; } catch { /* not remembered */ }
    return 'light';
  }
  let look: Look = $state(loadLook());
  function chooseLook(v: Look) {
    look = v;
    try { localStorage.setItem('cindrix-map-look', v); } catch { /* not remembered */ }
    // the map is drawn once when it opens, so it is opened again with the new colours
    location.reload();
  }
  type MapKind = 'street' | 'topo' | 'aerial';
  let mapKind: MapKind = $state('street');
  let rasterIds: Record<'topo' | 'aerial', string[]> = { topo: [], aerial: [] };
  let vecLayers: { id: string; type: string }[] = [];
  function applyKind() {
    if (!map || !mapReady) return;
    const show = (ids: string[], on: boolean) => { for (const id of ids) if (map!.getLayer(id)) map!.setLayoutProperty(id, 'visibility', on ? 'visible' : 'none'); };
    show(rasterIds.topo, mapKind === 'topo');
    show(rasterIds.aerial, mapKind === 'aerial');
    // topo maps carry their own names and roads; over aerial photos only the roads, borders and names are kept
    for (const l of vecLayers) {
      const on = mapKind === 'street' || (mapKind === 'aerial' && (l.type === 'line' || l.type === 'symbol'));
      if (map.getLayer(l.id)) map.setLayoutProperty(l.id, 'visibility', on ? 'visible' : 'none');
    }
  }
  $effect(() => { mapKind; mapReady; applyKind(); });
  // one map source per street file (the lower 48, Alaska and Hawaii are separate files): 'protomaps', 'protomaps1', ...
  let srcNames: string[] = ['protomaps'];
  const poiIds = (gid: string): string[] => srcNames.flatMap((_, i) => [`poi-${gid}${i ? '-' + i : ''}`, `poi-${gid}${i ? '-' + i : ''}-label`]);
  let mapEl: HTMLDivElement | undefined = $state();
  let mapReady = $state(false);
  let tileError = $state('');
  // The tool that decides what a click on the map does. 'off' is the plain hand: move the map, pick things.
  type Tool = 'off' | 'pin' | 'rendezvous' | 'corner' | 'line' | 'shape' | 'rect' | 'circle' | 'text' | 'measure' | 'erase';
  let addMode: Tool = $state('off');
  let map: MLMap | null = null;
  let resizer: ResizeObserver | null = null;
  let onScale: (() => void) | null = null;
  let dprQuery: MediaQueryList | null = null;
  function syncSize(m: MLMap) {
    try { m.setPixelRatio(window.devicePixelRatio || 1); } catch { /* older map library */ }
    m.resize();
  }
  let startBox: { box: [[number, number], [number, number]]; maxZoom: number } | null = null;
  let markers: Marker[] = [];
  // full screen, cursor readout, centre cross-hair
  let stageEl: HTMLDivElement | undefined = $state();
  let full = $state(false);
  let apiFull = false;
  let cursor: LatLon | null = $state(null);
  let centre: LatLon | null = $state(null);
  let cross = $state(true);
  let drawer = $state(true);
  // places marked on the map (hospitals, water, fuel ...), switched on and off by group
  let layersOpen = $state(false);
  let labelsOn = $state(false); // place names are drawn only when the font files are on the drive
  let poiOn: Record<string, boolean> = $state(loadPoiChoice());
  type PoiPick = { name: string; kind: string; lat: number; lon: number };
  let picked: PoiPick | null = $state(null);
  let nearGroup = $state('emergency');
  let nearList: (PoiPick & { km: number; dir: string })[] = $state([]);
  let nearNote = $state('');

  function loadPoiChoice(): Record<string, boolean> {
    const base: Record<string, boolean> = Object.fromEntries(POI_GROUPS.map((g) => [g.id, g.on]));
    try {
      const raw = localStorage.getItem('cindrix-map-layers');
      if (raw) {
        const v = JSON.parse(raw) as Record<string, unknown>;
        for (const g of POI_GROUPS) if (typeof v[g.id] === 'boolean') base[g.id] = v[g.id] as boolean;
      }
    } catch { /* the choice is just not remembered */ }
    return base;
  }
  function savePoiChoice() {
    try { localStorage.setItem('cindrix-map-layers', JSON.stringify(poiOn)); } catch { /* not remembered */ }
  }
  function setGroup(id: string, on: boolean) {
    poiOn[id] = on;
    savePoiChoice();
    applyPoiVisibility();
  }
  function setAllGroups(on: boolean) {
    for (const g of POI_GROUPS) poiOn[g.id] = on;
    savePoiChoice();
    applyPoiVisibility();
  }
  function applyPoiVisibility() {
    if (!map || !mapReady) return;
    for (const g of POI_GROUPS) {
      const v = poiOn[g.id] ? 'visible' : 'none';
      for (const id of poiIds(g.id)) if (map.getLayer(id)) map.setLayoutProperty(id, 'visibility', v);
    }
  }
  function addPoiLayers(m: MLMap, withNames: boolean) {
    for (const g of POI_GROUPS) {
      const filter = ['match', ['get', 'kind'], g.kinds, true, false];
      const vis = poiOn[g.id] ? 'visible' : 'none';
      srcNames.forEach((src, i) => {
        const sfx = i ? '-' + i : '';
        m.addLayer({
          id: `poi-${g.id}${sfx}`, type: 'circle', source: src, 'source-layer': 'pois', minzoom: 7, filter,
          layout: { visibility: vis },
          paint: { 'circle-color': g.color, 'circle-radius': ['interpolate', ['linear'], ['zoom'], 7, 2.5, 12, 5, 16, 8], 'circle-stroke-color': '#0b0d0c', 'circle-stroke-width': 1.5 },
        } as never);
        if (withNames) {
          m.addLayer({
            id: `poi-${g.id}${sfx}-label`, type: 'symbol', source: src, 'source-layer': 'pois', minzoom: 12.5, filter,
            layout: { visibility: vis, 'text-field': ['coalesce', ['get', 'name'], ''], 'text-font': ['Noto Sans Regular'], 'text-size': 11, 'text-anchor': 'top', 'text-offset': [0, 0.8], 'text-optional': true, 'text-max-width': 8 },
            paint: { 'text-color': look === 'light' ? '#1d2420' : '#ece7de', 'text-halo-color': look === 'light' ? '#ffffff' : '#0b0d0c', 'text-halo-width': 1.4 },
          } as never);
        }
      });
    }
  }
  /** Is there a marked place under this point of the screen? */
  function poiAt(m: MLMap, point: { x: number; y: number }): PoiPick | null {
    const layers = POI_GROUPS.filter((g) => poiOn[g.id]).flatMap((g) => srcNames.map((_, i) => `poi-${g.id}${i ? '-' + i : ''}`)).filter((id) => m.getLayer(id));
    if (!layers.length) return null;
    const r = 8;
    const f = m.queryRenderedFeatures([[point.x - r, point.y - r], [point.x + r, point.y + r]], { layers })[0];
    if (!f || f.geometry.type !== 'Point') return null;
    const [lon, lat] = f.geometry.coordinates as [number, number];
    const kind = String(f.properties?.kind ?? '');
    return { name: String(f.properties?.name ?? '') || kindName(kind), kind, lat, lon };
  }
  function findNearest() {
    nearNote = '';
    nearList = [];
    if (!map || !mapReady) return;
    const g = POI_GROUPS.find((x) => x.id === nearGroup);
    if (!g) return;
    const c = map.getCenter();
    const from: [number, number] = [c.lat, c.lng];
    const feats = srcNames.flatMap((src) => map!.querySourceFeatures(src, { sourceLayer: 'pois', filter: ['match', ['get', 'kind'], g.kinds, true, false] as never }));
    const seen = new Set<string>();
    const out: (PoiPick & { km: number; dir: string })[] = [];
    for (const f of feats) {
      if (f.geometry.type !== 'Point') continue;
      const [lon, lat] = f.geometry.coordinates as [number, number];
      const kind = String(f.properties?.kind ?? '');
      const name = String(f.properties?.name ?? '') || kindName(kind);
      const key = `${name}|${lat.toFixed(4)}|${lon.toFixed(4)}`;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push({ name, kind, lat, lon, km: distanceKm(from, [lat, lon]), dir: compass(from, [lat, lon]) });
    }
    out.sort((a, b) => a.km - b.km);
    nearList = out.slice(0, 8);
    nearNote = out.length
      ? 'Nearest to the middle of the map. This looks only at the part of the map on screen, so zoom out to look further.'
      : 'None found on the part of the map on screen. Zoom out or move the map and try again.';
  }
  function showPick(h: PoiPick) {
    if (!map) return;
    map.jumpTo({ center: [h.lon, h.lat], zoom: Math.max(map.getZoom(), 14) });
    picked = h;
  }
  function pinPick(h: PoiPick) {
    const id = pins.reduce((a, x) => Math.max(a, x.id), 0) + 1;
    pins = [...pins, { id, label: h.name, kind: 'pin', lat: Math.round(h.lat * 1e5) / 1e5, lon: Math.round(h.lon * 1e5) / 1e5 }];
  }
  const dist = (km: number) => (settings.values.mapUnits === 'metric' ? `${km.toFixed(km < 10 ? 1 : 0)} km` : `${(km * 0.621371).toFixed(km * 0.621371 < 10 ? 1 : 0)} mi`);

  function toggleFull() {
    full = !full;
    if (full) {
      // Also ask the window to hide its own frame. If that is refused the map still fills the app.
      try { void stageEl?.requestFullscreen?.().then(() => (apiFull = true)).catch(() => (apiFull = false)); } catch { apiFull = false; }
    } else if (document.fullscreenElement) {
      void document.exitFullscreen().catch(() => {});
      apiFull = false;
    }
    queueMicrotask(() => map?.resize());
    setTimeout(() => map?.resize(), 150);
  }
  function onFullChange() {
    if (apiFull && !document.fullscreenElement) { full = false; apiFull = false; setTimeout(() => map?.resize(), 100); }
  }
  function onKey(e: KeyboardEvent) {
    const typing = e.target instanceof HTMLElement && (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA' || e.target.tagName === 'SELECT');
    if (e.key === 'Escape') {
      if (building.length || measurePts.length || draft.length) { e.preventDefault(); cancelDrawing(); return; }
      if (addMode !== 'off') { e.preventDefault(); addMode = 'off'; return; }
      if (selectedId !== null) { selectedId = null; return; }
      if (full) { e.preventDefault(); toggleFull(); }
      return;
    }
    if (typing || !map) return;
    if (e.key === 'Enter' && (addMode === 'line' || addMode === 'shape')) { e.preventDefault(); finishBuilding(); }
    else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') { e.preventDefault(); if (e.shiftKey) redo(); else undo(); }
    else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') { e.preventDefault(); redo(); }
    else if ((e.key === 'Delete' || e.key === 'Backspace') && selectedId !== null && addMode === 'off') { e.preventDefault(); deleteDrawing(selectedId); }
  }
  let ml: typeof import('maplibre-gl') | null = null;
  let protocol: import('pmtiles').Protocol | null = null;

  onMount(async () => {
    document.addEventListener('fullscreenchange', onFullChange);
    await refresh();
    try {
      packs = await mapPacks();
    } catch (e) {
      tileError = String(e);
    }
  });
  onDestroy(() => {
    document.removeEventListener('fullscreenchange', onFullChange);
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
    resizer?.disconnect();
    if (onScale) window.removeEventListener('resize', onScale);
    if (onScale) dprQuery?.removeEventListener('change', onScale);
    map?.remove();
    map = null;
  });

  $effect(() => {
    if (streetPacks.length && mapEl && !map) void initMap(streetPacks, mapEl);
  });

  async function initMap(streets: Pack[], el: HTMLDivElement) {
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
      // Read each file's header first: it says which area and zoom levels the pack really holds.
      // A pack cut for one town has nothing to show at the whole-country view the map used to open on.
      type Area = { w: number; s: number; e: number; n: number; minZoom: number; maxZoom: number };
      const usable: { pack: Pack; url: string }[] = [];
      const areas: Area[] = [];
      for (const pack of streets) {
        const url = await tileUrl(pack.rel);
        const archive = new PMTiles(url);
        protocol.add(archive);
        try {
          const h = await archive.getHeader();
          if ([h.minLon, h.minLat, h.maxLon, h.maxLat].every((v) => Number.isFinite(v)) && h.maxLon > h.minLon && h.maxLat > h.minLat) {
            areas.push({ w: h.minLon, s: h.minLat, e: h.maxLon, n: h.maxLat, minZoom: h.minZoom, maxZoom: h.maxZoom });
          } else {
            areas.push({ w: -180, s: -85, e: 180, n: 85, minZoom: 0, maxZoom: Number.isFinite(h.maxZoom) ? h.maxZoom : 14 });
          }
          usable.push({ pack, url });
        } catch (e) {
          tileError = 'The map file ' + pack.name + ' could not be read: ' + String((e as Error)?.message ?? e) + ' The file may be damaged or incomplete; cut it again with scripts\\get-maps.ps1.';
        }
      }
      if (!usable.length) return;
      const area: Area | null = {
        w: Math.min(...areas.map((a) => a.w)), s: Math.min(...areas.map((a) => a.s)),
        e: Math.max(...areas.map((a) => a.e)), n: Math.max(...areas.map((a) => a.n)),
        minZoom: Math.min(...areas.map((a) => a.minZoom)), maxZoom: Math.max(...areas.map((a) => a.maxZoom)),
      };
      packInfo = `${usable.map((u) => u.pack.name).join(', ')}: covers ${area.s.toFixed(2)}, ${area.w.toFixed(2)} to ${area.n.toFixed(2)}, ${area.e.toFixed(2)}; zoom ${area.minZoom} to ${area.maxZoom}.`;
      srcNames = usable.map((_, i) => (i ? 'protomaps' + i : 'protomaps'));
      // No glyph (font) files ship yet, so the text layers are left out: roads, water, land and
      // buildings are drawn, place names are not. Pin labels are drawn by this screen instead.
      // Place names need the font files in maps/fonts. When they are there the lettering is drawn (street, town and
      // water names, and the names of marked places); when not, only shapes are drawn and the map says how to add them.
      let glyphs: string | undefined;
      try {
        const tpl = (await tileUrl('fonts/FONTSTACKX/FONTRANGEX.pbf')).replace('FONTSTACKX', '{fontstack}').replace('FONTRANGEX', '{range}');
        const probe = tpl.replace('{fontstack}', encodeURIComponent('Noto Sans Regular')).replace('{range}', '0-255');
        const r = await fetch(probe);
        if (r.ok && (await r.arrayBuffer()).byteLength > 0) glyphs = tpl;
      } catch { /* no fonts: shapes only */ }
      labelsOn = !!glyphs;
      // symbol layers that use picture icons are left out (no icon sheet ships); text-only ones are kept when the fonts are there
      const keep = (l: { type: string; layout?: Record<string, unknown>; 'source-layer'?: string }) =>
        l.type !== 'symbol' || (labelsOn && l['source-layer'] !== 'pois' && !(l.layout && 'icon-image' in l.layout));
      // raster pictures: one source per file, drawn under the street lines
      const imgSources: Record<string, unknown> = {};
      const imgLayers: Record<'topo' | 'aerial', unknown[]> = { topo: [], aerial: [] };
      rasterIds = { topo: [], aerial: [] };
      for (const [kind, list] of [['topo', topoPacks], ['aerial', aerialPacks]] as const) {
        // widest file first so a sharper file for one area is drawn over it
        const sorted = [...list].sort((a, b) => b.mb - a.mb);
        let i = 0;
        for (const pk of sorted) {
          try {
            const u = await tileUrl(pk.rel);
            protocol.add(new PMTiles(u));
            const sid = `${kind}-src-${i}`, lid = `${kind}-img-${i}`;
            imgSources[sid] = { type: 'raster', url: 'pmtiles://' + u, tileSize: 256, attribution: kind === 'aerial' ? 'Imagery: USGS The National Map (NAIP)' : 'Topo: USGS The National Map' };
            imgLayers[kind].push({ id: lid, type: 'raster', source: sid, layout: { visibility: 'none' } });
            rasterIds[kind].push(lid);
            i++;
          } catch { /* a damaged picture file is just left out */ }
        }
      }
      const style = {
        version: 8,
        ...(glyphs ? { glyphs } : {}),
        sources: { ...Object.fromEntries(usable.map((u, i) => [srcNames[i], { type: 'vector', url: 'pmtiles://' + u.url, attribution: '© OpenStreetMap contributors' }])), ...imgSources },
        // the same drawing rules for every file; the background is drawn once, and each layer name is made different
        layers: (() => {
          const vec = usable.flatMap((_, i) =>
            layers(srcNames[i], namedFlavor(look), { lang: 'en' })
              .filter((l) => keep(l as never) && (i === 0 || (l as { type: string }).type !== 'background'))
              .map((l) => (i === 0 ? l : { ...l, id: (l as { id: string }).id + '-' + i })),
          ) as { id: string; type: string }[];
          vecLayers = vec.map((l) => ({ id: l.id, type: l.type }));
          // the background first, then the pictures, then everything else
          const [bg, ...rest] = vec;
          return [bg, ...imgLayers.topo, ...imgLayers.aerial, ...rest];
        })(),
      } as unknown as StyleSpecification;
      const m = new maplibregl.Map({
        container: el,
        style,
        center: area ? [(area.w + area.e) / 2, (area.s + area.n) / 2] : [-98.5, 39.8],
        zoom: area ? Math.max(area.minZoom, 3) : 3.5,
        attributionControl: { compact: true },
        pixelRatio: window.devicePixelRatio || 1,
        fadeDuration: 0,
        maxTileCacheSize: 200,
      });
      // Open on the United States when the pack covers any of it (a world pack would otherwise open on the whole
      // globe); a pack cut for one town or one country elsewhere opens on what it covers.
      if (area) {
        const us = { w: -125, s: 24, e: -66.5, n: 49.5 };
        const w = Math.max(area.w, us.w), s = Math.max(area.s, us.s), e = Math.min(area.e, us.e), n = Math.min(area.n, us.n);
        const [bw, bs, be, bn] = e > w && n > s ? [w, s, e, n] : [area.w, area.s, area.e, area.n];
        startBox = { box: [[bw, bs], [be, bn]], maxZoom: Math.min(area.maxZoom, 15) };
        m.fitBounds(startBox.box, { padding: 20, duration: 0, maxZoom: startBox.maxZoom });
      }
      // A map file cut for one area has nothing beyond its edge, which shows as grey. Stop the map at the edge.
      if (area && (area.e - area.w < 40 || area.n - area.s < 20)) {
        partial = true;
        m.setMaxBounds([[area.w - 0.2, area.s - 0.2], [area.e + 0.2, area.n + 0.2]]);
      }
      // The map must follow its box when the window or the interface size changes, or part of it stays grey.
      if (typeof ResizeObserver !== 'undefined') {
        resizer = new ResizeObserver(() => { syncSize(m); });
        resizer.observe(el);
      }
      // Windows screen scaling (125%, 150%, 175%) changes when the window moves or the scale is changed: the picture
      // must be drawn at the screen's real pixel size, or it fills only a corner of the box.
      onScale = () => syncSize(m);
      window.addEventListener('resize', onScale);
      try {
        dprQuery = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
        dprQuery.addEventListener('change', onScale);
      } catch { /* an old web view: the resize listener above still covers most cases */ }
      for (const t of [200, 800, 2500]) setTimeout(() => syncSize(m), t);
      m.addControl(new maplibregl.NavigationControl({ showCompass: true }), 'bottom-right');
      m.addControl(new maplibregl.ScaleControl({ unit: settings.values.mapUnits === 'metric' ? 'metric' : 'imperial' }), 'bottom-left');
      m.on('error', (e) => {
        const msg = String((e as { error?: Error }).error?.message ?? e);
        if (/image|sprite|glyph|font/i.test(msg) && !/tile/i.test(msg)) return; // a missing picture or letter is not a broken map
        tileError = 'The map could not read its tiles: ' + String((e as { error?: Error }).error?.message ?? e);
      });
      m.on('load', () => {
        for (const id of ['route', 'nogo', 'draft', 'dir']) m.addSource(id, { type: 'geojson', data: empty() });
        m.addLayer({ id: 'nogo-fill', type: 'fill', source: 'nogo', paint: { 'fill-color': '#e5533d', 'fill-opacity': 0.25 } });
        m.addLayer({ id: 'nogo-line', type: 'line', source: 'nogo', paint: { 'line-color': '#e5533d', 'line-width': 2, 'line-dasharray': [2, 2] } });
        m.addLayer({ id: 'draft-line', type: 'line', source: 'draft', paint: { 'line-color': '#e5533d', 'line-width': 2, 'line-dasharray': [1, 2] } });
        // what the person drew: shapes (with a light fill), lines, a halo round the picked one, and the unfinished shape
        for (const id of ['draw', 'drawtmp']) m.addSource(id, { type: 'geojson', data: empty() });
        m.addLayer({ id: 'draw-fill', type: 'fill', source: 'draw', filter: ['==', ['geometry-type'], 'Polygon'], paint: { 'fill-color': ['get', 'color'], 'fill-opacity': 0.22 } });
        m.addLayer({ id: 'draw-sel', type: 'line', source: 'draw', filter: ['==', ['get', 'sel'], 1], layout: { 'line-cap': 'round', 'line-join': 'round' }, paint: { 'line-color': '#1a73e8', 'line-opacity': 0.35, 'line-width': ['+', ['get', 'width'], 8] } });
        m.addLayer({ id: 'draw-line', type: 'line', source: 'draw', layout: { 'line-cap': 'round', 'line-join': 'round' }, paint: { 'line-color': ['get', 'color'], 'line-width': ['get', 'width'] } });
        m.addLayer({ id: 'drawtmp-line', type: 'line', source: 'drawtmp', filter: ['!', ['has', 'pt']], paint: { 'line-color': '#1a73e8', 'line-width': 3, 'line-dasharray': [2, 2] } });
        m.addLayer({ id: 'drawtmp-pts', type: 'circle', source: 'drawtmp', filter: ['has', 'pt'], paint: { 'circle-radius': 5, 'circle-color': '#ffffff', 'circle-stroke-color': '#1a73e8', 'circle-stroke-width': 2 } });
        addPoiLayers(m, labelsOn);
        m.addLayer({ id: 'route-line', type: 'line', source: 'route', paint: { 'line-color': '#ff9a1f', 'line-width': 4 } });
        m.addLayer({ id: 'dir-casing', type: 'line', source: 'dir', layout: { 'line-cap': 'round', 'line-join': 'round' }, paint: { 'line-color': '#ffffff', 'line-width': 9 } });
        m.addLayer({ id: 'dir-line', type: 'line', source: 'dir', layout: { 'line-cap': 'round', 'line-join': 'round' }, paint: { 'line-color': '#1a73e8', 'line-width': 5.5 } });
        mapReady = true;
        // The box may have settled to its real size after the map was made: measure again, then frame the start view.
        syncSize(m);
        if (startBox && pins.length === 0 && nogo.length === 0 && drawings.length === 0) m.fitBounds(startBox.box, { padding: 20, duration: 0, maxZoom: startBox.maxZoom });
        zoomToPoints();
      });
      m.on('click', (e) => {
        // with the plain hand, a click on a marked place opens its card, and a click on something drawn picks it
        if (addMode === 'off') {
          const hit = poiAt(m, e.point);
          if (hit) { picked = hit; selectedId = null; return; }
          picked = null;
          const did = drawingAt(m, e.point);
          selectedId = did;
          return;
        }
        onMapClick(e.lngLat.lat, e.lngLat.lng, e.point);
      });
      m.on('mousemove', (e) => {
        cursor = [Math.round(e.lngLat.lat * 1e5) / 1e5, Math.round(e.lngLat.lng * 1e5) / 1e5];
        if (addMode === 'line' || addMode === 'shape' || addMode === 'rect' || addMode === 'circle' || addMode === 'measure') hover = [e.lngLat.lat, e.lngLat.lng];
      });
      m.on('mouseout', () => { cursor = null; hover = null; });
      const setCentre = () => { const c = m.getCenter(); centre = [Math.round(c.lat * 1e5) / 1e5, Math.round(c.lng * 1e5) / 1e5]; };
      m.on('move', setCentre);
      setCentre();
      map = m;
    } catch (e) {
      tileError = 'The map viewer could not start: ' + String(e);
    }
  }

  const empty = () => ({ type: 'FeatureCollection' as const, features: [] });
  const ring = (pts: LatLon[]) => [...pts, pts[0]].map(([la, lo]) => [lo, la]);

  function onMapClick(lat: number, lon: number, point?: { x: number; y: number }) {
    const p: LatLon = [Math.round(lat * 1e6) / 1e6, Math.round(lon * 1e6) / 1e6];
    const p5: LatLon = [Math.round(lat * 1e5) / 1e5, Math.round(lon * 1e5) / 1e5];
    if (addMode === 'pin' || addMode === 'rendezvous') {
      pushUndo();
      const id = pins.reduce((m, x) => Math.max(m, x.id), 0) + 1;
      pins = [...pins, { id, label: (addMode === 'rendezvous' ? 'Rendezvous ' : 'Pin ') + id, kind: addMode, lat: p5[0], lon: p5[1] }];
    } else if (addMode === 'corner') {
      draft = [...draft, p5];
    } else if (addMode === 'line' || addMode === 'shape') {
      building = [...building, p];
    } else if (addMode === 'rect') {
      if (building.length === 0) building = [p];
      else addDrawing('rect', [building[0], p]);
    } else if (addMode === 'circle') {
      if (building.length === 0) building = [p];
      else {
        const r = distanceKm(building[0], p) * 1000;
        if (r >= 1) addDrawing('circle', [building[0]], r);
      }
    } else if (addMode === 'text') {
      addDrawing('text', [p], undefined, textDraft.trim() || 'Note');
    } else if (addMode === 'measure') {
      measurePts = [...measurePts, p];
    } else if (addMode === 'erase' && map && point) {
      const did = drawingAt(map, point);
      if (did !== null) { deleteDrawing(did); return; }
      const ng = nogoAt(map, point);
      if (ng !== null) { pushUndo(); nogo = nogo.filter((_, i) => i !== ng); }
    }
  }

  function zoomToPoints() {
    if (!map || !ml) return;
    const all: LatLon[] = [...pins.map((p) => [p.lat, p.lon] as LatLon), ...nogo.flatMap((a) => a.points), ...drawings.flatMap((d) => (d.kind === 'circle' ? handlesOf(d) : d.points))];
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
      features: nogo.flatMap((a, i) => (a.points.length >= 3 ? [{ type: 'Feature' as const, properties: { i }, geometry: { type: 'Polygon' as const, coordinates: [ring(a.points)] } }] : [])),
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
      el.addEventListener('click', (ev) => {
        if (addMode === 'erase') { ev.stopPropagation(); pushUndo(); removePin(p.id); }
      });
      const mk = new ml!.Marker({ element: el, anchor: 'left', draggable: addMode === 'off' }).setLngLat([p.lon, p.lat]).addTo(m);
      mk.on('dragstart', () => pushUndo());
      mk.on('dragend', () => {
        const ll = mk.getLngLat();
        pins = pins.map((q) => (q.id === p.id ? { ...q, lat: Math.round(ll.lat * 1e5) / 1e5, lon: Math.round(ll.lng * 1e5) / 1e5 } : q));
      });
      return mk;
    });
  });

  // ---------- drawing: lines, areas, boxes, circles, text, measuring, erasing, undo ----------
  const TOOLS: { k: Tool; label: string; tip: string; d: string }[] = [
    { k: 'off', label: 'Move', tip: 'Move the map and pick things you drew. Drag a pin or a corner to move it.', d: 'M5 3l14 8-6 2-2 6z' },
    { k: 'pin', label: 'Pin', tip: 'Click the map to drop a pin', d: 'M12 21s-6-6.2-6-11a6 6 0 1112 0c0 4.8-6 11-6 11z M12 7.5a2.5 2.5 0 100 5 2.5 2.5 0 000-5z' },
    { k: 'rendezvous', label: 'Meet', tip: 'Click the map to mark a place to meet', d: 'M6 21V4 M6 5h11l-2 4 2 4H6' },
    { k: 'corner', label: 'Avoid', tip: 'Click corners to outline an area to avoid (flooded, blocked or unsafe)', d: 'M12 3a9 9 0 100 18 9 9 0 000-18z M6 6l12 12' },
    { k: 'line', label: 'Line', tip: 'Draw a line: click each point, then Finish (or press Enter)', d: 'M4 19l6-8 4 4 6-9' },
    { k: 'shape', label: 'Shape', tip: 'Draw an area: click each corner, then Finish (or press Enter)', d: 'M5 8l7-4 7 5-2 9H7z' },
    { k: 'rect', label: 'Box', tip: 'Draw a box: click two opposite corners', d: 'M4 6h16v12H4z' },
    { k: 'circle', label: 'Circle', tip: 'Draw a circle: click the middle, then click the edge', d: 'M12 4a8 8 0 100 16 8 8 0 000-16z' },
    { k: 'text', label: 'Text', tip: 'Put a written note on the map: type it, then click where it goes', d: 'M5 6h14 M12 6v13 M9 19h6' },
    { k: 'measure', label: 'Measure', tip: 'Measure distance and area: click points along the way', d: 'M3 16l13-13 5 5-13 13z M8 8l2 2 M11 5l2 2 M5 11l2 2' },
    { k: 'erase', label: 'Erase', tip: 'Click anything you drew, a pin or a no-go area to remove it', d: 'M7 21h10 M5 14l7-9 7 7-5 6H9z' },
  ];
  let selectedId: number | null = $state(null);
  let building: LatLon[] = $state([]); // corners clicked so far for the line, shape, box or circle being drawn
  let hover: LatLon | null = $state(null); // where the pointer is, for the rubber band
  let measurePts: LatLon[] = $state([]);
  let drawColor = $state(DEFAULT_COLOR);
  let drawWidth = $state(4);
  let textDraft = $state('Note');
  let handleTick = $state(0);
  let labelMarkers: Marker[] = [];
  let handleMarkers: Marker[] = [];
  const metric = $derived(settings.values.mapUnits === 'metric');
  let sel: Drawing | null = $derived(drawings.find((d) => d.id === selectedId) ?? null);

  // undo and redo: whole-map snapshots
  let undoStack: string[] = [];
  let redoStack: string[] = [];
  let canUndo = $state(false);
  let canRedo = $state(false);
  const snapshot = () => JSON.stringify({ pins, route, nogo, drawings });
  function pushUndo() {
    undoStack.push(snapshot());
    if (undoStack.length > 100) undoStack.shift();
    redoStack = [];
    canUndo = true;
    canRedo = false;
  }
  function restore(json: string) {
    const f = JSON.parse(json) as { pins: Pin[]; route: number[]; nogo: Area[]; drawings: Drawing[] };
    pins = f.pins;
    route = f.route;
    nogo = f.nogo;
    drawings = f.drawings;
    selectedId = null;
    handleTick++;
  }
  function undo() {
    const last = undoStack.pop();
    if (!last) return;
    redoStack.push(snapshot());
    restore(last);
    canUndo = undoStack.length > 0;
    canRedo = true;
  }
  function redo() {
    const next = redoStack.pop();
    if (!next) return;
    undoStack.push(snapshot());
    restore(next);
    canUndo = true;
    canRedo = redoStack.length > 0;
  }
  function resetUndo() {
    undoStack = [];
    redoStack = [];
    canUndo = canRedo = false;
  }

  function cancelDrawing() {
    building = [];
    hover = null;
    measurePts = [];
    draft = [];
  }
  function chooseTool(t: Tool) {
    cancelDrawing();
    selectedId = null;
    picked = null;
    addMode = addMode === t && t !== 'off' ? 'off' : t;
  }
  const nextDrawId = () => drawings.reduce((m, d) => Math.max(m, d.id), 0) + 1;
  function addDrawing(kind: DrawKind, points: LatLon[], radiusM?: number, label = '') {
    pushUndo();
    const id = nextDrawId();
    drawings = [...drawings, { id, kind, label, color: drawColor, width: drawWidth, points, ...(radiusM ? { radiusM } : {}) }];
    building = [];
    hover = null;
    selectedId = id;
    addMode = 'off';
  }
  function finishBuilding() {
    if (addMode === 'line' && building.length >= 2) addDrawing('line', building);
    else if (addMode === 'shape' && building.length >= 3) addDrawing('shape', building);
  }
  function setSel(key: 'label' | 'color' | 'width', value: string | number) {
    if (selectedId === null) return;
    drawings = drawings.map((d) => (d.id === selectedId ? { ...d, [key]: value } : d));
  }
  function deleteDrawing(id: number) {
    pushUndo();
    drawings = drawings.filter((d) => d.id !== id);
    if (selectedId === id) selectedId = null;
  }
  function moveHandle(id: number, idx: number, pt: LatLon) {
    drawings = drawings.map((d) => {
      if (d.id !== id) return d;
      if (d.kind === 'circle') return idx === 0 ? { ...d, points: [pt] } : { ...d, radiusM: Math.max(1, distanceKm(d.points[0], pt) * 1000) };
      return { ...d, points: d.points.map((q, i) => (i === idx ? pt : q)) };
    });
  }
  /** The id of the drawing under a point of the screen, if any. */
  function drawingAt(m: MLMap, point: { x: number; y: number }): number | null {
    const r = 7;
    const f = m.queryRenderedFeatures([[point.x - r, point.y - r], [point.x + r, point.y + r]], { layers: ['draw-fill', 'draw-line'].filter((l) => m.getLayer(l)) })[0];
    const id = Number(f?.properties?.id);
    return f && Number.isFinite(id) ? id : null;
  }
  /** The no-go area under a point of the screen, if any (for erasing). */
  function nogoAt(m: MLMap, point: { x: number; y: number }): number | null {
    const r = 7;
    const f = m.queryRenderedFeatures([[point.x - r, point.y - r], [point.x + r, point.y + r]], { layers: ['nogo-fill'].filter((l) => m.getLayer(l)) })[0];
    if (!f) return null;
    const ids = (f.properties?.i as number | undefined);
    return typeof ids === 'number' ? ids : null;
  }

  // (read through a function so the checker does not assume the tool is still 'off' as when it was declared)
  const toolNow = (): Tool => addMode;
  let measureKm = $derived.by(() => pathKm(hover && toolNow() === 'measure' ? [...measurePts, hover] : measurePts));
  let measureArea = $derived(measurePts.length >= 3 ? areaKm2(measurePts) : 0);
  let buildKm = $derived.by(() => pathKm(hover && (toolNow() === 'line' || toolNow() === 'shape') ? [...building, hover] : building));
  let selMeasure = $derived(sel ? measureOf(sel) : { km: 0, km2: 0 });

  // what is drawn, and the names on it
  $effect(() => {
    if (!map || !ml || !mapReady) return;
    const m = map;
    const list = drawings;
    const sid = selectedId;
    (m.getSource('draw') as GeoJSONSource).setData(drawingFeatures(list, sid) as never);
    for (const k of labelMarkers) k.remove();
    labelMarkers = list.flatMap((d) => {
      const at = labelPoint(d);
      if (!at || (!d.label.trim() && d.kind !== 'text')) return [];
      const el = document.createElement('div');
      el.className = d.kind === 'text' ? 'dtext' : 'dname';
      el.textContent = d.label || '(text)'; // text, never markup
      if (d.kind === 'text') el.style.color = d.color === '#ffffff' ? '#ffffff' : d.color;
      el.style.cursor = 'pointer';
      el.addEventListener('click', (ev) => {
        ev.stopPropagation();
        if (addMode === 'erase') deleteDrawing(d.id);
        else if (addMode === 'off') selectedId = d.id;
      });
      return [new ml!.Marker({ element: el, anchor: 'center' }).setLngLat([at[1], at[0]]).addTo(m)];
    });
  });
  // the unfinished shape and the measuring path
  $effect(() => {
    if (!map || !mapReady) return;
    const kind: DrawKind | 'measure' | null = addMode === 'measure' ? 'measure' : addMode === 'line' || addMode === 'shape' || addMode === 'rect' || addMode === 'circle' ? addMode : null;
    const pts = addMode === 'measure' ? measurePts : building;
    (map.getSource('drawtmp') as GeoJSONSource).setData((kind ? previewFeatures(pts, kind, hover, null) : empty()) as never);
  });
  // the handles a selected drawing can be reshaped by
  $effect(() => {
    handleTick;
    const id = selectedId;
    const tool = addMode;
    for (const h of handleMarkers) h.remove();
    handleMarkers = [];
    if (!map || !ml || !mapReady || id === null || tool !== 'off') return;
    untrack(() => {
      const d = drawings.find((x) => x.id === id);
      if (!d) return;
      handlesOf(d).forEach((pt, idx) => {
        const el = document.createElement('div');
        el.className = 'dhandle';
        const mk = new ml!.Marker({ element: el, draggable: true }).setLngLat([pt[1], pt[0]]).addTo(map!);
        mk.on('dragstart', () => pushUndo());
        mk.on('drag', () => { const ll = mk.getLngLat(); moveHandle(id, idx, [Math.round(ll.lat * 1e6) / 1e6, Math.round(ll.lng * 1e6) / 1e6]); });
        mk.on('dragend', () => { handleTick++; });
        handleMarkers.push(mk);
      });
    });
  });
  // the pointer shows what a click will do
  $effect(() => {
    if (!map) return;
    map.getCanvas().style.cursor = addMode === 'off' ? '' : addMode === 'erase' ? 'not-allowed' : 'crosshair';
    if (addMode === 'off') map.doubleClickZoom.enable();
    else map.doubleClickZoom.disable();
  });

  // ---------- directions: from one place to another ----------
  let dirOpen = $state(false);
  let dirFrom = $state('');
  let dirTo = $state('');
  let dirA: PlaceHit | null = $state(null);
  let dirB: PlaceHit | null = $state(null);
  let dirAlt: { from: PlaceHit[]; to: PlaceHit[] } = $state({ from: [], to: [] });
  let dirBusy = $state(false);
  let dirErr = $state('');
  let dirResult: Directions | null = $state(null);
  let dirWay: 'car' | 'foot' = $state('car');
  let dirMarkers: Marker[] = [];
  const dirRoute: DirRoute | null = $derived(dirResult ? dirResult[dirWay].route : null);
  const hitName = (h: PlaceHit) => h.name + (h.state ? ', ' + h.state : '');
  const fmtMin = (m: number) => {
    const t = Math.max(1, Math.round(m));
    return t < 60 ? `${t} min` : `${Math.floor(t / 60)} h ${t % 60} min`;
  };

  async function lookUp(text: string): Promise<PlaceHit[]> {
    const c = map?.getCenter();
    return (await placeSearch(text, c ? [c.lat, c.lng] : undefined)).places;
  }
  async function runDirections(keepPlaces = false) {
    dirErr = '';
    if (!dirFrom.trim() || !dirTo.trim()) {
      dirErr = 'Type where to start and where to go.';
      return;
    }
    dirBusy = true;
    dirResult = null;
    try {
      if (!keepPlaces || !dirA || !dirB) {
        const [fa, ta] = await Promise.all([lookUp(dirFrom.trim()), lookUp(dirTo.trim())]);
        if (!fa.length || !ta.length) {
          dirErr = `Could not find ${!fa.length ? 'the start "' + dirFrom.trim() + '"' : 'the end "' + dirTo.trim() + '"'}. Try a street address with the house number first, a town and state, a ZIP code, or a latitude and longitude.`;
          return;
        }
        dirA = fa[0];
        dirB = ta[0];
        dirAlt = { from: fa.slice(1, 5), to: ta.slice(1, 5) };
      }
      const a = dirA as PlaceHit | null;
      const b = dirB as PlaceHit | null;
      if (!a || !b) return;
      const r = await askDirections([a.lat, a.lon], [b.lat, b.lon], nogo.map((x) => x.points));
      dirResult = r;
      dirWay = r.car.route || !r.foot.route ? 'car' : 'foot';
      queueMicrotask(fitDirections);
    } catch (e) {
      dirErr = String(e);
    } finally {
      dirBusy = false;
    }
  }
  function pickAlt(side: 'from' | 'to', h: PlaceHit) {
    if (side === 'from' && dirA) {
      dirAlt = { ...dirAlt, from: [dirA, ...dirAlt.from.filter((x) => x !== h)].slice(0, 4) };
      dirA = h;
      dirFrom = hitName(h);
    } else if (side === 'to' && dirB) {
      dirAlt = { ...dirAlt, to: [dirB, ...dirAlt.to.filter((x) => x !== h)].slice(0, 4) };
      dirB = h;
      dirTo = hitName(h);
    }
    void runDirections(true);
  }
  function swapDirections() {
    [dirFrom, dirTo] = [dirTo, dirFrom];
    [dirA, dirB] = [dirB, dirA];
    dirAlt = { from: dirAlt.to, to: dirAlt.from };
    if (dirA && dirB) void runDirections(true);
  }
  function closeDirections() {
    dirOpen = false;
    dirResult = null;
    dirErr = '';
  }
  function fitDirections() {
    const r = dirRoute;
    if (!map || !ml || !r || r.points.length < 2) return;
    const b = new ml.LngLatBounds([r.points[0][1], r.points[0][0]], [r.points[0][1], r.points[0][0]]);
    for (const [la, lo] of r.points) b.extend([lo, la]);
    map.fitBounds(b, { padding: { top: 90, bottom: 90, left: 480, right: 90 }, maxZoom: 16, duration: 0 });
  }
  // draw the way on the map, with an A and a B at the ends
  $effect(() => {
    if (!map || !ml || !mapReady) return;
    const m = map;
    const r = dirRoute;
    (m.getSource('dir') as GeoJSONSource | undefined)?.setData(
      r ? { type: 'Feature', properties: {}, geometry: { type: 'LineString', coordinates: r.points.map(([la, lo]) => [lo, la]) } } : empty(),
    );
    for (const k of dirMarkers) k.remove();
    dirMarkers = [];
    if (r && dirA && dirB) {
      for (const [h, letter, color] of [[dirA, 'A', '#188038'], [dirB, 'B', '#d93025']] as const) {
        const el = document.createElement('div');
        el.textContent = letter;
        el.style.cssText = `width:26px;height:26px;border-radius:50%;background:${color};color:#fff;font:700 14px sans-serif;display:flex;align-items:center;justify-content:center;border:2px solid #fff;box-shadow:0 1px 4px #0007`;
        dirMarkers.push(new ml.Marker({ element: el }).setLngLat([h.lon, h.lat]).addTo(m));
      }
    }
  });

  // ---------- find a place ----------
  let findTerm = $state('');
  let finds: PlaceHit[] = $state([]);
  let findNote = $state('');
  let findBusy = $state(false);
  let foundMarker: Marker | null = null;
  const ZOOM_FOR: Record<string, number> = { state: 6, county: 9, city: 11, zip: 13, point: 15, address: 17 };

  async function findPlace() {
    const q = findTerm.trim();
    if (!q) return;
    findBusy = true;
    findNote = '';
    try {
      const c = map?.getCenter();
      const r = await placeSearch(q, c ? [c.lat, c.lng] : undefined);
      finds = r.places;
      if (!finds.length) {
        findNote = r.has_list
          ? 'Nothing matches. Try the town and state ("Denver, CO"), a ZIP code, or a latitude and longitude.' + (r.has_addresses ? ' For a street address type the house number first ("123 Main St, Denver CO"); the address data does not have every house.' : ' To look up street addresses, run Get-Addresses.bat from the scripts folder on the drive.')
          : 'Nothing matches. Without the places list on the drive only states and a latitude and longitude are found (see Settings, Maps, or run scripts\\get-maps.ps1 with -Places).';
      } else if (finds.length === 1) {
        goToPlace(finds[0]);
      }
    } catch (e) {
      findNote = 'The search could not run: ' + String(e);
    }
    findBusy = false;
  }
  function goToPlace(h: PlaceHit) {
    if (!map || !ml) return;
    finds = [];
    findTerm = h.name + (h.state ? ', ' + h.state : '');
    map.jumpTo({ center: [h.lon, h.lat], zoom: ZOOM_FOR[h.kind] ?? 12 });
    foundMarker?.remove();
    const el = document.createElement('div');
    el.className = 'pinmark';
    el.innerHTML = '<span class="pdot"></span><span class="plab"></span>';
    (el.querySelector('.pdot') as HTMLElement).style.background = '#5aa9e6';
    (el.querySelector('.plab') as HTMLElement).textContent = h.name;
    foundMarker = new ml.Marker({ element: el, anchor: 'left' }).setLngLat([h.lon, h.lat]).addTo(map);
  }
  function pinFound() {
    const m = foundMarker?.getLngLat();
    if (!m) return;
    const id = pins.reduce((a, x) => Math.max(a, x.id), 0) + 1;
    pins = [...pins, { id, label: findTerm || 'Pin ' + id, kind: 'pin', lat: Math.round(m.lat * 1e5) / 1e5, lon: Math.round(m.lng * 1e5) / 1e5 }];
  }

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
    pushUndo();
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
    pushUndo();
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
    drawings = [];
    draft = [];
    cancelDrawing();
    selectedId = null;
    resetUndo();
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
      drawings = f.drawings ?? [];
      selectedId = null;
      resetUndo();
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
    const file: MapFile = { version: 1, name: name.trim(), pins, route, nogo, drawings };
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

{#snippet pinList()}
  <ul class="plain">
    {#each pins as p}
      <li>
        <span class="dot" style="background:{pinColor(p.kind)}"></span>
        {p.label} <span class="muted">{p.lat.toFixed(4)}, {p.lon.toFixed(4)}</span>
        <button class="link" title="Go to this pin on the map" onclick={() => map?.jumpTo({ center: [p.lon, p.lat], zoom: Math.max(map.getZoom(), 12) })}>go to</button>
        <button class="link" title="Add this pin to the end of your route" onclick={() => toRoute(p.id)}>add to route</button>
        <button class="link" title="Delete this pin" onclick={() => { pushUndo(); removePin(p.id); }}>remove</button>
      </li>
    {:else}
      <li class="muted">No pins yet. Choose Pins above, then click the map.</li>
    {/each}
  </ul>
{/snippet}

<svelte:window onkeydown={onKey} />

<section>
  <h1>Maps</h1>
  <p class="sub">Plan with your own points: places to meet, routes between them, and areas to avoid. Save the plan, print it, and export the route for a phone map app.</p>
</section>

{#if street}
  <div class="warn">
    <strong>Map: {streetPacks.map((p) => p.name).join(', ')}</strong> ({streetPacks.reduce((a, p) => a + p.mb, 0)} MB, from this drive).{#if packInfo} This covers {packInfo.replace(/^.*?: covers /, '')}{/if}{#if partial} It covers only part of the country, so the map stops at its edge. This build comes with only a small sample area. To get the whole United States, double-click scripts\Get-Maps.bat (it needs internet and tens of gigabytes of space, and takes hours).{/if} Roads, water, land and buildings are drawn{#if labelsOn}, with names{:else}; place names need the map fonts (run scripts\get-maps.ps1 -FontsOnly){/if}. Use Places on the map to mark hospitals, water, fuel and more. Positions you place are approximate. The route you build from your own pins is straight lines between them; for the way by road, car or on foot, use Directions on the map, which keeps out of the areas you mark to avoid.
  </div>
{:else}
  <div class="warn">
    <strong>No map tiles on this drive yet.</strong> Street and topo maps come from map packs (see doc 7), so for now this is a plan view of the points you enter, drawn to scale with nothing underneath. Positions are typed in by hand and are approximate. The route you build from your own pins is straight lines between them. Directions along roads need a street map on the drive.
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
      <div class="stage" class:full bind:this={stageEl}>
        <div class="tools">
          <div class="row">
            <button class="btn" title="Zoom so everything you placed or drew fits on screen" onclick={zoomToPoints} disabled={pins.length === 0 && nogo.length === 0 && drawings.length === 0}>Zoom to my points</button>
            <button class="btn" class:primary={layersOpen} title="Choose which kinds of places are marked on the map: hospitals, water, fuel, shelter and more" onclick={() => (layersOpen = !layersOpen)}>Places on the map</button>
            <button class="btn" class:primary={cross} title="Show or hide the cross-hair and the coordinates of the middle of the map" onclick={() => (cross = !cross)}>Cross-hair</button>
            <button class="btn" title={full ? 'Go back to the normal page (or press Esc)' : 'Make the map fill the whole screen. Your pins and tools stay.'} onclick={toggleFull}>{full ? 'Exit full screen' : 'Full screen'}</button>
            <span class="muted">COLOURS:</span>
            <select class="input" aria-label="Map colours" title="Choose how the street map is coloured" value={look} onchange={(e) => chooseLook((e.currentTarget as HTMLSelectElement).value as Look)}>
              <option value="light">Light (full colour: blue water, green parks)</option>
              <option value="dark">Dark (dark with some colour)</option>
              <option value="black">Black (darkest, greys only)</option>
            </select>
          </div>
        </div>
        <div class="mapbox">
          <div class="tilemap" bind:this={mapEl} role="application" aria-label="Map"></div>
          <div class="ov ov-tl">
            <div class="gsearch">
              <input class="gin" type="search" placeholder="Search for a place or address" aria-label="Find a place" bind:value={findTerm}
                onkeydown={(e) => { if (e.key === 'Enter') void findPlace(); }} />
              <button class="gbtn" title="Look this place up and move the map to it" onclick={findPlace} disabled={!findTerm.trim() || findBusy}>Find</button>
              <button class="gbtn" title="Keep the place you just found as a pin on your map" onclick={pinFound} disabled={!foundMarker}>Pin it</button>
              <button class="gbtn" title="Work out the way between two places, by car and on foot" onclick={() => (dirOpen = !dirOpen)}>Directions</button>
            </div>
            {#if dirOpen}
              <div class="gdir" role="region" aria-label="Directions">
                <div class="gdir-head"><strong>Directions</strong>
                  <button class="gbtn" title="Close the directions and take the line off the map" onclick={closeDirections}>Close</button></div>
                <div class="gdir-row"><span class="gdot a">A</span>
                  <input class="gin box" placeholder="Start: an address, town, ZIP code or latitude, longitude" aria-label="Start" bind:value={dirFrom}
                    onkeydown={(e) => { if (e.key === 'Enter') void runDirections(); }} oninput={() => (dirA = null)} /></div>
                <div class="gdir-row"><span class="gdot b">B</span>
                  <input class="gin box" placeholder="End: an address, town, ZIP code or latitude, longitude" aria-label="End" bind:value={dirTo}
                    onkeydown={(e) => { if (e.key === 'Enter') void runDirections(); }} oninput={() => (dirB = null)} /></div>
                <div class="gdir-row">
                  <button class="gbtn" title="Swap the start and the end" onclick={swapDirections}>Swap</button>
                  <button class="gbtn on" title="Work out the way between the two places" onclick={() => runDirections()} disabled={dirBusy || !dirFrom.trim() || !dirTo.trim()}>{dirBusy ? 'Working it out…' : 'Get directions'}</button>
                </div>
                {#if nogo.length}<p class="gnote2">The areas you marked to avoid are kept out of the way.</p>{/if}
                {#if dirErr}<p class="gerr">{dirErr}</p>{/if}
                {#if dirAlt.from.length && dirResult}<p class="gnote2">Start not the right place? {#each dirAlt.from as h}<button class="glink" onclick={() => pickAlt('from', h)}>{hitName(h)}</button>{/each}</p>{/if}
                {#if dirAlt.to.length && dirResult}<p class="gnote2">End not the right place? {#each dirAlt.to as h}<button class="glink" onclick={() => pickAlt('to', h)}>{hitName(h)}</button>{/each}</p>{/if}
                {#if dirResult}
                  <div class="gtabs" role="tablist" aria-label="How you travel">
                    {#each [['car', 'By car'], ['foot', 'On foot']] as [k, label]}
                      {@const w = dirResult[k as 'car' | 'foot']}
                      <button role="tab" aria-selected={dirWay === k} class:on={dirWay === k} onclick={() => { dirWay = k as 'car' | 'foot'; queueMicrotask(fitDirections); }}
                        title={w.route ? `${label}: about ${fmtMin(w.route.minutes)}` : w.problem}>
                        <span class="gt-l">{label}</span><span class="gt-t">{w.route ? fmtMin(w.route.minutes) : 'no way'}</span>
                        <span class="gt-d">{w.route ? dist(w.route.km) : ''}</span></button>
                    {/each}
                  </div>
                  {#if dirRoute}
                    {#if dirRoute.note}<p class="gnote2">{dirRoute.note}</p>{/if}
                    <ol class="dsteps" aria-label="The way">
                      {#each dirRoute.steps as st}
                        <li>{st.name} <span class="gmuted">· {dist(st.km)} · {fmtMin(st.minutes)}</span></li>
                      {/each}
                    </ol>
                    <p class="gnote2">Times are estimates from the kind of road. The map does not know about one-way streets, traffic, closures or stairs.</p>
                  {:else}
                    <p class="gerr">{dirResult[dirWay].problem}</p>
                  {/if}
                {/if}
              </div>
            {/if}
            {#if finds.length > 1}
              <ul class="gfinds" aria-label="Places found">
                {#each finds as h}
                  <li><button onclick={() => goToPlace(h)}>{h.name}{h.state ? ', ' + h.state : ''} <span class="gmuted">{h.kind === 'address' ? '' : h.kind}</span></button></li>
                {/each}
              </ul>
            {/if}
            {#if findNote}<p class="gnote">{findNote}</p>{/if}
            <div class="gtools" role="toolbar" aria-label="Drawing tools">
              {#each TOOLS as t}
                <button class="gt" class:on={addMode === t.k} title={t.tip} onclick={() => chooseTool(t.k)}>
                  <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={t.d} /></svg>
                  <span>{t.label}</span>
                </button>
              {/each}
              <span class="gsep"></span>
              <button class="gt" title="Undo the last change (Ctrl+Z)" disabled={!canUndo} onclick={undo}>
                <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 7L4 12l5 5 M4 12h10a6 6 0 010 8" /></svg><span>Undo</span>
              </button>
              <button class="gt" title="Put back what you undid (Ctrl+Y)" disabled={!canRedo} onclick={redo}>
                <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M15 7l5 5-5 5 M20 12H10a6 6 0 000 8" /></svg><span>Redo</span>
              </button>
            </div>
          </div>
          <div class="ov ov-bl" role="group" aria-label="Kind of map">
            <button class="gl" class:on={mapKind === 'street'} title="The street map: roads, towns, water, buildings" onclick={() => (mapKind = 'street')}>
              <span class="gsw sw-map"></span><span>Map</span></button>
            <button class="gl" class:on={mapKind === 'topo'} disabled={!topoPacks.length}
              title={topoPacks.length ? 'USGS topographic map: contour lines, hills, trails' : 'No topo map on this drive yet. Double-click scripts\\Get-Imagery.bat to put one on it.'}
              onclick={() => (mapKind = 'topo')}><span class="gsw sw-topo"></span><span>Topo</span></button>
            <button class="gl" class:on={mapKind === 'aerial'} disabled={!aerialPacks.length}
              title={aerialPacks.length ? 'Aerial photos from above, with roads and names drawn on top' : 'No aerial photos on this drive yet. Double-click scripts\\Get-Imagery.bat to put them on it.'}
              onclick={() => (mapKind = 'aerial')}><span class="gsw sw-air"></span><span>Satellite</span></button>
          </div>
          {#if addMode !== 'off'}
            <div class="ov ov-hint" role="status">
              {#if addMode === 'pin'}<span>Click the map to drop a pin.</span>
              {:else if addMode === 'rendezvous'}<span>Click the map to mark a place to meet.</span>
              {:else if addMode === 'corner'}
                <span>{draft.length} corner{draft.length === 1 ? '' : 's'} so far. Click the next corner.</span>
                <button class="gbtn" title="Close the area from the corners clicked so far" onclick={finishArea} disabled={draft.length < 3}>Finish area</button>
                {#if draft.length > 0}<button class="gbtn" title="Throw away these corners and begin again" onclick={() => (draft = [])}>Start over</button>{/if}
              {:else if addMode === 'line' || addMode === 'shape'}
                <span>{building.length} point{building.length === 1 ? '' : 's'}{building.length > 1 ? ' · ' + fmtLength(buildKm, metric) : ''}. Click each {addMode === 'line' ? 'point' : 'corner'}.</span>
                <button class="gbtn" title="Finish the drawing (or press Enter)" onclick={finishBuilding} disabled={building.length < (addMode === 'line' ? 2 : 3)}>Finish</button>
                <button class="gbtn" title="Throw this drawing away (or press Esc)" onclick={cancelDrawing}>Cancel</button>
              {:else if addMode === 'rect'}<span>{building.length === 0 ? 'Click one corner of the box.' : 'Click the opposite corner.'}</span>
              {:else if addMode === 'circle'}<span>{building.length === 0 ? 'Click the middle of the circle.' : 'Click the edge of the circle.'}</span>
              {:else if addMode === 'text'}
                <label class="gtext">Text <input class="gin small" bind:value={textDraft} aria-label="Text to place" maxlength="80" /></label>
                <span>then click where it goes.</span>
              {:else if addMode === 'measure'}
                <span>{measurePts.length === 0 ? 'Click points to measure.' : 'Distance ' + fmtLength(measureKm, metric)}{measureArea > 0 ? ' · Area if closed ' + fmtArea(measureArea, metric) : ''}</span>
                {#if measurePts.length > 0}<button class="gbtn" title="Start a new measurement" onclick={() => (measurePts = [])}>Clear</button>{/if}
              {:else if addMode === 'erase'}<span>Click anything you drew, a pin or a no-go area to erase it. Undo brings it back.</span>
              {/if}
            </div>
          {/if}
          {#if sel}
            <aside class="ov ov-sel" aria-label="Drawing details">
              <div class="gtitle">{sel.kind === 'line' ? 'Line' : sel.kind === 'shape' ? 'Shape' : sel.kind === 'rect' ? 'Box' : sel.kind === 'circle' ? 'Circle' : 'Text'}</div>
              <input class="gin" value={sel.label} placeholder={sel.kind === 'text' ? 'Text' : 'Name (shown on the map)'} aria-label="Name"
                onfocus={() => pushUndo()} oninput={(e) => setSel('label', (e.currentTarget as HTMLInputElement).value)} maxlength="80" />
              <div class="gswatches" role="group" aria-label="Colour">
                {#each DRAW_COLORS as c}
                  <button class="gcol" class:on={sel.color === c.hex} style="background:{c.hex}" title={c.name} aria-label={c.name} onclick={() => { pushUndo(); setSel('color', c.hex); drawColor = c.hex; }}></button>
                {/each}
              </div>
              {#if sel.kind !== 'text'}
                <div class="gwidths" role="group" aria-label="Thickness">
                  {#each DRAW_WIDTHS as w}
                    <button class="gbtn" class:on={sel.width === w.px} title={w.name + ' line'} onclick={() => { pushUndo(); setSel('width', w.px); drawWidth = w.px; }}>{w.name}</button>
                  {/each}
                </div>
                <div class="gmeasure">
                  {#if sel.kind === 'circle'}Radius {fmtLength((sel.radiusM ?? 0) / 1000, metric)} · {/if}{#if sel.kind === 'line'}Length {fmtLength(selMeasure.km, metric)}{:else}Edge {fmtLength(selMeasure.km, metric)}{/if}{#if selMeasure.km2 > 0} · Area {fmtArea(selMeasure.km2, metric)}{/if}
                </div>
              {/if}
              <div class="gwidths">
                <button class="gbtn" title="Remove this drawing (Delete key)" onclick={() => { if (sel) deleteDrawing(sel.id); }}>Delete</button>
                <button class="gbtn" title="Done with this drawing" onclick={() => (selectedId = null)}>Done</button>
              </div>
              <div class="gmuted">Drag the round handles on the map to reshape it.</div>
            </aside>
          {/if}
          {#if cross}<div class="cross" aria-hidden="true"><span></span></div>{/if}
          {#if layersOpen}
            <aside class="layers" aria-label="Places on the map">
              <div class="row between"><span class="label">PLACES ON THE MAP</span><button class="link" title="Close this list" onclick={() => (layersOpen = false)}>close</button></div>
              <div class="row"><button class="link" title="Mark every kind of place" onclick={() => setAllGroups(true)}>all</button><button class="link" title="Mark none" onclick={() => setAllGroups(false)}>none</button></div>
              {#each POI_GROUPS as g}
                <label class="grp"><input type="checkbox" checked={poiOn[g.id]} onchange={(e) => setGroup(g.id, (e.currentTarget as HTMLInputElement).checked)} /><span class="swatch" style="background:{g.color}"></span>{g.label}</label>
              {/each}
              <p class="muted small">Places appear when you zoom in. {labelsOn ? 'Names show up close in.' : 'Names need the map fonts: run scripts\\get-maps.ps1 -FontsOnly.'} Click a marked place for its card.</p>
              <div class="label gap">FIND THE NEAREST</div>
              <div class="row">
                <select class="input grow" bind:value={nearGroup} aria-label="Kind of place to find">{#each POI_GROUPS as g}<option value={g.id}>{g.label}</option>{/each}</select>
                <button class="btn" title="List the closest places of this kind to the middle of the map" onclick={findNearest}>Find</button>
              </div>
              {#if nearNote}<p class="muted small">{nearNote}</p>{/if}
              {#each nearList as h}
                <button class="near" title="Move the map to this place" onclick={() => showPick(h)}><span>{h.name}</span><span class="muted">{kindName(h.kind)} · {dist(h.km)} {h.dir}</span></button>
              {/each}
            </aside>
          {/if}
          {#if picked}
            <div class="pick" role="dialog" aria-label="Marked place">
              <div><strong>{picked.name}</strong> <span class="muted small">{kindName(picked.kind)}{groupOf(picked.kind) ? ' · ' + groupOf(picked.kind)?.label : ''}</span></div>
              <div class="muted small">{picked.lat.toFixed(5)}, {picked.lon.toFixed(5)}</div>
              <div class="row">
                <button class="btn" title="Keep this place as a pin on your map" onclick={() => { if (picked) pinPick(picked); }}>Pin it</button>
                <button class="btn" title="Close this card" onclick={() => (picked = null)}>Close</button>
              </div>
            </div>
          {/if}
          <div class="hud" aria-live="off">
            {#if cursor}<div>CURSOR {cursor[0].toFixed(5)}, {cursor[1].toFixed(5)}</div>{/if}
            {#if cross && centre}<div>CENTRE {centre[0].toFixed(5)}, {centre[1].toFixed(5)}</div>{/if}
          </div>
          {#if full}
            <aside class="drawer" class:closed={!drawer}>
              <button class="link" title="Show or hide your list of pins and areas" onclick={() => (drawer = !drawer)}>{drawer ? 'Hide list ›' : '‹ My points'}</button>
              {#if drawer}
                <div class="label">My points</div>
                {@render pinList()}
                {#if route.length > 0}
                  <p class="muted small">Route: {route.map((id) => pins.find((p) => p.id === id)?.label ?? '?').join(' → ')}
                    <button class="link" title="Remove the route but keep the pins" onclick={() => (route = [])}>clear route</button></p>
                {/if}
                {#if stats}<p class="small"><strong>Route:</strong> {settings.values.mapUnits === 'metric' ? `${stats.km.toFixed(1)} km` : `${stats.miles.toFixed(1)} miles`}, straight line</p>{/if}
                <div class="label">No-go areas</div>
                <ul class="plain">
                  {#each nogo as a, i}
                    <li>{a.label} <button class="link" title="Remove this area" onclick={() => (nogo = nogo.filter((_, j) => j !== i))}>remove</button></li>
                  {:else}<li class="muted">None.</li>{/each}
                </ul>
                <div class="row">
                  <input class="input" placeholder="Map name" bind:value={name} aria-label="Map name" />
                  <button class="btn primary" title="Save this map and everything on it" onclick={save}>Save</button>
                </div>
                {#if message}<p class="small">{message}</p>{/if}
                {#if error}<p class="small errtxt">{error}</p>{/if}
              {/if}
            </aside>
          {/if}
        </div>
      </div>
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
        {@render pinList()}
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
    <p class="muted small">Map data comes from © OpenStreetMap contributors. Place names: GeoNames (CC BY 4.0). Saved maps are plain files in the user/saved-maps folder.</p>
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
  .find { margin-top: 2px; }
  .finds { list-style: none; margin: 0; padding: 4px; border: 1px solid var(--line2); border-radius: 8px; background: var(--panel2); }
  .finds button { display: block; width: 100%; text-align: left; background: none; border: 0; padding: 6px 8px; border-radius: 6px; }
  .finds button:hover { background: var(--blue-soft); }
  .stage { display: flex; flex-direction: column; gap: 8px; }
  .stage.full { position: fixed; inset: 0; z-index: 2000; background: #0b0d0c; padding: 10px; }
  .tools { display: flex; flex-direction: column; gap: 8px; }
  .mapbox { position: relative; flex: 1; min-height: 0; }
  .stage.full .mapbox { display: flex; }
  .stage.full .tilemap { height: auto; flex: 1; }
  .cross { position: absolute; inset: 0; pointer-events: none; display: grid; place-items: center; }
  .cross span { width: 34px; height: 34px; position: relative; opacity: .85; }
  .cross span::before, .cross span::after { content: ''; position: absolute; background: #ff9a1f; }
  .cross span::before { left: 50%; top: 0; bottom: 0; width: 1px; }
  .cross span::after { top: 50%; left: 0; right: 0; height: 1px; }
  .hud { position: absolute; left: 50%; transform: translateX(-50%); bottom: 8px; padding: 6px 9px; background: #0b0d0ccc; border: 1px solid #3a3f3a; border-left: 3px solid #ff9a1f; border-radius: 4px; font-family: var(--mono, monospace); font-size: 11px; letter-spacing: .06em; color: #cfd6cc; pointer-events: none; }
  .hud:empty { display: none; }
  .drawer { position: absolute; top: 10px; right: 10px; bottom: 28px; width: 290px; overflow: auto; padding: 10px; background: #0b0d0cee; border: 1px solid #3a3f3a; border-radius: 8px; display: flex; flex-direction: column; gap: 8px; z-index: 5; }
  .layers { position: absolute; top: 190px; left: 10px; bottom: 120px; width: 270px; z-index: 6; overflow: auto; padding: 10px; background: #0b0d0cee; border: 1px solid #3a3f3a; border-radius: 8px; display: flex; flex-direction: column; gap: 6px; z-index: 5; }
  .grp { display: flex; align-items: center; gap: 8px; font-size: 13px; padding: 2px 0; }
  .swatch { width: 10px; height: 10px; border-radius: 50%; border: 1px solid #0b0d0c; flex: 0 0 auto; }
  .near { display: flex; flex-direction: column; text-align: left; gap: 1px; padding: 6px 8px; border: 1px solid var(--line); border-radius: 6px; background: transparent; font-size: 13px; }
  .near:hover { border-color: var(--blue-line); }
  .pick { position: absolute; left: 50%; transform: translateX(-50%); bottom: 34px; min-width: 240px; max-width: 360px; padding: 10px 12px; background: #0b0d0cf2; border: 1px solid #3a3f3a; border-left: 3px solid #ff9a1f; border-radius: 6px; display: flex; flex-direction: column; gap: 6px; z-index: 6; }
  .gap { margin-top: 8px; }
  .drawer.closed { width: auto; bottom: auto; }
  .errtxt { color: #ffb4a6; }
  .tilemap { width: 100%; height: clamp(520px, calc(100vh - 260px), 1200px); border: 1px solid var(--line); border-radius: 10px; overflow: hidden; background: #0f1110; }
  :global(.pinmark) { display: flex; align-items: center; gap: 6px; pointer-events: none; }
  :global(.pinmark .pdot) { width: 12px; height: 12px; border-radius: 50%; border: 2px solid #0f1110; display: inline-block; }
  :global(.pinmark .plab) { color: #ece7de; font-size: 12px; text-shadow: 0 0 3px #000, 0 0 3px #000; white-space: nowrap; }

  /* Google-Maps-like controls floating over the map */
  .ov { position: absolute; z-index: 5; pointer-events: none; }
  .ov > * { pointer-events: auto; }
  .ov-tl { top: 10px; left: 10px; width: min(460px, calc(100% - 70px)); display: flex; flex-direction: column; gap: 8px; }
  .ov-bl { left: 10px; bottom: 40px; display: flex; gap: 6px; }
  .ov-hint { left: 50%; transform: translateX(-50%); bottom: 36px; display: flex; gap: 8px; align-items: center; flex-wrap: wrap; justify-content: center; max-width: calc(100% - 200px); padding: 8px 12px; background: #fff; color: #202124; border-radius: 10px; box-shadow: 0 2px 8px #0005; font-size: 13px; }
  .ov-sel { right: 10px; top: 10px; width: 230px; display: flex; flex-direction: column; gap: 8px; padding: 12px; background: #fff; color: #202124; border-radius: 10px; box-shadow: 0 2px 8px #0005; }
  .gsearch { display: flex; align-items: center; gap: 4px; background: #fff; border-radius: 24px; box-shadow: 0 2px 6px #0005; padding: 4px 6px 4px 14px; }
  .gin { flex: 1 1 auto; min-width: 0; border: 0; outline: 0; background: transparent; color: #202124; font-size: 14px; padding: 6px 0; }
  .ov-sel .gin { border: 1px solid #dadce0; border-radius: 6px; padding: 6px 8px; }
  .gin.small { width: 150px; border: 1px solid #dadce0; border-radius: 6px; padding: 4px 6px; flex: none; }
  .gbtn { background: #fff; border: 1px solid #dadce0; color: #1a73e8; border-radius: 16px; padding: 5px 12px; font-size: 12px; font-weight: 600; cursor: pointer; }
  .gbtn:hover:not(:disabled) { background: #e8f0fe; }
  .gbtn:disabled { color: #9aa0a6; cursor: default; }
  .gbtn.on { background: #e8f0fe; border-color: #1a73e8; }
  .gfinds { list-style: none; margin: 0; padding: 4px; background: #fff; border-radius: 10px; box-shadow: 0 2px 6px #0005; max-height: 260px; overflow: auto; }
  .gfinds button { display: block; width: 100%; text-align: left; background: none; border: 0; padding: 8px 10px; border-radius: 6px; color: #202124; font-size: 13px; cursor: pointer; }
  .gfinds button:hover { background: #f1f3f4; }
  .gmuted { color: #5f6368; font-size: 12px; }
  .gnote { margin: 0; padding: 8px 12px; background: #fff; color: #5f6368; border-radius: 10px; box-shadow: 0 2px 6px #0005; font-size: 12px; }
  .gtools { display: flex; flex-wrap: wrap; align-items: center; gap: 2px; background: #fff; border-radius: 10px; box-shadow: 0 2px 6px #0005; padding: 4px; }
  .gt { display: flex; flex-direction: column; align-items: center; gap: 1px; min-width: 46px; padding: 5px 4px; background: transparent; border: 0; border-radius: 8px; color: #3c4043; font-size: 10px; cursor: pointer; }
  .gt:hover:not(:disabled) { background: #f1f3f4; }
  .gt.on { background: #e8f0fe; color: #1a73e8; }
  .gt:disabled { color: #bdc1c6; cursor: default; }
  .gsep { width: 1px; align-self: stretch; background: #dadce0; margin: 2px 4px; }
  .gdir { background: #fff; color: #202124; border-radius: 12px; box-shadow: 0 2px 8px #0005; padding: 10px 12px; display: flex; flex-direction: column; gap: 8px; max-height: calc(100vh - 220px); overflow: auto; }
  .gdir-head { display: flex; justify-content: space-between; align-items: center; }
  .gdir-row { display: flex; align-items: center; gap: 8px; }
  .gin.box { border: 1px solid #dadce0; border-radius: 6px; padding: 7px 9px; flex: 1 1 auto; }
  .gdot { width: 22px; height: 22px; border-radius: 50%; color: #fff; font: 700 12px sans-serif; display: inline-flex; align-items: center; justify-content: center; flex: none; }
  .gdot.a { background: #188038; } .gdot.b { background: #d93025; }
  .gnote2 { margin: 0; color: #5f6368; font-size: 12px; }
  .gerr { margin: 0; color: #b3261e; font-size: 13px; }
  .glink { background: none; border: 0; color: #1a73e8; cursor: pointer; font-size: 12px; padding: 0 6px 0 0; text-decoration: underline; }
  .gtabs { display: flex; gap: 6px; }
  .gtabs button { flex: 1; display: flex; flex-direction: column; align-items: center; padding: 6px; background: #fff; border: 1px solid #dadce0; border-radius: 8px; cursor: pointer; color: #202124; }
  .gtabs button.on { border-color: #1a73e8; background: #e8f0fe; }
  .gt-l { font-size: 12px; color: #5f6368; } .gt-t { font-size: 16px; font-weight: 700; } .gt-d { font-size: 12px; color: #5f6368; }
  .dsteps { margin: 0; padding-left: 20px; font-size: 13px; }
  .dsteps li { padding: 2px 0; }
  .dsteps li::first-letter { text-transform: uppercase; }
  .gl { display: flex; flex-direction: column; align-items: center; gap: 2px; width: 62px; padding: 4px; background: #fff; border: 2px solid #fff; border-radius: 8px; box-shadow: 0 2px 6px #0005; color: #202124; font-size: 11px; cursor: pointer; }
  .gl.on { border-color: #1a73e8; }
  .gl:disabled { opacity: .55; cursor: not-allowed; }
  .gsw { width: 100%; height: 34px; border-radius: 5px; display: block; }
  .sw-map { background: linear-gradient(135deg, #e8eaed 0 45%, #aadaff 45% 62%, #c8e6c9 62%); }
  .sw-topo { background: repeating-linear-gradient(160deg, #d9c9a3 0 5px, #c4b184 5px 6px), #d9c9a3; }
  .sw-air { background: linear-gradient(135deg, #3d5a3a, #6b7f4a 55%, #2f4a5a); }
  .stage.full .ov-sel { right: 310px; }
  .gtitle { font-weight: 700; font-size: 14px; }
  .gswatches { display: flex; flex-wrap: wrap; gap: 6px; }
  .gcol { width: 22px; height: 22px; border-radius: 50%; border: 2px solid #dadce0; cursor: pointer; padding: 0; }
  .gcol.on { border-color: #202124; box-shadow: 0 0 0 2px #fff inset; }
  .gwidths { display: flex; gap: 6px; flex-wrap: wrap; }
  .gmeasure { font-size: 12px; color: #3c4043; }
  .gtext { display: flex; align-items: center; gap: 6px; font-size: 13px; }
  :global(.dtext) { font: 700 15px system-ui, sans-serif; text-shadow: 0 0 3px #fff, 0 0 3px #fff, 0 0 5px #fff; white-space: nowrap; padding: 2px 4px; }
  :global(.dname) { font: 600 12px system-ui, sans-serif; color: #202124; text-shadow: 0 0 3px #fff, 0 0 3px #fff; white-space: nowrap; }
  :global(.dhandle) { width: 14px; height: 14px; border-radius: 50%; background: #fff; border: 3px solid #1a73e8; box-shadow: 0 1px 4px #0006; cursor: grab; }
</style>
