# 7. Content manifest

What goes on the 128 GB drive. The machine-readable version is `manifest/content.json`, which the build script reads. **Every size below is an estimate.** The build script records the real size and checksum of each file.

## Space budget (a 128 GB stick holds about 119 GiB; leave 10 GiB free for user data and growth)
| Item | Estimate | Notes |
|---|---|---|
| App, launcher, engines, guide | 1–2 GB | |
| AI models | 8–10 GB | Qwen3.5 2B, 4B and 9B, Q4 quantized |
| Wikipedia (English) | 10–55 GB | Choose a tier below |
| WikiMed, WikEM, medical archives | 2–6 GB | |
| Other reference (Appropedia, how-to, Gutenberg picks) | 3–10 GB | Only items with clear licenses |
| US government manuals (Army, FEMA, CDC, FDA) | 1–3 GB | Public domain |
| Street maps, US-wide | 10–20 GB | OpenStreetMap vector tiles |
| Topo, US-wide | 5–20 GB | Contours and shading from elevation data |
| Emergency cards, guide, drills | under 1 GB | Ours |

A full Wikipedia with pictures would not leave room for maps, so the choice is a tier:

- **Tier A:** Wikipedia English, text only, about 10 GB. More room for maps and other items.
- **Tier B:** Wikipedia English, text plus intro pictures, about 55 GB. Fits only if maps use the smaller estimate.

**OPEN:** pick a tier after the real sizes are measured.

## Rules for every item
Has a source URL, edition and date, license, checksum, and an entry in `SOURCES.md` and `LICENSES/`. Items without an entry do not ship. The drive is sold as a one-time product, so the manifest also records the drive version and build date. See doc 3 for exclusions.

## Machine-readable manifest
`manifest/content.json` lists each item with: `id`, `name`, `category`, `source_url`, `license`, `attribution`, `size_estimate_gb`, `required`, `tier`, `sha256` (filled by the build). The first version has placeholders where the exact file names are not yet confirmed.


## Editions: Go and Vault

One program, one manifest, two builds. The program is a portable folder, so it does not change between editions. Only the contents do. Each manifest item lists the editions it belongs to, and the build script takes an edition name.

| | Go | Vault |
|---|---|---|
| Medium | 128 GB USB stick | 4 TB external drive |
| Wikipedia | English, text only (about 10 GB) | English, with pictures (about 55 GB) |
| Library | WikiMed, WikEM, Appropedia, government manuals | Go, plus full Project Gutenberg, Wikibooks, Wikisource, Stack Exchange DIY/home/outdoors/gardening |
| Maps | US street map and topo | Same, plus satellite imagery (NAIP) preloaded for chosen states or counties |
| Models | 2B, 4B (9B optional) | all three, plus room for more later |
| Extra | | Large free space for the owner's backups, scans and photos |

Notes:
- Format the drive exFAT or NTFS. FAT32 cannot hold a file over 4 GB, and Wikipedia and the map files are larger.
- A spinning 4 TB drive loads the model and big archives a little slower than a stick or SSD and does not like being dropped. An SSD is better for a drive that gets carried.
- Whole-US satellite imagery is far too large for any drive here (many terabytes), so Vault preloads chosen states or counties only.
- The Vault is also the natural off-grid server: plug it into a laptop or mini PC on a hotspot and turn on library sharing.

## Size budgets (hard limits)

- **Go:** content must stay at or under 100 GB, so a 128 GB stick has room left for the owner's files and backups.
- **Vault:** content may go up to 3,400 GB on the 4 TB drive.
- The build script adds up the real file sizes for the chosen edition and fails the build when a budget is exceeded. Today it counts the `library` folder only (models and maps are not counted yet), and the limits are also in `core/src/product.rs`: keep the two in step. The Hub edition has no budget; it serves whatever its drive holds. See [17](17-three-products.md). Today's Go estimate is about 51 GB with the Wikipedia text tier, so it fits with room for more.

## Checking a finished drive

`cd core && cargo run --release --example check_drive -- <drive folder>` reads a built drive and reports:
**MUST FIX** for a missing `cindrix-drive.json` or `library/` folder, content over the edition's budget
(everything except the owner's `user/` folder is counted), a `.zim` that will not open, a `.zim` whose
metadata license is non-commercial (or whose name is wikiHow, iFixit or Hesperian), and a PDF stamped
with a restricted distribution statement; **CHECK BY HAND** for an archive with no license in its metadata
and for PDFs whose statement cannot be read. The PDF check only reads text stored uncompressed, so most
real manuals will land in "check by hand" until the build extracts their text; it never counts as a pass.
It exits with 1 when anything must be fixed. It does not yet check checksums against the manifest.

## Importing .zim files and making your own

- **Add a .zim file (planned next):** an "Add archive" button on the Archives tab that copies a chosen .zim into `library/` and restarts the reader. It shows what the file says about itself (name, size, license) and warns if the license looks non-commercial. This is for the owner's private use. Built drives still go through the manifest rules.
- **Website to .zim:** not built into the app for now. The free tools for it (Kiwix's Zimit crawler, or `zimwriterfs`) need a web crawler or Docker and a lot of internet time. The user guide will explain how to make one on a computer with internet and then add it with the button. Copying other people's sites also raises permission questions, so we do not ship any such copies.
