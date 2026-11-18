# Videos (how-to videos on the drive)

**Built 2026-11-17, not yet run on real hardware.** Menu: Morale > Videos (follows the same on/off switch as Music and Podcasts).

## Where the files go

`library/videos/` on any drive that may be read (the main drive's first; several drives merge into one list):

```
library/videos/
  Butchering and Processing/
  Bushcraft/
  Foraging/
  Trapping and Fishing/
  Shelter and Fire/
  Water/
  Medical/
  Gardening and Food/
  Repairs and Skills/
  Other/
```

- Each folder is a **topic**; the MP4 file name is the **title** (underscores read as spaces). Add any topic folder you like.
- One level of **group folders** inside a topic is allowed (`Bushcraft/Fire/Friction fire.mp4`); deeper folders are not read.
- A picture with the same name as a video (`Friction fire.jpg`, also png or webp) is its thumbnail.
- An optional `topic.txt` in a topic folder: its first line is the name shown on screen.
- Videos dropped straight into `videos/` are listed under "Other".
- The same topic folder on two drives is one topic. If two drives hold the same path, the first drive's copy is used.

## What plays, and the other formats

MP4 / M4V (H.264 video with AAC audio plays everywhere), WebM and OGV play as they are.

MKV, AVI, MOV, WMV, FLV, MPG, TS, M2TS, VOB, DivX, ASF, RM and 3GP are listed with a **Make playable** button (host only). It uses `ffmpeg.exe` from the drive's `engines/` folder, the same helper the music player uses for WMA. The original is never changed; the copy goes into a hidden `.converted` folder inside the `videos` folder the original is in, so it travels with that drive and any host (also the phone) finds it again. "Delete the converted copies" on the page removes them.

- Quick way (seconds, no quality loss): if the picture is already H.264 (MKV and MOV usually are) it is copied as it is into an MP4; VP8, VP9 and AV1 are copied into WebM. Only a sound the player cannot use (AC3, DTS and so on) is re-made, into AAC or Opus.
- Slow way (minutes, depends on the video): other pictures (old DivX, WMV, MPEG-2) are re-made with the best encoder that ffmpeg has: libx264, then Windows' own (`h264_mf`), then a graphics card's, then VP9. An ffmpeg with none of these says so in plain words.
- One conversion at a time, stoppable, with a time limit of three hours. The page shows how long it has run and carries on if you leave the screen.
- Guests and phones cannot start a conversion; they play the copies that already exist. A phone host cannot run ffmpeg, so convert on the Windows computer and the copies are on the drive.
- Conversion needs room on the drive: a converted MKV that is copied is about the size of the original; a re-made video is often smaller.

## What it does

- Search across every topic (all words must match the title, group or topic name).
- Plays in the page with skipping (the server answers range requests in pieces of up to 32 MB), remembers where each video was left off (`user/video-progress.txt`, host-only, same file format as podcasts) and lets you mark videos watched.
- Guests and trusted phones on the hotspot may list and watch (`videos_list` is a guest command; the files come from the `/vid/` address). They do not get saved places.

## The floating player and lessons (2026-11-18)

There is one player for the whole program (`src/lib/videoplayer.svelte.ts`, window `src/lib/VideoPlayer.svelte`). Playing a video from the Videos screen or from an Academy lesson opens it in a window at the bottom right, which keeps playing when you move to another screen ("Smaller" / "Bigger" / "Close"). Starting a video pauses the music and podcast players. Saved places work as before. An Academy lesson names a video with `[video] caption | Topic/File.mp4` (doc 21).

Converting shows a percent bar: ffmpeg's `-progress` output against the length read from the file (`videoconv::Progress`); until the length is known the bar is a plain "getting ready".

## Code

`core/src/videos.rs` (scan, search, safe `resolve`, tests), `core/src/videoconv.rs` (what to copy and what to re-make, ffmpeg arguments, converted copies; `core/tests/video_ffmpeg.rs` runs it with a real ffmpeg), `src-tauri/src/videos.rs` (the `cindrix-vid` address and commands), share-server route `/vid/` in `src-tauri/src/share.rs`, `src/views/Videos.svelte`, `api.ts` (`videosList`, `videoSource`, progress). `core/tests/security_rules.rs` guards the guest rules.

## Not yet

Hardware-accelerated converting and a "continue watching" tile on the Dashboard.
