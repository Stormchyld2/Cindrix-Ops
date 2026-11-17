# 32. A phone copy of the map

On the Maps screen, **Send to phone** cuts the part of the map you are looking at (plus a small margin)
into one file, `user/exports/<name>.cmap`, that you copy to a phone (cable, NFC jump or card).
You tick which kinds to carry (street, topo, aerial photos), choose how sharp each one is, and see
the size before the file is made. Your pins, drawings, areas to avoid, saved route and the directions
on screen go inside the same file. The file is written to a `.part` file first and renamed when done.

After the file is made, a box opens listing the plugged-in drives and cards (a phone that shows as a drive is put first), plus "Choose a folder". Picking one copies the file there (into `Download` on a phone), under a temporary name first, and checks the size. A phone in the usual "charging" mode, or one that only shares files over MTP, may not show as a drive; use "Choose a folder" or copy by hand.

**Over Wi-Fi to Cindrix Pocket (2026-11-17).** Every finished map stays in `user/exports/`, and a phone that joined this Hub with an invite QR (Cindrix Pocket) can list them and copy one from its own Maps screen. The Hub answers two things, for trusted phones only (never a PIN guest, because the file carries the owner's own pins, drawings and areas to avoid): `phone_maps_list` (name, size, when made) and `GET /api/phone_map_read?name=<file>.cmap`. The file is sent piece by piece, never read whole into memory, and honours a `Range` header so a phone can carry on a download that broke. A name must be a plain `.cmap` file name; nothing outside `user/exports/` can be reached. The code is `core/src/phonemaps.rs` (tested there); the streaming is `phone_map_stream` in `src-tauri/src/share.rs`. Pocket downloads into a `.part` file, checks it is a real map, then keeps it.

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

Zstd tiles need a decoder on the phone (Cindrix Pocket carries one; the browser viewer does not yet); gzip is built into browsers. Not offered yet: contours or
hillshade made on the fly, searching places inside the file.
