# 29. Topo and aerial maps

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
