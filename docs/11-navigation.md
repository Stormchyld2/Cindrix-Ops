# 11. Navigation

Ten groups (Ask, Library, Intel, Academy, Maps, Music, Plan, Inventory, Security Operations, Settings). The mockups show an earlier six-group menu; the program is the reference.

| Group | Screens inside |
|---|---|
| Ask | Chat |
| Library | Shelf (books), Archives (add, remove, save a website, share), emergency cards |
| Intel | The offline internet: archives opened like web pages, with tabs, bookmarks and a search across every archive |
| Maps | Map, drawing, saving, exporting |
| Music | Library (search, artists, albums, folders), Queue, Playlists, Equalizer. A small player bar stays at the bottom of every screen while a song is loaded. |
| Plan | Start (the walkthrough), What next (plans that carry on), Drills, Family, Binder, What fails next |
| Inventory | Your supplies with use-by dates (its own link in the top bar, between Plan and Settings) |
| Security Operations | Command (security dashboard), Security, Operations, Planning, Personnel, Records; see [Security Operations Architecture](44-security-operations-architecture.md) |
| Settings | General, Assistant, Library, Intel, Music, Maps, Plan, Drive, Sources and licenses, About |

On a phone the groups become a bottom bar. On a computer they sit across the top, with the screens of the current group underneath.

The footer keeps the honest status line: model, speed, memory, "this app only talks to itself".


## Opening screen and hints (2026-10-22)

- The program opens on the **Dashboard** (Settings, General, "Open on this screen" can change it). People who still had the old default are switched once.
- Every button has hover text (`src/lib/tooltips.ts`): hand-written sentences for the known buttons, and a plain fallback ("Press to: ...") for any new one, so none is left bare. Settings, General can turn the hints off. A button with its own `title` keeps it.
- The Dashboard keeps its tiles, suggestions and running-low list, and adds the plans being followed (and how many checks are due), Academy progress, supplies by kind, and a ready check for the drive.


## Security Operations navigation and dashboards (approved 2026-11-07)

In the current top bar (doc 35) Security Operations is one word that opens its own screen, with the six subtabs below along the top of that screen.

- Add exactly **one** new top-level parent tab labeled **Security Operations**. On entry, open **Command** as its default subtab and security-specific dashboard. Do not add Command as a separate top-level section.
- Show six subordinate tabs in order: **Command**, **Security**, **Operations**, **Planning**, **Personnel**, **Records**. On Android/Hub, fit these within the established mobile navigation using a horizontally scrollable/overflow subtab row, not six additional bottom-navigation parents.
- **Command** shows site posture, verification/freshness, current watch and coverage gaps, patrol/check-in status, incidents, overdue tasks, inspections and quick-access emergency procedures.
- **Security** covers perimeters, site profiles, inspections and physical protective measures. **Operations** covers duty rosters, patrols, handovers and response assignments. **Planning** owns Defensive Plans and **Operational Response**. **Personnel** surfaces role assignments, training/readiness and gear references. **Records** houses SOPs, logs, incidents, AARs and authorized exports.
- Keep the existing main **Dashboard**, adding a small, optionally hideable Security summary: posture/freshness; watch/coverage; open incidents and pending alerts; due/overdue security tasks. Each summary links to Security Operations. Command and the main Dashboard must consume the **same read-only security projection**, not separate copies of status records.
- Security posture values: **Normal**, **Elevated**, **Emergency**, **Unknown**. New/unverified sites default to Unknown. Never infer Normal from silence or an absent feed; show separate stale/unavailable indicators when appropriate.
- Support independently configured **security site profiles** (e.g., home, campsite, evacuation destination), each with its own status, maps, watch roster, procedures and incidents; shared people/equipment retain their authoritative Family/Inventory IDs.
- Offer a **global Emergency Procedures shortcut** as well as entry from Command. It must open locally available approved checklists without internet, AI or optional app integrations, and must respect locked-data permissions.
- No existing top-level tab or implemented route is removed by this proposal. This is approved **navigation design**, not a claim that the UI has already been implemented. The detailed specification and acceptance tests are in [doc 44](44-security-operations-architecture.md).
