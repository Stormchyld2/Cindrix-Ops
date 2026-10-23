# 34. The share server

**BUILT (2026-10-23), not yet run on real hardware.** Decided by the owner on 2026-10-22: other devices get the **full app screens** in a browser, behind a **PIN**, and sharing **switches on by itself when the hotspot starts**. Plan and background: `24-hosts-libraries-browser-clients.md`.

## What it does
- The laptop (Vault) or phone (Hub) runs a small web server on port 8765 (any free port if that one is taken). It hands the browser the same screens the app shows and answers the same questions the screens ask.
- **When it switches on:** the program looks every 5 seconds for a hotspot (Windows Mobile Hotspot, 192.168.137.x; Android 192.168.43.x or an access-point adapter). A hotspot appearing switches sharing on; it switches off when the hotspot does. Settings, Sharing, has a switch to turn that off, and a button to turn sharing on or off by hand. Sharing started by hand stays on until switched off. If the person switches off a hotspot-started share, it stays off until the hotspot has been off once.
- **Not on at start-up** unless a hotspot is already on.
- **PIN:** 4 to 8 digits, kept in `user/share-pin.txt` (made at random the first time). A guest types it once on a plain page and gets a cookie. Five wrong tries lock that device out for a minute. Changing the PIN signs everyone out.
- **The page for guests:** Settings, Sharing, shows the address(es), a QR code (made by `src/lib/qr.ts`, no library), the PIN and how many devices are signed in.

## What a guest can do
- Read: archives and the reader (`/content/…` pages), the Shelf (books are downloaded to the browser and read there), emergency cards, Academy, plans, drills, maps (`/tiles/…`, same range requests as the app), music (`/music/…`), and Ask (answers stream in).
- Only the actions in `GUEST_COMMANDS` (`core/src/share.rs`) work. Everything else answers "This can only be done on the host…". That keeps the family page, inventory, My details, notes, chats, settings changes, adding or removing things and saving websites on the host.
- Ask for guests never reads the `user` folders (`filesearch::Limits.hide_user`), and only two questions run at once; others are told to wait.
- Each browser keeps its own bookmarks and open tabs. Reading position and saved maps are host-only for now.

## How it is built
- `core/src/hotspot.rs`: reads `ipconfig` / `ip` output to find the hotspot and the addresses. `core/src/share.rs`: reading requests, replies, ranges, PIN and sign-in, the guest list, the question gate. All tested in `core/`.
- `src-tauri/src/share.rs`: the listener, sign-in page, the action table (`dispatch`), server-sent events for Ask, and the three readers (`serve_tile`, `serve_music`, `zimview::serve`) reused as they are. The interface files come from the app's own bundle.
- `src/lib/api.ts`: a page served by the share server contains `<meta name="cindrix-share">`; then `call()` asks the host over the network instead of Tauri, and the picture and music addresses point at the host. This is still the only place the interface reaches the backend.
- Plain HTTP (no certificate possible offline). Nothing here needs a secure connection.

## Not done yet
- Guest-friendly versions of screens that need host-only data (the Dashboard shows errors for those parts).
- Reading position, bookmarks and chats kept in the guest's browser.
- Uploading to the Shelf, music playlists and the like from a guest (host-only on purpose).
- Android: the server is the same code; the Android wrap and the hotspot detection on a real phone are untested.

## To test (Windows first)
1. Start the program, turn on Windows Mobile Hotspot. Within about 5 seconds, Settings, Sharing, says "Sharing is on, switched on by your hotspot" with an address starting 192.168.137.1.
2. Windows may ask to allow the program on the network: allow private networks.
3. On a phone joined to that hotspot, scan the code. Type the PIN. The screens appear.
4. As a guest: open an archive page, a book, a card, a lesson; ask a question; open Maps; play a song. Try a wrong PIN five times (it should wait a minute).
5. As a guest, try something that changes things (Family page, Inventory): it should refuse in plain words.
6. Turn the hotspot off: sharing goes off and the phone loses the page.
