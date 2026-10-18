# 17. Three products, one code base

Plan, not built. Names chosen by the owner: Cindrix Ops Go, Cindrix Ops Vault, Cindrix Ops Hub Spelling confirmed: C-I-N-D-R-I-X.

| Product | What it is | Runs on | Content size |
|---|---|---|---|
| **Cindrix Ops Go** | The full app on a 128 GB USB stick: assistant, library, shelf, cards, drills, binder, maps | Windows PC (Android later) | up to 100 GB |
| **Cindrix Ops Vault** | The same app on a 4 TB external drive, with the big archives and saved websites | Windows PC | up to 3400 GB |
| **Cindrix Ops Hub** (the "internet in a box") | A server only: it holds the archives and shares them over its own Wi-Fi to phones and laptops. No assistant. | A small always-on device (Raspberry Pi, mini PC, old tablet) | whatever the drive holds |

## Not three programs

Go and Vault are the same program with different drive contents (the `editions` and `budget_gb`
fields in the manifest already do this). The Hub needs a different shape, but not new content or
new core logic: it reuses the same archives, the same build script and the same Kiwix server.

## Options for the Hub

1. **Use Internet-in-a-Box (IIAB).** Free, open source, already does a Wi-Fi hotspot, Kiwix, maps
   and more on a Raspberry Pi. We would supply a recipe and our content, not new software.
   Least work, most proven. Needs checking against our license rules for anything it bundles.
2. **Our own thin Hub.** A small Rust server built from the existing core, serving the read-only
   screens (Library, Shelf, Cards, Drills, Binder) as web pages, plus Kiwix, to any phone browser.
   Phones would get our look and our safety wording, and could use saved drills and cards.
   More work; the interface would need a web build alongside the desktop one.
3. **Kiwix only** (the old Fire tablet plan). Just the Kiwix Android app. No work, no extras.

Recommendation: start with option 3 for the owner's own Fire tablet now, plan option 1 for anything
sold, and build option 2 only if phones need our cards and drills without the PC.

## Order of work

1. Finish the base app (Maps, real books, build-drive script with license checks).
2. Go: a tested 128 GB drive image.
3. Vault: the same, with the big archives.
4. Hub, once the first two are in people's hands.

## Hub: building our own (decision: owner wants our own)

Open-source starting points found (details from project pages; check each license before copying
code, nothing has been tried):

| Piece | Candidate | Notes |
|---|---|---|
| Serve archives | `kiwix-serve` (GPL-3.0) | Already used by the app. |
| Search across many archives | Zimi (MIT, Python) | Web UI, JSON API, can make ZIMs, LAN sharing; Python dependency. Good to study; maybe not to ship. |
| Own Wi-Fi network | `hostapd` + `dnsmasq`, or NetworkManager hotspot mode | Standard Linux parts. RaspAP is a ready-made admin page for them (license not confirmed). |
| Build a Pi image | Kiwix Hotspot / "offspot" imager (GPL-3.0, github.com/offspot/imager-service) | Images up to 1 TB; Pi 3B+/4/5/400. Reference for how to pack content onto an image. |
| Whole-box reference | Internet-in-a-Box (IIAB), community project | Admin console, maps, many apps. License not confirmed from its wiki. Study its menu and admin design. |

Proposed shape: a small Rust server built on `cindrix-core` (cards, drills, shelf, search, ZIM
writer, website saver) that serves our read-only screens to phone browsers, with `kiwix-serve`
behind it for the archives, on a Pi or mini PC that makes its own Wi-Fi. The Hub could also run the
website saver on a schedule when it has internet. First step when it is time: a web build of the
existing interface plus a read-only HTTP layer over the core crate.
