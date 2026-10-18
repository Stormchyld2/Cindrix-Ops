# 1. Architecture

## Goals
- Works with no internet and no installation. Windows runs from the USB stick; Android installs from an APK.
- One interface for both platforms: a dark web UI served from the device itself.
- Everything the user creates is plain files on the drive.
- Open-source and free content only.

## Parts

| Part | Job | Software | License |
|---|---|---|---|
| App shell and core | Starts everything; owns user data, search, safety check, maps, exports, encryption | Tauri 2 with a Rust core (see doc 6) | Ours (GPL-compatible) |
| Interface | Chat, Library, Maps, Inventory, Drills, Family, Binder, Settings, and the rest | Svelte + TypeScript, system fonts only | Ours |
| AI model server | Runs the model, streams answers | llama.cpp | MIT |
| Model | Answers from the library; thinking mode off | Qwen3.5 GGUF (2B default on 8 GB, 4B on larger, 9B optional) | Apache 2.0 |
| Library reader | Serves offline archives and searches them | Kiwix (libzim, kiwix-serve) | GPL |
| Map data | Street and topo maps for the whole US | OpenStreetMap vector tiles (PMTiles), elevation contours and shading | ODbL, public domain elevation |
| Map viewer | Pan, zoom, draw, route | A JavaScript map library (**OPEN**, see decisions) | Open source |
| Aerial photos | Optional satellite view by county | USGS NAIP, downloaded by the user in Settings | Public domain |

## How a question gets answered
1. The user types a question in Chat.
2. The Rust core runs the **safety check** (see doc 3). Dosing questions get a fixed reply, not a model answer.
3. The core searches the library (archives, manuals, emergency cards) and picks the best passages.
4. The core sends the question plus those passages to the model.
5. The reply streams back with the sources it used. If nothing relevant was found, the reply says so instead of guessing.

## Network behavior
- The interface talks to the Rust core directly, so no network port is opened for the app itself. The interface shows this honestly: "This app only talks to itself on this device." It does not claim to switch off Wi-Fi or radios.
- The only features that use the internet are ones the user starts: downloading a satellite county or an update pack.

## Platforms
- **Windows:** portable folder, launcher starts the servers and opens the default browser or an embedded window. Unsigned at first, so SmartScreen will warn (**TO TEST**). Code signing later.
- **Android:** an APK containing the interface, the app server logic, and llama.cpp. Big content (library, maps, models) is copied from the drive into a folder the user picks. Sideloading shows a Play Protect notice (**TO TEST** on a Galaxy Note 20, 8 GB).
- **Later:** a bootable launcher for non-Windows PCs, then Mac and iPhone.

## Hardware targets
- Minimum: an 8 GB RAM Windows laptop with an Intel i7 (reported as an i7-8700). Expect the 2B model.
- Reference desktop: Windows, i7-8700, 16 GB RAM.
- Phone: Galaxy Note 20, 8 GB RAM.
- Speeds in the mockup are made up. Real speeds are **TO TEST**.
