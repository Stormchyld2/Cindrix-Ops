# 5. Open decisions

| # | Decision | Notes |
|---|---|---|
| 1 | File and folder storage | Agreed in principle: plain files, no database. Exact layout is in doc 2. To confirm against the earlier project the author mentioned. |
| 2 | Encryption and passphrase lock for `family.json` | Decided: optional passphrase, Family page only. See doc 9. |
| 3 | Map viewer for the interface | Proposed: Protomaps (PMTiles) with a JavaScript viewer on Windows; Organic Maps for phone companion use. **TO TEST.** |
| 4 | Update and sales model | Decided: one-time sale, no update server. A new drive is a new version. Safety notices go on GitHub releases. See doc 4. |
| 5 | GitHub builds | Yes. Check Actions limits. |
| 6 | Map coverage | US-wide street and topo maps are built into the drive. Satellite is the user's own state and county download from USGS (Settings). |
| 7 | Stack | Decided: Rust, Tauri 2, Svelte, TypeScript. See doc 6. Android and llama.cpp are the main risks to test first. |
| 8 | Accent color | Amber for the flame and OPS, a signal blue (#5aa9e6, already used on the mockup map) for interactive items, red for emergency cards. Default unless changed. |
| 9 | Hardware facts | Laptop reported as i7-8700 with 8 GB RAM. Drive not needed yet. |
| 10 | Clinician for the Diabetes Pack | Who reviews, and when. |
| 11 | Legal | Closed. The author has checked the licensing and the name. |
| 12 | SERE material | Author's own work, to be reviewed before it is included. |
| 13 | Wikipedia tier | Text only (about 10 GB) or with pictures (about 55 GB). Decide after real sizes are measured. See doc 7. |
