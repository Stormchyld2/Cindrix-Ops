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

## What plays

MP4 / M4V (H.264 video with AAC audio plays everywhere), WebM and OGV. MKV, AVI, MOV, WMV and similar are listed, marked "save it as an MP4 to watch it here", and are never sent to the player.

## What it does

- Search across every topic (all words must match the title, group or topic name).
- Plays in the page with skipping (the server answers range requests in pieces of up to 32 MB), remembers where each video was left off (`user/video-progress.txt`, host-only, same file format as podcasts) and lets you mark videos watched.
- Guests and trusted phones on the hotspot may list and watch (`videos_list` is a guest command; the files come from the `/vid/` address). They do not get saved places.

## Code

`core/src/videos.rs` (scan, search, safe `resolve`, tests), `src-tauri/src/videos.rs` (the `cindrix-vid` address and commands), share-server route `/vid/` in `src-tauri/src/share.rs`, `src/views/Videos.svelte`, `api.ts` (`videosList`, `videoSource`, progress). `core/tests/security_rules.rs` guards the guest rules.

## Not yet

Converting MKV/AVI to MP4 inside the program, a "continue watching" tile on the Dashboard, linking an Academy lesson's `[video]` line to a file here, and a mini player that keeps playing while you move between screens.
