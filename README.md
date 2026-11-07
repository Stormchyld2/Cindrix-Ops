# Cindrix Ops Survival Library

*Carry the library. Keep the fire.*

An offline survival library for Windows and Android: a local AI assistant that answers from the library on the drive and shows its sources, emergency cards, offline maps, drills, family information, and an inventory.

**Status (2026-11-07): the Windows program is feature-built and passes its automated checks in CI, but no person has run it on real hardware yet. That first real test is the next milestone: docs/22-real-test.md. Android (the Hub phone) has not been started; it waits for the Windows test. What is still open is listed in docs/49-status-and-open-work.md.**

What exists: the Dashboard, Ask (local assistant with chat history, source links, projects, calendar and to-do), Library and archive reader (ZIM files read directly), Intel, Academy (course folders), Plan and Seed vault, Inventory with shelf life and places, Maps (street, topo, aerial, drawing, directions, MGRS grid, phone map export), Music, Morale (Reading Corner, Journal, Photos), and the share server for other devices on the hotspot. The theme is charcoal and blue (docs/41-theme.md).

## Rules for this project
- Shipped content is open-source and free (public domain, CC BY / CC BY-SA, Apache 2.0, MIT, GPL where required). The owner's private drive may hold other material by his own choice; only the version for sale is stripped to freely licensed and public-domain material (docs/05-open-decisions.md).
- Every source and license is listed in the app and in a LICENSES folder.
- The assistant never gives prescription-drug or insulin doses (herbal, home-remedy and kitchen amounts are allowed).
- Everything runs offline. Satellite photos (USGS NAIP) are an optional per-county download in Settings.

## Layout
- `core/`: Rust core logic (safety check, cards, inventory, library search, the Go/Vault/Hub product setup). No outside dependencies. `cd core && cargo test`
- `src/`, `src-tauri/`: Svelte 5 + TypeScript interface and Tauri 2 shell (built in CI; Windows build is the portable download; see docs/13 and docs/22)
- `content/`: shipped cards, drills, guides, plans, Academy courses and seed guides
- `crypto/`: passphrase lock
- `tests/safety/`, `core/tests/safety/`: medication safety test cases
- `docs/`: architecture docs
- `mockups/`: HTML screens for the interface concept. They still show the old amber look; the screens now follow docs/41-theme.md.
