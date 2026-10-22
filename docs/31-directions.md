# 31. Directions (by car and on foot)

The Maps screen has Directions boxes at the top of the left column, above My saved maps. Type a start
and an end (a street address, a town and state, a ZIP code, or a position in any form, see
`core/src/coords.rs`), press Get directions, and the way is drawn on the map with the time and
distance by car and on foot, and the roads to follow.

## Where the roads come from

Nothing new is downloaded. The roads are read out of the street map file already on the drive
(`maps/.../street/*.pmtiles`, the Protomaps layout). Each map tile has a `roads` layer whose lines
carry a `kind` and a `kind_detail` (motorway, primary, residential, footway ...).

- `core/src/inflate.rs`: gzip decoder (the tiles and tables in the file are gzip).
- `core/src/roadtiles.rs`: reads the PMTiles file (header, directories, tile numbers) and the roads
  layer of a vector tile.
- `core/src/routing.rs`: builds the road network and finds the quickest way (A*).
- `src-tauri/src/lib.rs`: `directions` command. `src/lib/api.ts`: `directions()`. The panel is in
  `src/views/Maps.svelte`.

## How the way is worked out

1. Pick a map zoom from the straight-line distance: 14 (every street) up to 12 km, 13 up to 50 km,
   12 up to 200 km, 11 up to 600 km, 10 beyond (to 1,500 km). Lower zooms hold only the bigger roads,
   which keeps a long trip quick. The pack's own zoom range caps this.
2. Read the tiles in a band around the straight line (wider on each retry if no way is found).
3. Cut each road line at the tile edge, join the pieces that meet, and **find crossings by geometry**:
   map tiles drop the corner point of a junction, so two roads that cross, or a side street that ends
   on a main road, have no shared point. Pieces on a bridge or in a tunnel are not joined to the ground.
4. Attach each end to the nearest road that way of travelling may use (within 1.5 km, 3.5 km on the
   coarse zooms). The gap is walked or driven in a straight line and said so.
5. A* with a time cost. Car speeds come from the road kind (motorway 100 km/h down to service roads
   20); paths, footways and steps are foot only; motorways are car only. Walking is 5 km/h (steps 3).
6. Areas drawn with Avoid on the map are kept out of: any stretch of road with a point inside one
   is not used (unless the start or end itself is inside one).

## What it does not know

No one-way streets, turn bans, traffic, closures, road surface, ferries, or stairs and slopes beyond
the road kind. Times are estimates. The answer says so on the panel.

## Tests

`core/` tests build tiny map files (`roadtiles::testkit`) and check crossings, side streets, a road
continuing into the next tile, foot paths, motorways, avoid areas, a far-away place, and a
144-tile town grid.
