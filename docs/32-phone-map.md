# 32. A phone copy of the map

On the Maps screen, **Send to phone** cuts the part of the map you are looking at (plus a small margin)
into one file, `user/exports/<name>.cmap`, that you copy to a phone (cable, NFC jump or card).
You tick which kinds to carry (street, topo, aerial photos), choose how sharp each one is, and see
the size before the file is made. Your pins, drawings, areas to avoid, saved route and the directions
on screen go inside the same file. The file is written to a `.part` file first and renamed when done.

The code is `core/src/phonepack.rs` (tested there). The viewer that opens these files on the phone is a
separate project; this page is the format it reads.

## The `.cmap` format (version 1)

All numbers are little-endian.

    "CMAP0001"                       8 bytes
    layer data, glyph files          raw bytes, in any order the manifest points to
    tile lists                       per layer: index_count x 20 bytes
    manifest                         JSON (UTF-8)
    manifest length                  u32
    "CMAPEND1"                       8 bytes

A reader takes the last 12 bytes, checks `CMAPEND1`, reads the length, then the manifest just before it.

Each tile list entry is five u32: `z, x, y, offset, length`. The tile bytes are at
`data_offset + offset` for `length` bytes, exactly as they were in the source `.pmtiles` file
(still compressed as the layer's `compression` says; z/x/y are standard web-map tile numbers).

Manifest fields: `format` (1), `name`, `created` (seconds since 1970), `bbox` [west, south, east, north],
`view` {center [lat, lon], zoom}, `layers` [{kind `street|topo|aerial`, tile_type `mvt|png|jpeg|webp|avif`,
compression `none|gzip|zstd`, min_zoom, max_zoom, tiles, data_offset, data_length, index_offset,
index_count}], `glyphs` [{font, range `0-255|256-511`, offset, length}] (lettering for the street map,
only when a street layer is carried), and `map`: the person's own map object
(`pins`, `route`, `nogo`, `drawings`, `directions`) as saved by the Maps screen.

Zstd tiles need a decoder on the phone; gzip is built into browsers. Not offered yet: contours or
hillshade made on the fly, searching places inside the file.
