# 45. Garden beds and crop rotation (2026-11-05)

**What:** `core/src/rotation.rs`. Each bed keeps a history of what grew in it (`user/beds.csv`: `bed,year,crop,notes`).
The checker warns when a plant family would follow itself too soon and suggests what to plant next.

**Why:** the Seed Vault knows what seed you hold, and the Planting Calendar knows when to sow, but nothing knew *where*
a crop went last year. Rotation is the cheapest pest and disease control a home garden has.

**Rules used** (from `content/guides/planting-calendar/succession-and-crop-rotation.md`):
- Nightshade, cabbage family, onion family: wait 3 years before the same family returns to a bed.
- Squash family, root crops: 2 years. Legumes, leafy greens, grains: 1 year.
- After a legume, plant a heavy feeder (cabbage family, corn). After a heavy feeder, roots or onions. Suggestions skip any family still resting.

**API:** `family_of(crop)`, `rest_years(family)`, `check(history, bed, year, crop)`, `check_all(history)`,
`suggest(history, bed, year)`, `parse(csv)`, `to_csv(history)`. Pure logic, std only, nine unit tests.

**Not built yet:** a Svelte screen (beds grid, per-bed history, "plant here instead" hint), linking beds to Seed Vault
rows, and auto-created sowing tasks from the calendar.

**Credit:** the idea of per-bed crop history with rotation checks was inspired by the feature list of the open-source
garden planner *Brinjel* (AGPL-3.0). We read its description only; **no Brinjel code or text is used**, and because it is
AGPL it must stay that way (see `licenses/README.md`).
