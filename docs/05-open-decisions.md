# 5. Open decisions

| # | Decision | Notes |
|---|---|---|
| 1 | File and folder storage | Agreed in principle: plain files, no database. Exact layout is in doc 2. To confirm against the earlier project the author mentioned. |
| 2 | Encryption and passphrase lock for `family.json` | Decided: optional passphrase, Family page only. See doc 9. |
| 3 | Map viewer for the interface | Chosen and coded, not yet run on real tiles: MapLibre GL JS reading Protomaps PMTiles through the shell's `cindrix-tiles` protocol. See [18](18-maps.md). **TO TEST.** Organic Maps stays an option for phone companion use. |
| 4 | Update and sales model | Decided: one-time sale, no update server. A new drive is a new version. Safety notices go on GitHub releases. See doc 4. |
| 5 | GitHub builds | Yes. Check Actions limits. |
| 6 | Map coverage | US-wide street and topo maps are built into the drive. Satellite is the user's own state and county download from USGS (Settings). |
| 7 | Stack | Decided: Rust, Tauri 2, Svelte, TypeScript. See doc 6. Android and llama.cpp are the main risks to test first. |
| 8 | Accent color | Amber for the flame and OPS, a signal blue (#5aa9e6, already used on the mockup map) for interactive items, red for emergency cards. Default unless changed. |
| 9 | Hardware facts | Laptop reported as i7-8700 with 8 GB RAM. Drive not needed yet. |
| 10 | Clinician for the Diabetes Pack | **Deferred (2026-10-20).** The Diabetes Pack comes later, with a herbal approach for when no medication is available. It still needs a qualified reviewer before it ships, and it must say plainly what herbs cannot replace (for example insulin for type 1 diabetes). The assistant keeps refusing medication doses. |
| 11 | Legal | Closed. The author has checked the licensing and the name. |
| 12 | SERE material | Author's own work, to be reviewed before it is included. |
| 13 | Wikipedia tier | Text only (about 10 GB) or with pictures (about 55 GB). Decide after real sizes are measured. See doc 7. |

## Decided 2026-10-18
- Three products from one code base: Cindrix Ops Go (128 GB stick, content up to 100 GB), Cindrix Ops Vault (4 TB drive, up to 3400 GB), Cindrix Ops Hub (own server for phones on its own Wi-Fi). Spelling: Cindrix. See [17](17-three-products.md).
- The Hub is our own build on `cindrix-core` with `kiwix-serve` behind it, not a repackaged Internet-in-a-Box. Order: finish the base app, then Go, then Vault, then Hub.
- The Hub is the same program as Go and Vault, run as a server (drive marker `edition: hub`), not a separate stripped-down app. Its first target is an Android tablet with a 4 TB drive and the tablet's own hotspot, with the normal app screens as the viewer on the tablet. See [17](17-three-products.md).
- **OPEN:** Hub has the assistant off by default; confirm. Which tablet and Android version, and whether the 4 TB drive is a spinning disk or an SSD.

## Decided 2026-10-20
- **Windows first.** Android, the Hub's web server and the other editions wait until the Windows version has been tested for real. See [22](22-real-test.md).
- The Diabetes Pack is deferred (decision 10).

## Decided 2026-10-21
- Vault is the core program only, installed on the laptop and reading content from the drive; Vault and Hub are two hosts for the same exFAT drive. Go keeps program and content on one stick. See [24](24-hosts-libraries-browser-clients.md).
- Other devices use a browser tab, not an app. The drive also carries a rescue copy of the portable Windows build and the Android app.
- The program lists every Cindrix drive (marker file) and asks before reading one: Always, This time only or Never, remembered by an id in the marker. No default pick, except that "Always" is suggested for the main library drive.
- The "main" drive is the 4 TB external drive. All library content (archives, Shelf, music, maps, Academy, cards, drills, Ask) is shared to browsers, replacing doc 17's "archives only". Content lives on Go (stripped down) or on the 4 TB drive.
- The Hub assistant stays on by default, with the model file on the drive. Memory, not storage, is the limit on the Note 10; to test early. See [24](24-hosts-libraries-browser-clients.md).
- The marker's `id` for the owner's drives is the edition word (`vault`, `go`, `hub`); a marker without an id uses its edition word. Built 2026-10-21 (doc 24, plan step 1).
- **OPEN:** whether the family page, inventory and notes (not content) stay host-only, as planned; whether Hub borrows a Vault laptop's assistant later.
