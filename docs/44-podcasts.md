# 44. Podcasts

Podcasts are saved episodes, read from the drive like music. Cindrix Radio (the phone app) downloads
them; this page plays them. Nothing here uses the internet.

## The folder

`podcasts/` on the drive (or the folder chosen in Settings > Music > Podcasts folder):

```
podcasts/
  <Show name>/
    show.txt                       optional; first line is the name to show
    2026-11-02 - Episode title.mp3
    2026-10-26 - Earlier episode.mp3
```

- One folder per show. The date in front of the file name orders episodes (newest first); a file with no date is ordered by when it was saved.
- Played in the web view: mp3, m4a, m4b, aac, ogg, opus, wav, flac. Formats that need converting (wma and so on) are not offered here.
- Files dropped straight into `podcasts/` are listed under "Other episodes".

## Getting episodes onto the drive

Copy the `Podcasts` folder from the phone's `Downloads/Cindrix` folder into the drive's `podcasts` folder. Cindrix Radio saves in this same shape, so nothing needs renaming.

## What is built

- `core/src/podcasts.rs` (tested): listing, name and date reading, safe paths, the saved-places file.
- `src-tauri/src/podcasts.rs`: the `cindrix-pod` address the web view plays from (range requests, like music), and the commands `podcasts_list`, `podcasts_progress_load`, `podcasts_progress_save`.
- `src/views/Podcasts.svelte` (page), `src/lib/podplayer.svelte.ts` (player: keeps playing between screens, speed 0.75 to 2, back 15 s, forward 30 s, next episode), `src/lib/PodMiniPlayer.svelte` (bar under the music bar).
- Where each episode was left off is saved in `user/podcast-progress.txt` (host-only). Finished means within 45 seconds of the end.
- Share server: guests may list and play episodes (`/pod/`), but have no saved places.
- Shown in the top bar under Morale, next to Music, on every edition (it follows the music switch in `Features`).

## To test (not yet run on real hardware)

- Windows (WebView2) and Android playback of mp3 and m4a through `cindrix-pod`, including seeking.
- A real Cindrix Radio download folder copied to the drive.
