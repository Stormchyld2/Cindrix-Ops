# 19. Music

A lightweight music player in all three products (Go, Vault, Hub). It plays the owner's own music from
the `music/` folder on the drive. The program adds no music of its own.

## What plays

| Plays directly | Converted once, then kept |
|---|---|
| MP3, WAV, FLAC, OGG, Opus, M4A, AAC | WMA, AIFF, APE, WavPack, Musepack, TTA, AMR, AU, AC3, MKA, MP2 |

The web view that draws the app plays the first group itself. It cannot play the second group, so
the program hands that file to **ffmpeg** (a separate program in `engines/`, started the same way as the
model and the archive server). ffmpeg writes a small Opus copy (about 160 kbit/s; FLAC if that ffmpeg has
no Opus encoder) into `user/cache/music/`, and that copy plays. The original is never touched. The
first play of such a file waits a few seconds; after that it is instant. Deleting the copies is safe
(Music, bottom of the Library tab, "Delete them").

ffmpeg is told to read local files only, so a damaged or hostile file cannot make it fetch anything.
Use an **LGPL** build of ffmpeg (the Windows build job downloads one). Without ffmpeg, everything in
the first column still works and the screen says plainly which files need it.

On Android there is no way to run ffmpeg, so WMA and similar must be converted on a PC first (copy the
converted files into `music/`). Everything in the first column plays on Android as it is.

## What it does

- **Library:** search (every word must match the title, artist, album or path), browse by Artists,
  Albums or Folders, Play all, Shuffle, Play next, Add to queue, Add to playlist.
- **Queue:** reorder, remove, jump, save as a playlist. Shuffle, repeat (off, all, one).
- **Playlists:** plain CSV files in `user/playlists/` (path, title, artist, album). A song that has
  moved is skipped and the screen says how many.
- **Equalizer:** ten bands (31 Hz to 16 kHz, plus or minus 12 dB) and eight presets. It is built only
  the first time a slider is moved, so ordinary playback has no extra processing.
- **Speed** (0.5 to 2 times), **sleep timer** (fades out for 15 seconds, then pauses), volume with a
  curve that is gentle at low levels.
- **Lock screen and keyboard media keys** (play, pause, next, previous, seek) through the browser's
  media session, with the cover picture.
- A **mini player** stays at the bottom of every screen. What was playing (queue, place in the song,
  volume, equalizer) is restored next time, from `user/music-state.json`.

## How it stays light

- No database and no outside libraries. Tags are read by small readers in `core/src/media/tags.rs`
  for ID3v1 and v2.2 to v2.4, FLAC and Ogg/Opus comments, WAV, M4A and WMA. Each reads a bounded amount
  and cannot panic on a broken file. Covers are read from the file (or `cover.jpg`, `folder.jpg`,
  `front.jpg`, `album.jpg` in the folder).
- The song list is kept in memory and saved to `user/cache/music-index.csv`. A scan reads only files
  whose size or date changed, so a library of tens of thousands of songs opens quickly after the first
  time. The scan runs in the background; the screen is usable at once.
- The screen asks for 200 songs at a time.
- Files are streamed in pieces (HTTP Range) through the `cindrix-music` address, so seeking works and
  a large file is never loaded whole.

## Safety rules

- Only audio files under `music/`, converted copies in the cache folder, and cover pictures are ever
  handed to the web view. Parent folders, absolute paths, links that leave `music/`, and other file
  types are refused (`core/src/media/serve.rs`, with tests).
- Hidden and system folders are skipped; links are not followed. Names Windows cannot use (such as
  `Song: Live.mp3`) are skipped and counted.
- The size budget (doc 7) does not count `music/`.
- Playlist names cannot be Windows device names (CON, NUL, COM1, ...). Saves keep a backup in
  `user/backups/`.

## Tested and not tested

Tested in the workspace: the tag and cover readers (hand-built bytes for every format plus real files
made by ffmpeg), the index cache, search and browsing, playlists, the safe-path rules, conversion of a
real WMA file, and the interface in a browser (a generated tone plays, the clock advances, the
equalizer opens without stopping it).

**Not tested by a person yet:** playing real music in the Windows app (the address handler and the
equalizer's access to the sound), a very large library, ffmpeg on the real Windows build, Android.

## Open

- Go gets the player too (it costs nothing in size); say if you want it off there.
- Gapless playback, replay-gain loudness matching and lyrics are not built.
- A music-folder picker (to point at a folder other than `music/`) is not built.

## When the built-in player refuses a file (2026-10-20)
The web view decodes most WAV, M4A and AAC files itself, but not every variant (24-bit or 32-bit integer, floating point, 8-bit, mu-law and some extensible WAVs have been refused). First real test: a WAV would not play. Now, when the player gets an error on a file it was playing directly, it asks once for a converted copy (the same ffmpeg path WMA uses, cached in `user/cache/music/`) and plays that. If ffmpeg is not in `engines`, the message says so and suggests converting to 16-bit WAV, FLAC or MP3. `core/tests/media_ffmpeg.rs` converts six awkward WAV variants with a real ffmpeg. **TO TEST** on the user's own files.



## Cover pictures (2026-10-22)

Covers come from the file's own tag (MP3, FLAC, M4A, and now WAV files that carry an `id3 ` chunk), or from a picture in the song's folder: one named like the song (`Song.wav` and `Song.jpg`), or `cover`, `folder`, `front`, `albumart`, `albumartsmall`, `albumartlarge`, `album`, `art` or `thumb` (`.jpg`, `.jpeg`, `.png`), or the only picture in the folder. Two unnamed pictures are not guessed between. Not yet read from OGG or WMA tags: put a picture in the folder for those.
