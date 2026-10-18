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
