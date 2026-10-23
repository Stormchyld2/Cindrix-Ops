# 29. Aerial maps (the topo map was taken out)

Maps has three buttons: **Street**, **Topo**, **Aerial**. Topo and Aerial are greyed out (with hover text saying why) until picture files are on the drive.

## Files
- `maps/us/topo/<name>.pmtiles`: USGS topographic map tiles.
- `maps/satellite/us/<name>.pmtiles`: USGS aerial photo tiles (mostly NAIP, about 1 m).
- Any number of files per layer. The widest file is drawn first, then sharper ones over it, so "whole US at a wide view" and
  "my county sharp" work together. Past a file's last zoom the picture is stretched, until a sharper file covers that spot.
- All street files are also read together now (lower 48, Alaska and Hawaii are separate files).
- Over aerial photos only roads, borders and names of the street map stay on; Topo shows the topo picture alone (it has its own names).

## Download (`scripts/get-imagery.ps1`, `Get-Imagery.bat`)
Tiles come from the USGS National Map tile services (`USGSImageryOnly`, `USGSTopo`) one request at a time, six at once, written to a work folder,
then packed with DuckDB into an MBTiles file and with `pmtiles convert` into PMTiles. Finished zoom levels are kept if it stops.
Without options: whole US, zoom 0 to 13. `-Bbox "w,s,e,n" -Name x -MaxZoom 16` makes one area sharp.

## Size (estimates for the lower 48)
zoom 13 about 15 GB, 14 about 55 GB, 15 about 220 GB, 16 (the sharpest USGS serves) about 1,000 GB. Each is millions of requests to USGS, so
"every house in the whole US" is not offered; the script refuses more than 1,500,000 tiles unless `-Yes` is given.

## Limits
- The CI job `imagery` runs the download on a very small area; the large runs are untested.
- Where USGS has no picture the tile is missing and the area shows dark.
- Credit "USGS The National Map" is on the map (attribution) and in each `.SOURCE.txt`.

## The last step was far too slow (found 2026-10-23, fixed)
A real run for the whole US topo at zoom 12 (648,356 tiles, ~470,000 kept) downloaded fine, but the final "Making the PMTiles file" step
(`pmtiles convert`, pass 2) ran at about one tile a second, an estimated 94 hours. Cause: the packed MBTiles file had no index on
`(zoom_level, tile_column, tile_row)`, and `pmtiles convert` looks every tile up by those three, so each lookup read the file.
The script now makes that index (`Add-TileIndex`: Windows uses the SQLite that ships with Windows, `winsqlite3.dll`; Linux uses the
`sqlite3` program) before converting. It also writes `packed.ok` after packing, so a run that stopped later reuses the packed file,
and `-UseExistingPack` does the same for a file packed by an older copy of the script (the tiles must be fully fetched).
About 5,500 of the 650,000 tiles came back in a different picture format (PNG, not JPEG) and are left out; they show as small gaps.

## The topo map was taken out (owner, 2026-10-23)
The USGS topo download worked (zoom 0 to 12, US wide, after the index fix) but the map looked like blurred colour swirls at every
zoom the owner tried, and flat colour close up, so it was removed rather than kept half working. Gone: the Topo button in Maps,
`-Layer topo` in `get-imagery.ps1`, the topo step in `Get-Imagery.bat`, the CI topo test, the `us-topo` line in `manifest/content.json`.
The code that recognises a `maps/us/topo` folder (`core/src/maps.rs`) and the phone-pack topo kind stay, unused, and a topo file left
on a drive is simply ignored. Cause not found: the geography was in the right place, so it was not scrambled tiles. The leading
suspects were USGS's own generalised small-scale tiles, or tiles from zoom 10 up not being read. If topo is wanted again, build contour
lines from elevation data (the original plan in docs/05) instead of using USGS's pictures.
The `.part` / index / header fixes above still apply to the aerial layer.

