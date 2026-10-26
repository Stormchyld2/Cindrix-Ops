# Licenses

Everything Cindrix Ops uses or ships, who made it, the license, and what that license asks of us.
This folder travels with the program (in the portable build it is the `LICENSES` folder) and is
shown in the app. The facts here come from each project's own license statement; **check them
again before selling anything** (see "Before you sell a drive" at the bottom). This is not legal advice.

## Full license texts kept here
| File | Covers |
|---|---|
| `Apache-2.0.txt` | Qwen3.5 model files, pdf.js, many Rust libraries |
| `GPL-3.0.txt` | Kiwix tools (`kiwix-serve`), and the reason this program's source is published |
| `LGPL-2.1.txt` | The ffmpeg build used for WMA playback |
| `third-party/` | Made at build time by `scripts/collect-licenses.mjs`: the license file of every interface library that ends up inside the program, and a list of every Rust library with its license |

MIT and BSD licenses are short: each library's own copy is in `third-party/`. The two data licenses
(OpenStreetMap's ODbL and the Creative Commons ones) are summarised in `ATTRIBUTION.md` with links to
their full text.

## What we use
| Part | What it does | License | What we must do |
|---|---|---|---|
| Qwen3.5 (2B, 4B) | The assistant's model | Apache-2.0 | Keep the license and notice that come with the model file |
| llama.cpp | Runs the model | MIT | Keep its copyright and license notice |
| Kiwix tools (`kiwix-serve`) | Serves the offline archives | GPL-3.0 | Offer the source (link below); the program is open source |
| Tauri 2 | The program's window and shell | MIT or Apache-2.0 | Keep notices (in `third-party/`) |
| Svelte, Vite | The interface | MIT | Keep notices |
| MapLibre GL JS | Draws the map | BSD-3-Clause | Keep notice |
| PMTiles (JavaScript) | Reads the map file | BSD-3-Clause | Keep notice |
| @protomaps/basemaps | The map's look | BSD-3-Clause | Keep notice |
| Noto Sans map fonts (Protomaps basemaps-assets) | Lettering on the map | SIL Open Font License 1.1 | Keep `OFL.txt` with the fonts (done by the download script) |
| pdf.js (pdfjs-dist) | Reads PDF books | Apache-2.0 | Keep license and notice |
| Project NOMAD (list of archives, answer-style numbers, golden-question idea) | `content/library-catalog.json` is adapted from its `collections/kiwix-categories.json`; the rest is our own code | Apache-2.0 | Say where it came from (here and in the file's note); its own creator-pack videos are NOT used (a separate license that bars copying) |
| FDA drug labels (openFDA, `scripts/get-druglabels.ps1`) | The label text for medicines, read in Intel > Drug labels | U.S. government information; openFDA terms (https://open.fda.gov/terms/) | Show it as published, say it is not advice and not endorsed by the FDA; the program never downloads it itself |
| Home remedies pages (`content/guides/home-remedies`) | Reworded from NOMAD's `home_remedies.json` and `natural_remedies.json`, which summarize CDC, MedlinePlus and NCCIH pages | Public domain (U.S. government), NOMAD Apache-2.0 | Keep each page's `Source:` line; keep them marked draft until a qualified person has checked them |
| ffmpeg (LGPL build) | Plays WMA by converting for a moment | LGPL-2.1-or-later | Keep license; offer the ffmpeg source (link below); we run it as a separate program |
| Our own code in `core/` and `crypto/` | All the logic, including our own zstd and xz readers | The project's license (see "Our own license") | |

## What we ship or tell people to download
| Content | License | What we must do |
|---|---|---|
| OpenStreetMap map data (via a Protomaps build) | ODbL 1.0 | Show "© OpenStreetMap contributors" on every map (done); if changed data is shared, share it under ODbL |
| GeoNames places list (made by `get-maps.ps1`) | CC BY 4.0 | Credit GeoNames (shown on the Maps screen and in `ATTRIBUTION.md`) |
| Street addresses (made by `get-addresses.ps1` from Overture Maps) | Mixed, per source (see docs.overturemaps.org/attribution) | Credit Overture Maps Foundation and its sources; the download writes `maps/addresses/<name>/SOURCE.txt`. DuckDB (MIT) is fetched by the script, not shipped. |
| USGS aerial photos (made by `get-imagery.ps1`) | Public domain (US government) | Credit "USGS The National Map"; shown on the map and in each `.SOURCE.txt`. DuckDB (MIT) and pmtiles (BSD-3) are fetched by the script, not shipped. |
| Wikipedia, WikiMed, WikEM and similar archives | CC BY-SA 4.0 | Credit and link; share-alike for anything we change |
| US government manuals and guides (Army, FEMA, CDC, FDA, USGS) | Public domain | Record edition and date |
| Project Gutenberg titles | Public domain in the US | Check each title |
| Academy lessons we wrote | Ours | Drafts until reviewed by a qualified person |
| Academy courses someone else wrote | Set by the author in `course.md` (`license:`) | The Academy shows the author; do not ship a course with no license |
| Music and books you add yourself | Yours | Not part of what we ship |

Left out on purpose: anything non-commercial (for example iFixit), anything with an unclear license,
and any copyrighted book (see doc 3).

## Where to get the source we must offer
- This program: the repository https://github.com/Stormchyld2/Cindrix-Ops
- Kiwix tools: https://github.com/kiwix/kiwix-tools
- ffmpeg: https://ffmpeg.org/download.html (the build we use is from https://github.com/BtbN/FFmpeg-Builds)
- llama.cpp: https://github.com/ggml-org/llama.cpp

## Our own license
**Not decided yet.** The owner wants our own code limited to personal use with no commercial use.
That cannot be done under the GPL (the GPL forbids such limits) and is not "open source" by the usual
definition; it is a "source-available" or proprietary license. It looks possible here because Kiwix
and ffmpeg run as separate programs and the libraries inside the program are MIT, BSD or Apache, but a
lawyer should confirm that before the first drive is sold. Until a `LICENSE` file is added, nobody else
has permission to reuse the code. This is not legal advice.

## Before you sell a drive
1. Open `content/sources.json` and `manifest/content.json`: every item on the drive needs a source, an edition and a license.
2. Run `node scripts/collect-licenses.mjs` (the build does this) and read `third-party/rust-crates.csv` for any license that is not MIT, Apache-2.0 or BSD.
3. Put `LICENSES/` on the drive next to the program, and keep "Licenses and sources" reachable from the app.

## Seed vault guides and course
Written for this project. Facts are paraphrased in our own words from university extension guidance (Purdue, University of Wisconsin-Madison, Oregon State, Penn State) and USDA pages. Nothing is copied. Drafts, not yet reviewed.

## Agro guides and radio reference
Written for this project (`content/guides`). Facts are paraphrased in our own words from general home-gardening and livestock practice (state Extension services), USDA, NOAA, USDA FSIS / FoodSafety.gov, EPA, the FCC (47 CFR Part 95), ITU and ICAO. US government material is public domain. Nothing is copied. Drafts, not yet reviewed. Home canning processing times are left out on purpose: see the USDA Complete Guide to Home Canning.
