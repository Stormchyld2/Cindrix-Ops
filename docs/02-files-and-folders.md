# 2. Files and folders

No database. The drive is a folder, user data is plain files you can open, copy and back up.

## Drive layout
```
Cindrix/
  Start-Cindrix.exe          launcher (Windows)
  README.txt                 how to start, in plain words
  guide/                     the complete user guide (HTML and PDF)
  app/                       interface and app server
  engines/                   llama.cpp, Kiwix tools
  models/                    Qwen3.5 GGUF files
  library/                   archives (.zim), manuals, emergency cards
  maps/
    us/street/               US street map tiles
    us/topo/                 elevation contours and shading
    satellite/<state>/<county>/   optional, added in Settings
  user/                      EVERYTHING the user creates (never overwritten by updates)
    family.json
    inventory.csv
    saved-maps/              one folder per map: map.json, picture.png, route.gpx
    drills/                  progress (json) and debrief notes (md)
    binder/                  last built binder (pdf, export only)
    settings.json
    backups/                 dated copies, made before risky changes
  LICENSES/                  full license text for every component and content source
  SOURCES.md                 every content source, edition, date, license
```

## Rules for user files
- **Five formats only for stored data:** JSON, MD, CSV, PNG and JPEG.
  - **JSON:** structured data (family, settings, saved map contents, drill progress).
  - **MD (Markdown):** anything written to be read: notes, drill debriefs, emergency cards, the user guide.
  - **CSV:** the inventory (opens in any spreadsheet).
  - **PNG:** exported map pictures and screenshots.
  - **JPEG:** photos, including NAIP aerial tiles.
  - **Export-only extras, not used for storage:** GPX (so phone map apps can open a route) and PDF (printing the binder). Say if you want them dropped.
- **Safe writes:** write to a temporary file, then rename, so a power loss never leaves a half-written file.
- **Backups:** copy the previous version into `user/backups/` before saving over family data or inventory.
- **Versioned:** every JSON file has a `"version"` number so later releases can migrate it.
- **Updates never touch `user/`.**
- **Portable:** moving or copying the Cindrix folder to another drive keeps everything.

## Inventory file (`inventory.csv`)
One row per item: id, category, name, location, quantity, unit, use per day, use by (month), notes. Medicine rows hold counts only, never doses. Other screens read this file: What fails next, the Binder shopping list, and the Start checklist.

## Saved maps
Each saved map is a folder with:
- `map.json`: view, pins, rendezvous points, route lines, no-go areas, labels
- `picture.png`: the exported picture
- `notes.md`: written notes for the map
- `route.gpx`: optional export, for phone map apps

## Family file
Names, conditions, medications as written by the doctor, allergies, contacts. An optional passphrase lock is **OPEN** (decision 2). Until it is decided, the interface warns that the drive is not encrypted.

## Android
The app keeps `user/` in a folder the person chooses, using the system file picker, so they can see and back it up. Large content packs are copied from the drive into the same area.
