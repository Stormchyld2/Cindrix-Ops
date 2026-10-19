# 4. Build and release

## Repository
GitHub: `Stormchyld2/Cindrix-Ops`, public. Mockups are in `mockups/`, docs in `docs/`. Code is in `core/` (Rust core), `crypto/` (passphrase lock), `src-tauri/` (Tauri shell) and `src/` (Svelte interface); `scripts/` holds the drive build script.

## Builds (GitHub Actions)
- A Windows job builds the Tauri app as a portable folder and a zip.
- An Android job builds the APK with the Tauri Android toolchain.
- A test job runs the Rust tests and the safety tests (`tests/safety/dosing.json`) on every change.
- **The finished drive includes everything, built in.** Models, library archives, and US street and topo maps are all on the drive, so it works fully offline with nothing to download. Only satellite counties are added by the user afterward.
- **The repository holds the recipe, not the big files.** GitHub rejects files over 100 MB and is not meant for tens of gigabytes. A build script downloads each item from its original source, checks it, and assembles the complete drive folder, recording exactly which edition of each source was used. The result is the drive image: a folder you copy onto the stick.
- The script runs on your Windows PC (my workspace can't download large files). GitHub Actions builds the app and the APK; the full drive image is assembled locally.
- **TO CHECK:** Actions limits, and whether to publish the finished image as split release files (each under 2 GB).

## Testing
1. **Windows first.** Test on the PC folder before any drive. Machines: the i7-8700 desktop (16 GB) and the 8 GB laptop (reported as an i7-8700). Check speed of the 2B and 4B models, memory use, start-up time, and the SmartScreen warning.
2. **Drive.** Not needed yet. When the build is ready, test the 128 GB stick for fake capacity (F3 or H2testw) before loading it.
3. **Android.** Galaxy Note 20 with 8 GB: install the APK, check the Play Protect notice, model speed, battery, offline maps, and GPS.
4. **Offline.** With networking disabled, every feature except downloads must work.
5. **Safety tests.** A fixed list of dosing questions must always get the fixed reply. This list runs on every build.

## Updates (decided)
- **One-time sale. No update server and no subscription.** A new version is a new drive, and buyers can buy it again if they want the newer content. Each drive shows its version and build date in Settings and on the label.
- Old drives keep working forever. Nothing expires and nothing phones home.
- **Safety notices:** if a real safety problem is found, a notice and the fix are published on the repository's GitHub releases page, which needs no server of ours. The user guide tells owners where to look. This is optional for owners and costs nothing to run.
- The user's own files live in `user/` and are never touched by a copy of newer app files, so an owner can move their data to a new drive by copying that folder.

## Milestones
1. Launcher, app server and interface shell running on Windows, with Chat answering from a small library.
2. Emergency cards and the safety check, with tests.
3. Maps (street and topo), drawing, saving, and exporting.
4. Inventory, Family, Drills, Binder, Start.
5. Android APK.
6. Satellite county downloads.
7. Clinical review, trademark check, then the first sale.
