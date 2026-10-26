# Cindrix Ops: notes for Claude

Offline survival-library program. One code base, three products (Go, Vault, Hub); the drive's
`cindrix-drive.json` says which one it is, and `core/src/product.rs` turns that into feature flags.
Windows comes first; Android and iPhone wrap it later.

## Layout

- `core/`: Rust library, **standard library only** (no outside crates; zstd and xz decoders are
  our own so it also builds for Android). All logic that can be tested lives here.
  `drive.rs` (finding drives by marker) and `libraries.rs` (which drives may be read, remembered answers),
  `retrieval.rs` (keyword passages for the assistant), `shelf.rs` (books), `library.rs` and
  `zimread.rs` (ZIM archives read directly), `kiwix.rs` (kiwix-serve helpers), `pipeline.rs` (ask).
- `src-tauri/src/lib.rs`: Tauri commands and app state. `zimview.rs` serves the `cindrix-zim`
  address for the archive reader. `music.rs` is the music player's side. `libs.rs` is the drive
  question and the library list (`all_zim_files()` is the one place archives are listed from).
- `src/`: Svelte 5 interface. `App.svelte` holds the top navigation (groups with click menus, `docs/35-top-bar.md`); screens are in `src/views/`;
  `src/lib/api.ts` is the only place that calls Tauri commands. `src/lib/Splash.svelte` is the loading screen (logo and background are in `public/`); it shows in the real program, and in a browser only with `?splash`.
- `core/src/routing.rs` (directions by car and on foot from the roads inside the street map), `roadtiles.rs` (reads the roads layer of a `.pmtiles` file) and `inflate.rs` (gzip): see `docs/31-directions.md`. `core/src/phonepack.rs` cuts a view of the map into one `.cmap` file for a phone: `docs/32-phone-map.md`. `core/src/share.rs` (PIN, guest list, request rules) and `hotspot.rs` are the share server's tested parts; the listener is `src-tauri/src/share.rs`: `docs/34-share-server.md`. `core/src/profile.rs` is Settings > My details (`user/profile.txt`, host-only): `docs/33-my-details.md`.
- `core/src/academy.rs` (course folders), `plans.rs` (plan files and progress), `cards.rs` (emergency cards from several `cards` folders), `training.rs` (per-person Academy sign-off) and `stockplan.rs` (how much to have): all read plain files, skip anything with a medicine dose, and are tested in `core/`. People's names come from the Family page.
- `core/src/druglabels.rs` reads the FDA's drug label files (fetched by `scripts/get-druglabels.ps1`) into one searchable list; screen `src/views/DrugLabels.svelte`, commands in `src-tauri/src/druglabels.rs`; host-only; docs/38.
- `core/src/ops.rs` is the connecting layer (to-do list, calendar, projects, the agenda, one search, links, change log; `user/tasks.txt`, `events.txt`, `projects.txt`): `docs/37-connecting-layer.md`. Host-only: none of its commands is allowed for guests. `core/src/seasons.rs` works out planting times and frost days for the calendar from the seed vault and My details.
- `src/lib/tooltips.ts` gives every button hover text (fallback for new ones); `src/lib/planfacts.ts` works out what plan steps can tick themselves from.
- `crypto/`: passphrase lock. `content/`: shipped sample content (cards, academy, plans). `docs/`: design docs, numbered. `licenses/`: license texts and credits; `scripts/collect-licenses.mjs` adds the libraries' own at build time.
- `mockups/`: design mockups. Match them when changing a screen.

## Direction (decided 2026-10-21, not built yet; details in `docs/24-hosts-libraries-browser-clients.md`)

- Go carries program and content on one stick. Vault and Hub are one exFAT drive of content; the program
  runs on a host (Windows laptop for Vault, Android phone for Hub). The drive also holds a rescue copy of
  the portable Windows build and the Android app.
- Other devices use a browser tab, never a client app. Keep `src/lib/api.ts` the only place the interface
  reaches the backend, and put new logic in `core/` so a second (HTTP) transport can use it too.
- Only drives with `cindrix-drive.json` count as libraries, are remembered by the marker's `id` (the
  edition word: `vault`, `go`, `hub`; not the drive letter), and the person is asked before one is read.
  Several libraries can be on at once. (Built for archives and loose files; see doc 24.)
- Sharing stays off at every start, and switches on by itself only when a hotspot starts (owner, 2026-10-22; docs/34). Guests need a PIN and are read-only (a PIN never makes a named person; a one-use invite QR makes a trusted family device that may add pictures to the shared album and its own, docs/34) and get all library content (archives, Shelf,
  music, maps, Academy, Ask); the family page, inventory and the person's own notes stay host-only.
  Do not rely on browser features that need HTTPS.
- The Hub assistant is on by default with the model file on the drive; memory on the phone is the limit.
- Windows first: the share server is built (doc 34) and waits for its real Windows test (doc 22); Android waits for that.

## Checks (CI runs these; see `.github/workflows/ci.yml`)

- `cd core && cargo test --all-targets` (the music tests want `ffmpeg`)
- `cd crypto && cargo test`
- `npm install && npm run check && npm run build`
- The Windows app is built only in CI (`npx tauri build --no-bundle`).

## Rules that matter

- The app never uses the internet, except "Save a website" while it runs. Nothing new may phone out.
- UI text is plain words for a non-technical person. No jargon, no blame in error messages.
- The assistant never gives medicine or insulin doses (`core/tests/safety/dosing.json`).
- Products switch screens on or off through `Features` in `product.rs`, not by asking which product.
- A user's own files live under `user/` on the drive as plain files. Never overwrite a damaged file
  silently; use `fsio::backup_then_write` / `write_atomic`.
- Add a test in `core/` for every new piece of logic, and keep `docs/` in step with what is built.
- Anything new that is shipped or downloaded needs a line in `content/sources.json` and `licenses/README.md`.
- `core/src/seeds.rs` is the Seed vault (Plan > Seed vault); its guides are `content/seed-guides`, its course `content/academy/seed-vault`.
- New cards, plans and categories need no code: a `.md` file in `content/cards` or `content/plans` (templates: `_template-card.md`, `_template-plan.md`, `_template-keep-it-from-failing.md`). Every card needs a `Source:` line.
- The program opens on the Dashboard. New buttons get hover text automatically; give important ones their own `title`.
