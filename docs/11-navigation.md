# 11. Navigation

Nine groups (Ask, Library, Intel, Academy, Maps, Music, Plan, Inventory, Settings). The mockups show an earlier six-group menu; the program is the reference.

| Group | Screens inside |
|---|---|
| Ask | Chat |
| Library | Shelf (books), Archives (add, remove, save a website, share), emergency cards |
| Intel | The offline internet: archives opened like web pages, with tabs, bookmarks and a search across every archive |
| Maps | Map, drawing, saving, exporting |
| Music | Library (search, artists, albums, folders), Queue, Playlists, Equalizer. A small player bar stays at the bottom of every screen while a song is loaded. |
| Plan | Start (the walkthrough), What next (plans that carry on), Drills, Family, Binder, What fails next |
| Inventory | Your supplies with use-by dates (its own link in the top bar, between Plan and Settings) |
| Settings | General, Assistant, Library, Intel, Music, Maps, Plan, Drive, Sources and licenses, About |

On a phone the groups become a bottom bar. On a computer they sit across the top, with the screens of the current group underneath.

The footer keeps the honest status line: model, speed, memory, "this app only talks to itself".


## Opening screen and hints (2026-10-22)

- The program opens on the **Dashboard** (Settings, General, "Open on this screen" can change it). People who still had the old default are switched once.
- Every button has hover text (`src/lib/tooltips.ts`): hand-written sentences for the known buttons, and a plain fallback ("Press to: ...") for any new one, so none is left bare. Settings, General can turn the hints off. A button with its own `title` keeps it.
- The Dashboard keeps its tiles, suggestions and running-low list, and adds the plans being followed (and how many checks are due), Academy progress, supplies by kind, and a ready check for the drive.
