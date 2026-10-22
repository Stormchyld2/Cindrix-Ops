# 30. Drawing on the map

The Maps screen is laid out like Google Maps: a search box and a tool row float at the top left of the map, the Map / Topo / Satellite
switcher at the bottom left, zoom buttons at the bottom right, and a details card appears at the right when something you drew is picked.

## Tools (`src/views/Maps.svelte`, `src/lib/mapdraw.ts`)
Move (pick things; drag a pin or a corner to move it), Pin, Meet, Avoid (no-go area), Line, Shape, Box, Circle, Text, Measure, Erase, Undo, Redo.
- Line and Shape: click each point, then Finish (or Enter); Esc cancels. Box: two opposite corners. Circle: middle, then edge.
- Text: type the words, click where they go. Any drawing can be named; the name shows on the map.
- A picked drawing shows colour, thickness, its length / area / radius, Delete (also the Delete key), and round handles to reshape it.
- Measure shows the distance along the clicked points and the area if the points were closed. It is not saved.
- Erase removes a drawing, a pin or a no-go area; Undo (Ctrl+Z) and Redo (Ctrl+Y) cover every change.
- Distances and areas follow the unit setting (miles / feet / acres or km / m / hectares).

## Saved with the map
`drawings` is added to the saved map file (`user/saved-maps`): `{ id, kind, label, color, width, points, radiusM? }`. Older maps without it open as before.

## Limits
- Routes are still straight lines between your pins; there is no road routing offline.
- The tools were checked by the build's type check and screenshots, not on a real drive.
