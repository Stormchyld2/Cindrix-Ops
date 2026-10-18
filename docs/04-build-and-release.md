# 4. Build and release

## Repository
GitHub: `Stormchyld2/Cindrix-Ops`, public. Mockups are in `mockups/`, docs in `docs/`. Code will go in `app/`, `launcher/`, and `android/`.

## Builds (GitHub Actions)
- A Windows job builds the portable folder and a zip.
- An Android job builds the APK.
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

## Updates (suggestion for decision 4)
- The app is open source, so people can copy it. What customers pay for is curation, testing, the printed or PDF guide, support, and tested content updates.
- Sell the loaded drive once. Offer a yearly **update pack** as an optional paid download: a signed zip with checksums.
- Settings gets "Install update pack": it checks the signature, backs up `user/`, replaces `app/`, `engines/`, `library/` and `maps/`, and never changes `user/`.
- Security and safety fixes are free for everyone.
- Content updates by their nature need an internet download once. Apps themselves stay offline.

## Milestones
1. Launcher, app server and interface shell running on Windows, with Chat answering from a small library.
2. Emergency cards and the safety check, with tests.
3. Maps (street and topo), drawing, saving, and exporting.
4. Inventory, Family, Drills, Binder, Start.
5. Android APK.
6. Satellite county downloads.
7. Clinical review, trademark check, then the first sale.
