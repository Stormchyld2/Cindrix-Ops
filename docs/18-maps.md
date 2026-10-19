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
```
powershell -File scripts\get-maps.ps1 -Target D:\Cindrix
```
This downloads the `pmtiles` tool from its GitHub releases, finds a recent Protomaps build, and extracts the contiguous US into `maps\us\street\conus.pmtiles` (add `-Regions conus,alaska,hawaii`). It checks the file starts like a real PMTiles v3 file and writes `SOURCE.txt` beside it. `-MaxZoom 14` or `13` gives a much smaller map; each zoom level roughly doubles the size. The planet file is about 120 GB, and the manifest's 15 GB guess for the US at full zoom is **unmeasured**. `check_drive` then verifies the pack (doc 7).

Things in the script taken from memory or documentation and **not tried**: the `build.protomaps.com/<date>.pmtiles` address pattern (the documented list of builds is https://maps.protomaps.com/builds; pass one with `-Source` if the guess fails), the name of the Windows download on the `pmtiles` releases page, and the `--bbox` and `--maxzoom` options of `pmtiles extract` (documented at docs.protomaps.com/pmtiles/cli).

## What to check first when you run it
1. The extract finishes and `check_drive` passes.
2. Open the app, Maps: the map should draw, and Pins add on click. If it stays blank, the error line under the map says what failed. The likeliest causes are the page-security setting blocking the tile address on Windows, the style not matching the tile layout, or the map library and `pmtiles` plugin disagreeing on version.
3. Speed on the 8 GB laptop at zoom 12 to 15 (doc 6, risk 5).

## Still to do
Topo layer (contours and shading from elevation data), satellite packs per county, fonts for place names, layer toggles, a warning when a route crosses a no-go area, picture export, GPS where hardware has it.
