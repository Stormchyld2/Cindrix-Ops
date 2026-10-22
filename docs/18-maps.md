# 18. Maps

Status: the saved-map features are built and tested. The tile viewer is **written but has never drawn a real map**: it needs a `.pmtiles` file from your PC (see "Getting map data"), and nobody has run it yet. Items marked **TO TEST** are unproven.

## What a person can do today
- Save a named map: pins, rendezvous points, a route, no-go areas, written notes.
- With no map pack installed: a plan view of the points, drawn to scale with a scale bar.
- With a street map pack installed: a real map (roads, water, land, buildings). Click the map to add pins, rendezvous points or no-go corners; zoom to the saved points.
- The route shows its straight-line length in km and miles. Real roads will be longer.
- Export the route as `route.gpx` for phone map apps.
- Saved maps are plain files in `user/saved-maps/<name>/` (`map.json`, `notes.md`, `route.gpx`), with a backup of the previous version before any save.

## Honest limits
- No road routing, and routes are not steered around no-go areas. The point-in-area test exists in the core (`maps::in_polygon`) but no screen uses it yet.
- No place names on the map: the style leaves out text layers because no font files ship yet. Pin labels are drawn by the app.
- No topo (contours, shading), no satellite layer, no GPS, no "line of sight", no layer toggles (hospitals, water and so on). The mockup shows all of these; none is built.
- Positions are placed by hand and are approximate.
- "Print or export a picture" is not built (`picture.png` is accepted by the store but nothing makes one yet).

## How the pieces fit
| Piece | Where | Job |
|---|---|---|
| Saved maps, distance, bearing, GPX, no-go test | `core/src/maps.rs` | Plain Rust, tested (19 tests) |
| Map packs and safe byte-range reads | `core/src/maps.rs` | Lists `.pmtiles` files under `maps/`; decides which files may be served; reads a range |
| `cindrix-tiles` protocol | `src-tauri/src/lib.rs` | Hands the web view byte ranges of `.pmtiles` files, and nothing else. A request with no Range header is refused (a pack can be many GB). |
| Map library | `src/views/Maps.svelte` | MapLibre GL JS with the `pmtiles` plugin and the `@protomaps/basemaps` style, dark flavor |
| Download tool | `scripts/get-maps.ps1` | Cuts a US extract out of a Protomaps build with the `pmtiles` command-line tool |

The map never reaches the internet: tiles come from the file on the drive. The shell's page-security setting allows only the `cindrix-tiles` address for this.

## Getting map data (run on your PC; needs internet and time)

**A small map of your own area first** (a few MB to a few hundred MB; the right size for a first test):
```
powershell -ExecutionPolicy Bypass -File scripts\get-maps.ps1 -Target D:\Cindrix -Bbox:"west,south,east,north" -Name my-area -MaxZoom 14
```
Replace `west,south,east,north` with four decimal numbers (US longitudes are negative), for example `-Bbox:"-105.10,39.62,-104.80,39.82"`. To find yours: open openstreetmap.org, zoom to the area, press Export, and read the left, bottom, right and top values. Write `-Bbox:"..."` with the colon so the minus sign is not mistaken for another option.

**The whole contiguous US:**
```
powershell -ExecutionPolicy Bypass -File scripts\get-maps.ps1 -Target D:\Cindrix
```
The script downloads the `pmtiles` tool from its GitHub releases, finds the newest Protomaps build, and extracts the area into `maps\us\street\<name>.pmtiles` (`conus.pmtiles` for the US; add `-Regions conus,alaska,hawaii`). It checks the file starts like a real PMTiles v3 file and adds a section to `SOURCE.txt` beside it. `-MaxZoom 14` or `13` gives a much smaller map; each zoom level roughly doubles the size. The planet file is about 120 GB, and the manifest's 15 GB guess for the US at full zoom is **unmeasured**. `check_drive` then verifies the pack (doc 7).

**The Windows download already has a test map.** Every Windows build in CI runs this very script for a small area (about 0.3 by 0.2 degrees, zoom 0 to 13) and puts the result in the portable folder's `maps` folder. That area is around Denver, Colorado, chosen only because it is well mapped; it is not anyone's home. If the Maps screen looks empty, pan there, or cut your own area as above. If that CI step failed, the download has no `maps` folder and the log of the "Windows app build" job says why.

The build is found from the list the Protomaps builds page reads (`build-metadata.protomaps.dev/builds.json`), then by trying recent dates on `build.protomaps.com` as a fallback; pass an address with `-Source` if both fail (the documented list is https://maps.protomaps.com/builds, and the Protomaps documentation warns that these addresses may change).

Still **not tried by a person**: the Windows run on your own machine. The `pmtiles extract` options `--bbox` and `--maxzoom` are as documented at docs.protomaps.com/pmtiles/cli.

## First real test (2026-10-20)
The map "did not load correctly" with the small test pack. Likely cause: the viewer opened on the middle of the US at zoom 3.5, while the pack holds only a small area. It now reads the pack's header and opens on the area the pack covers (shown in the note above the map). If the map still looks wrong, the error line under the note and a screenshot say why. **TO TEST.**

## What to check first when you run it
1. The extract finishes and `check_drive` passes.
2. Open the app, Maps: the map should draw, and Pins add on click. If it stays blank, the error line under the map says what failed. The likeliest causes are the page-security setting blocking the tile address on Windows, the style not matching the tile layout, or the map library and `pmtiles` plugin disagreeing on version.
3. Speed on the 8 GB laptop at zoom 12 to 15 (doc 6, risk 5).

## Still to do
Topo layer (contours and shading from elevation data), satellite packs per county, fonts for place names, layer toggles, a warning when a route crosses a no-go area, picture export, GPS where hardware has it.


## Finding a place (2026-10-21)

The Maps screen has a "Find a place" box above the map. It works with no internet, from a list on the
drive (`maps/places/places.tsv`, made by `scripts\get-maps.ps1 -Places` or `-PlacesOnly` from the free
GeoNames data, CC BY 4.0). It finds towns, counties, ZIP codes, parks, lakes, peaks, airports and
hospitals by name ("Denver, CO", "80202", "Pikes Peak"), the 50 states and DC (built in, so they work even
without the list), and a latitude and longitude. **Street addresses are not found**: that needs a far
larger address file (for example OpenAddresses), which is not included yet. The rules and tests are in
`core/src/places.rs`. The map opens on the United States when the street map covers any of it.


## Whole US, bigger map, full screen (2026-10-22)

- `scripts\get-maps.ps1` now cuts the lower 48, Alaska and Hawaii by default (three files in `maps\us\street`). That is tens of gigabytes at zoom 15; use `-MaxZoom 13` or `-Regions conus` for something smaller, and `-Bbox` for one area. Sizes depend on the day's build, so the script prints each file's real size. Run it with `-Places` once to make the offline places list.
- The map is taller (it uses most of the window height). **Full screen** makes the map fill the whole screen and keeps everything: the pin, rendezvous and no-go tools, the Find box, "Zoom to my points", and a side list of your points, route and areas with Save. Esc or "Exit full screen" goes back.
- A tactical readout sits on the map: the coordinates under the pointer and of the map's centre (latitude and longitude, five decimals), and a centre cross-hair that can be switched off.
- Hover text explains each tool.


## Places on the map, names and ZIP search (2026-10-22)

- **Places on the map:** nine groups (emergency and health, drinking water, fuel and charging, food and supplies, shelter and lodging, camping and outdoors, transport, services and civic, military and border), each its own colour, switched on and off from "Places on the map" (also in full screen). They come from the `pois` layer of the street map file, so they need no extra download, and appear as you zoom in. Click a marked place for a card (name, kind, coordinates, "Pin it"). "Find the nearest" lists the closest places of one kind to the middle of the map; it only looks at the part of the map that is on screen. The groups are in `src/lib/mapkinds.ts`.
- **Names:** street, town and water names and the names of marked places are drawn when the font files are in `maps\fonts` (Protomaps basemaps-assets, SIL Open Font License; `get-maps.ps1` fetches them, `-FontsOnly` fetches just them). The program serves them only from that folder and only as `fonts/<name>/<range>.pbf` (`maps::resolve_font_path`, tested). Without them only shapes are drawn.
- **ZIP search:** needs the places list (`maps\places\places.tsv`, GeoNames, CC BY 4.0). The Windows build made by CI now includes the list and the fonts, so ZIP codes, towns, parks and lakes are found with no setup. If a build has no list, the Find box says how to make it.
- **Not built:** street-address search, terrain shading and contours (they need elevation data), live position, and turn-by-turn roads.
- Ideas for this screen came from looking at other offline map programs; the look is our own (dark, amber and blue).
