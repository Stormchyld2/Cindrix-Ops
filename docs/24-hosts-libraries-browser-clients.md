# 24. Hosts, libraries and browser clients

**DECIDED (owner, 2026-10-21). Planned, not built yet.** Nothing here has been tried on real hardware.

## The idea in one paragraph

Go carries its program and its content on the same stick. Vault and Hub do not: they are a big drive
full of content, and the program runs on a *host* that the drive is plugged into. The host is a Windows
laptop (Vault) or an Android phone, the owner's old Samsung Note 10 (Hub). Other devices use a plain
browser tab to reach a host; they install nothing.

## Decisions

1. **One drive, two possible hosts.** The 4 TB exFAT drive works in either. Plugged into the laptop it
   is Vault; plugged into the phone it is Hub. The drive's marker (`cindrix-drive.json`, field `edition`)
   still sets the defaults. A drive is attached to one host at a time; running both at once needs two
   copies of the content.
2. **Vault is the core program only.** It is installed on the laptop and reads its content from the
   drive. It does not live on the drive. (Go is the exception: its stick carries both.)
3. **The drive also carries a rescue copy** of the portable Windows build and the Android app, next to
   the content, so any surviving machine can bring the library back if the laptop or phone is lost.
   This is a spare, not the Vault program.
4. **Engine and window.** The engine is the library server (archives, search, Ask, Shelf, music). The
   window is the interface. On the host the window is the app itself; on every other device it is a
   browser tab pointed at the host.
5. **Libraries, not "the drive".** The program lists every Cindrix drive it sees and asks before reading
   one (below).
6. **Browser clients only** for other devices, including iPhone. No separate client app.

## Libraries: finding drives and asking first

- A drive counts as a library only if it carries the marker file, `cindrix-drive.json`. The laptop's own
  disk and random USB sticks never trigger a question. Any other folder can still be added by hand in
  Settings so Ask can search loose files.
- The marker gets a new `id` field (random, written by `scripts/build-drive.ps1`), so a drive is
  remembered by its id and not by its drive letter, which Windows reassigns.
- When a marked drive appears the person is asked: **Always**, **This time only** or **Never**. No
  answer is pre-picked for the person, with one exception: for the main library drive, the 4 TB
  external drive (marker says `vault` or `hub`), "Always" is suggested.
- Settings gets a library list: each library on or off, forget a remembered answer.
- Several libraries can be on at once (for example Go stick and Vault drive). Search merges them and
  labels each result with its library.
- A drive is read, never run: nothing on it is started automatically. Nothing is scanned before the
  person says yes.
- Unplugging mid-use is normal: the library shows as offline and returns when the drive does, with no
  crash.
- Android cannot scan for drives. The person picks the drive's folder once with the system file picker
  and it is remembered (see doc 17). The library list looks the same.
- Settings split into two kinds: those that travel with the drive (library list, search index, Shelf
  index) and those tied to the device (interface size, which screen opens first).

## Browser clients

How other devices reach the host:

1. The person turns on sharing on the host (it is off at every start, doc 14).
2. The host shows an address and a QR code. Hub: the person turns on the phone's own hotspot first (an app
   cannot reliably do that, doc 17); the phone's address on its own hotspot is often the same each time,
   but the app shows whatever it really is. Vault: the laptop's address depends on the network.
3. Another device joins that Wi-Fi, scans the code or types the address, and gets the same screens.

Design points:

- **Plain HTTP, no certificate.** There is no way to get a normal certificate offline. The interface must
  not depend on browser features that need a secure connection (installing as an app, service workers).
- **Guests are read-only by default.** Changing settings, adding or removing archives, the library list
  and "Save a website" are host-only, behind a PIN.
- **All library content is shared to browsers** (owner, 2026-10-21): archives, Shelf, music, maps,
  Academy, cards and drills, and Ask. This replaces doc 17's "archives only". The person's own private
  pages, the family page, inventory and notes under `user/`, are not content and stay host-only unless the
  owner says otherwise. OPEN: confirm that reading.
- **Content lives in one of two places:** the stripped-down set on Go, or the full set on the 4 TB drive
  that a host shares.
- **Per-device state** (bookmarks, open tabs, text size) is kept in each browser. The host holds only
  shared things.
- **Ask on a shared host** is limited to one or two questions at a time, with a queue, so a phone is not
  swamped.
- **Streaming** of music and large archive pages needs proper range requests.
- **Narrow screens.** Most guests will be on phones.

## What exists today (checked in the code, 2026-10-21)

- The interface reaches the backend through one file, `src/lib/api.ts`. That is the seam this plan uses.
- The backend is about 75 Tauri commands in `src-tauri/src/lib.rs` (library, Ask, Shelf, inventory,
  Academy, music, maps, settings, Ghostnet and others), plus three custom address schemes the interface
  uses directly: `cindrix-tiles` (maps), `cindrix-music` and `cindrix-zim` (`zimview.rs`).
- `kiwix-serve` is already an HTTP server. `library_share` can already offer the archives on the local
  network; it is off at every start.
- Drive finding exists (`core/src/drive.rs`: `find_all`, `check`), but `choose` picks **one** drive and the
  app keeps a single root (`library_dir()`). Several libraries means changing that to a list.
- Much of the command logic sits in `lib.rs` next to Tauri state. For a browser to use it, the shared logic
  must live in `core/` (which is also where the CLAUDE.md rules say tested logic belongs).
- Doc 17 already lists the Hub's web layer as "not built yet" and proposes a small Rust HTTP server over
  the core crate. This doc keeps that plan and widens it to Vault.

## Plan, in order

Windows first (doc 5, 2026-10-20): steps 1 and 2 belong to the Windows program and Vault needs them;
steps 3 onward wait for the real Windows test (doc 22).

1. **Libraries.** Add `id` to the marker; replace the single drive root with a list of libraries; the
   ask-first prompt; Settings library list; merged, labeled search. Tests in `core/`.
2. **One transport seam.** Move command logic that is still in `lib.rs` into `core/`, and put the Tauri
   calls behind an interface in `api.ts` so a second transport can be added without touching screens.
3. **Read-only HTTP server** over `core/`, and a browser build of the same Svelte interface using the HTTP
   transport. Special cases: Ask streaming (a Tauri channel today) becomes server-sent events; the three
   custom schemes become ordinary HTTP paths; file upload (`shelf_add`) stays host-only.
4. **Sharing screen:** address, QR code, PIN, who is connected.
5. **Android wrap and the Note 10 tests** below.
6. **The assistant on the phone.** The owner expects the Note 10 to run it, with the model file on the
   drive. That keeps the phone's storage free, but not its memory: a model has to be loaded into RAM to
   run, so the Note 10's RAM (8 GB; the Note 10+ has 12 GB), shared with Android, the hotspot and the
   server, is the limit, and the drive's read speed decides how long loading takes. The `llama-server`
   engine also has to be built for Android. Test this early (below). The Hub assistant stays on by default
   (`product.rs`); if a model proves too heavy, a smaller one or a setting to turn it off is the fallback.
   Later, if wanted: Hub using a Vault laptop's assistant over the network.

## To test on real hardware (none done)

- Reading a large ZIM from an exFAT drive on Android through the system picker.
- Powering a 4 TB drive plus a hotspot from the phone; a powered USB hub is expected to be needed.
- Whether the Note 10 can keep a hotspot up and serve several readers at once, and for how long.
- Whether the phone can be on Wi-Fi and run a hotspot at the same time (many phones cannot).
- Performance with several readers, spinning disk against SSD.
- The assistant on the Note 10: which model size loads, how long loading takes from the drive, how fast
  answers come, and what happens to the hotspot and server while it runs. Several people asking at once
  go through the queue.
