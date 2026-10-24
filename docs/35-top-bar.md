# 35. The top bar and the Dashboard

Mockup: `mockups/Navigation.html` (approved by the owner, 2026-10-23). Only the top bar and the Dashboard's look changed. Every screen behind them is the same as before.

## The top bar

Ten words, each with a small coloured diamond. A word with a menu opens it on a click; a click anywhere else, or Escape, closes it. Under 900 pixels wide the bar folds into a ☰ button.

| Word | Opens |
|---|---|
| Dashboard, Ask, Maps | the screen itself |
| Operations | Get ready, Plans, Drills, Inventory, Family, Binder, What fails next, Calendar, To-do list, Projects (doc 37) |
| Intel | Offline browser, Shelf, Archives, Emergency cards |
| Academy | Courses, Training sign-off |
| Agro | Crop guides, Planting calendar, Food preservation, Livestock, Soil and water (62 pages, `content/guides`; widened 2026-10-23 from 13 starter pages, now with farm safety, sanitation, flood and contaminated ground, land suitability, breeding, pollinators, wheat-to-flour and a growing-enough-to-eat page), Seed vault |
| Morale | Reading Corner, Music, Journal, Photos |
| Comms | Comms plan, Radio reference (30 pages, 2026-10-24), Sharing |
| System | Settings, My details |

Screens that are not built show the "Not built yet" page (`Soon.svelte`). Nothing in a menu is hidden to look finished.

Products still switch parts off through `Features` (`product.rs`): a word with nothing left in it disappears.

## How it is wired (`src/App.svelte`)

- The old screen state (`group`, `plan`, `lib`) is unchanged, so the Dashboard's links, Ask's "Open the source" button and Settings' start screen all keep working.
- `tops` describes the bar. Each menu entry has a `tab` that goes to `goTo`: a screen name, `soon:<title>`, `settings:<tab>` or `training`.
- `viewRequest` (`chat.svelte.ts`) carries a Settings tab or the Training sign-off to the screen that opens.
- Colours are `--cat-*` in `app.css`: Operations amber, Intel blue, Academy terracotta, Agro green, Maps teal, Morale pink, Comms purple, System grey.
- `scripts/shots.mjs` has a `nav(page, top, item)` helper that opens the ☰ button first on a phone.

## The Dashboard

Same blocks, tiles and panels as before, with a coloured left edge and a condensed heading on each panel. New: shortcut blocks for Agro and Comms, and a Sharing panel (on or off, hotspot, devices signed in).

Each status tile (Water, Food, Running low, Past or near their date, Plan checks due, Drills done) opens a pop-up that lists what the number is made of, with a button to the screen where it is fixed.

## Not built yet

Everything that is only a proposal in the architecture doc: the CNAR data pack, frost-date tables and the regional layer behind Agro.

On another device (a guest on the hotspot) the menus hide what is host-only: Journal, Comms plan, Sharing and My details.

## Settings

The tabs now run down the left side in three groups (You, The screens, The drive) with the same content as before. The Drive tab is shown as "Drives and libraries" and the Library tab as "Shelf and Archives".
