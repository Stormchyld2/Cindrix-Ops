# 17. Three products, one code base

> **Update 2026-10-21:** Vault is now the core program only, reading content from the drive; Vault and Hub are two hosts for the same drive, reached from other devices by a browser tab. See [24](24-hosts-libraries-browser-clients.md). The product table below describes the earlier plan where it differs.

**DECIDED (owner, 2026-10-18).** Three products: Cindrix Ops Go, Cindrix Ops Vault, Cindrix Ops Hub.
All three are **one program**. The drive's marker file (`cindrix-drive.json`, field `edition`:
`go`, `vault` or `hub`) says which one it is, and `core/src/product.rs` turns that word into a name,
a size budget and a list of features. Spelling: C-I-N-D-R-I-X.

| Product | What it is | Runs on | Content size |
|---|---|---|---|
| **Cindrix Ops Go** | The full app on a 128 GB USB stick: assistant, library, shelf, cards, drills, binder, maps, music | Windows PC (Android later) | up to 100 GB |
| **Cindrix Ops Vault** | The same app on a 4 TB external drive, with the big archives and saved websites | Windows PC | up to 3400 GB |
| **Cindrix Ops Hub** (the "internet in a box") | The same program run as a server: it holds the archives on a large drive and shares them over a Wi-Fi hotspot to phones and laptops. It also has a viewer, so ZIM and other files can be opened on the server device itself. | Android tablet first; a Raspberry Pi or mini PC later, if wanted | whatever the drive holds |

## One program, three switches

Go and Vault differ only in drive contents (the `editions` and `budget_gb` fields in the manifest).
The Hub is the same code with a different set of features turned on:

| Feature | Go | Vault | Hub |
|---|---|---|---|
| Assistant (local model) | on | on | **off by default** (the model is the heaviest and least proven part on Android; OPEN: confirm) |
| Library: shelf, archives, cards | on | on | on |
| Maps | on | on | on |
| Music player (the owner's own music) | on | on | on |
| Plan screens (inventory, drills, family, binder) | on | on | on |
| Save a website | on | on | on |
| Offer to share the archives on the network | normal | normal | prominent |
| Content budget | 100 GB | 3400 GB | none |

Sharing is never switched on by itself, in any product: it stays off until the person turns it on and
confirms (doc 14). Only the archives are served, not the shelf, inventory, family page or assistant.

Built so far (base program): `core/src/product.rs` (products, features, budgets, `check_budget`),
`DriveInfo::product()`, a `product_info` command in the Tauri shell, the interface showing the product
name and hiding screens a product turns off, and `scripts/build-drive.ps1 -Edition go|vault|hub` with a
size check on the `library` folder (models and maps are not counted yet). Maps now have a tile viewer (see doc 18); the drive checker also checks map packs. Also built for Go: a drive checker (`core/src/drivecheck.rs`, run with `cargo run --example check_drive -- <folder>`;
see doc 7). Not built yet: the Hub's
web layer for other devices' browsers, and the Android build.

## Hub on an Android tablet (DECIDED target; nothing tried on hardware)

The owner's plan: a 4 TB drive full of files on an Android tablet, other devices join the tablet's
Wi-Fi hotspot and open the files in a browser, and the tablet has its own viewer.

- **Hotspot:** the person turns on Android's built-in hotspot in system settings. An app cannot
  reliably start a hotspot with a name and password of its own choosing, so the app shows the address
  to type once the hotspot is on. Other devices then open that address in a browser.
- **Server:** the Rust core is compiled into the Android app and runs the archive server as a
  foreground service so it keeps running with the screen off. **TO TEST** (Tauri on Android is newer
  than on Windows; see doc 6).
- **Viewer on the tablet:** the normal app screens: Archives (the Kiwix reader), the Shelf and its PDF/Markdown
  reader. Nothing extra to build for the viewer.
- **The 4 TB drive:** connects over USB-C. Android limits what folders an app may read on external
  storage, so the person picks the drive's folder once with the system file picker (same approach as
  doc 2, "Android"). Format exFAT: FAT32 cannot hold files over 4 GB, and NTFS support on Android
  varies. A drive this size likely needs a powered USB hub; a bare tablet port may not supply enough.
  A spinning disk draws more power and streams less well to several readers at once than an SSD.
- **Open questions for the owner:** which tablet and Android version; spinning disk or SSD.
- This replaces the Fire tablet plan in doc 16 (which could only run the Kiwix app) for anything
  beyond the owner's first test.

## Options considered for the Hub

1. **Internet-in-a-Box (IIAB).** Free, open source, Wi-Fi hotspot, Kiwix, maps and more on a
   Raspberry Pi. Rejected as the base: the owner wants our own safety wording, cards and drills, on a
   tablet.
2. **Our own Hub on the shared core. CHOSEN.** The read-only screens (library, shelf, emergency cards,
   drills, binder) are served as web pages to phone browsers, with `kiwix-serve` behind them for the
   archives.
3. **Kiwix only** (the old Fire tablet plan). No work, no extras. Still fine as a stopgap for a first test.

## Order of work

1. Base program: product setup shared by all three (done in code, see above), then Maps, real books,
   and the build-drive script with license checks.
2. Go: a tested 128 GB drive image.
3. Vault: the same, with the big archives.
4. Hub: the web layer for other devices, then the Android build.

## Open-source starting points for the Hub

Details come from project pages; check each license before copying code. Nothing has been tried.

| Piece | Candidate | Notes |
|---|---|---|
| Serve archives | `kiwix-serve` (GPL-3.0) | Already used by the app. |
| Search across many archives | Zimi (MIT, Python) | Web UI, JSON API, can make ZIMs, LAN sharing; Python dependency. Good to study; maybe not to ship. |
| Own Wi-Fi network (Pi or mini PC only) | `hostapd` + `dnsmasq`, or NetworkManager hotspot mode | Standard Linux parts. They do not apply on Android. RaspAP is a ready-made admin page for them (license not confirmed). |
| Build a Pi image (later) | Kiwix Hotspot / "offspot" imager (GPL-3.0, github.com/offspot/imager-service) | Images up to 1 TB; Pi 3B+/4/5/400. Reference for packing content onto an image. |
| Whole-box reference | Internet-in-a-Box (IIAB), community project | Admin console, maps, many apps. License not confirmed from its wiki. Study its menu and admin design. |

Proposed shape of the web layer: a small Rust HTTP server on `cindrix-core` (cards, drills, shelf,
search, ZIM writer, website saver) that serves our read-only screens to phone browsers, with
`kiwix-serve` behind it. It could also run the website saver on a schedule when it has internet.
First step when it is time: a web build of the existing interface plus a read-only HTTP layer over the
core crate.
