# Cindrix Ops Survival Library

*Carry the library. Keep the fire.*

An offline survival library for Windows and Android: a local AI assistant that answers from the library on the drive and shows its sources, emergency cards, offline maps, drills, family information, and an inventory.

**Status: first slice in progress. The Rust core is written and tested; the interface and Tauri shell are written but not yet built. See docs/13-run-the-first-slice.md.**

## Rules for this project
- Open-source and free content only (public domain, CC BY / CC BY-SA, Apache 2.0, MIT, GPL where required). No copyrighted books.
- Every source and license is listed in the app and in a LICENSES folder.
- The assistant never gives insulin or other medication doses. Medical cards are drafts until a clinician reviews them.
- Everything runs offline. Satellite photos (USGS NAIP) are an optional per-county download in Settings.

## Layout
- `core/`: Rust core logic (safety check, cards, inventory, library search). No outside dependencies. `cd core && cargo test`
- `src/`, `src-tauri/`: Svelte + TypeScript interface and Tauri shell (first slice, not yet built; see docs/13)
- `content/`: sample emergency cards and sample library (drafts)
- `tests/safety/`: medication safety test cases
- `docs/`: architecture docs
- `mockups/`: HTML screens for the interface concept (Chat, Library, Maps, What fails next, Inventory, Drills, Family, Binder, Devices, Sources, Android, Settings, Start). Sample content only.
