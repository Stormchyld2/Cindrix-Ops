# 42. Shelf life, places inside places, and packaging

Three small additions to the Inventory page. Host-only, like the rest of the inventory. Nothing goes online.

## Shelf-life guess
- `content/shelf-life.csv` (columns: words, kind, food, store_months, long_months, note). Words are separated by `|`; the longest matching word wins; names with "canned", "can of" and so on look at canned rows first.
- Store-bought months are the lower end of the Kansas State publication MF3131 ranges. Long-term months come from the longer-term storage chart and only apply when the food is packed for it (see below) and kept dry at 75°F or cooler.
- `shelflife::suggest_use_by(name, packaging, bought)` returns a use-by month. With no bought month, this month is used and the screen says so.
- The guess is never written on its own. The person presses "Suggest a use-by date", reads it, and presses "Use". The date on the package always comes first.
- No medicine or supplement rows. A test checks this.

## Packaging
Saved as a comma list in the new last column `packaging`: `mylar`, `o2`, `vacuum`, `sealed-can`, `jar`, `bucket`, `desiccant`. Older files without the column still load and gain it on the next save.
Counts as long-term packing: sealed metal can, vacuum sealed, or oxygen absorber inside a foil pouch or glass jar. A bucket with an absorber alone does not count (lids leak).

## Places
A location may be written as a trail with `>`: `Garage > Shelf 2 > Black pail`. Plain text such as `Pantry` works as before. A `/` stays part of a name.
- `placepath::places`, `children`, `is_inside`, `where_is`.
- The Inventory chips open a place and show what is one step inside it. The search box also answers "where is it?" and a click opens the item.

## Commands
`inventory_places`, `inventory_where`, `shelf_life_suggest` in `src-tauri/src/lib.rs`; none is allowed for guests.
