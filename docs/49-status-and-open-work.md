# 49. Status and open work (audit of 2026-11-07)

A read-through of the repo, docs and CI config. Nothing was built or run for this audit, so "built" below means
"written and passing the automated checks", never "seen working by a person".

## Built (automated checks only)
Dashboard, Ask (chat history, source links, projects, calendar, to-do), Library and archive reader, Intel, Academy,
Plan and Seed vault, Inventory (shelf life, places), Maps (street, topo, aerial, drawing, directions, MGRS grid,
phone map export, Send to a device), Music, Morale (Reading Corner, Journal, Photos), Family, drug labels,
Safe backup mailbox, share server with PIN and invites, drive markers and library questions, charcoal-and-blue theme.

## Open, in priority order
1. **Real Windows test** (doc 22). Never run by a person: the model, kiwix search on real archives, map tiles,
   music, Save a website, libraries on real drives, share server and invites. Several CI jobs are
   `continue-on-error`, so a green build does not prove these.
2. **Android Hub** not started. Needs a Tauri-on-Android test, the llama.cpp assistant on the Note 10, and a
   replacement for `kiwix-serve` (does not run on Android). Waits for item 1.
3. **Bible study (Canonleaf's files)**, doc 44: built and unit-tested, never run on the owner's real Canonleaf library, hotspot or hardware, and its Tauri and Svelte parts need a first CI check.
4. **Academy**: the four security-school-style courses were written on 2026-11-07 (protective driving,
   noticing surveillance, home and residential security, safe and accurate shooting; 6 lessons each, all marked
   draft, and they pass the Academy load, quiz and no-dose tests). Still thin: the seven original courses have
   4 lessons each (navigation and shelter/fire/water now have 7 and 6 after the 2026-11-07 additions), and a
   Khan-style course archive is only an idea on the download list.
5. **Content review**: about 534 content files carry draft or "pending review" markers. The owner plans a human
   check of the medical cards. 13 navigation, water and group-skills cards and 10 drills were added on 2026-11-07;
   more are still wanted (Ranger Handbook-style skills beyond these, and the other original courses are thin).
6. **Map data**: street addresses for the whole US not done (doc 28); Cheatham and Robertson counties are not in
   the Tennessee aerial/topo box; Maps "Still to do" in doc 18 (place-name fonts, no-go-area route warnings, GPS).
7. **License of our own code**: owner leans to a private repo and a custom personal-use license (doc 05, row 14). A draft is in `licenses/PERSONAL-USE-LICENSE-DRAFT.md`; it needs a lawyer, and the repo must be switched to private by the owner. There is deliberately still no LICENSE file at the root.
8. **Installer and signing**: builds are unsigned (SmartScreen warns); signing postponed by the owner.
9. **Standalone map viewer (`.cmap`)** is a separate project; spec only (`CINDRIXOPS_CMAP_VIEWER_ARCHITECTURE.md`).
10. **Placeholder screen** `Soon.svelte` still shows for planned tabs.

Update this page, not the older "still to do" lists, when items change.
