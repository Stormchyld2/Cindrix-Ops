# Cindrix Ops: architecture docs

Status: first draft, before any code. Items marked **TO TEST** have not been tried on real hardware. Items marked **OPEN** are waiting on a decision.

1. [Architecture](01-architecture.md): the parts and how they connect
2. [Files and folders](02-files-and-folders.md): the drive layout and where user data lives
3. [Safety and licenses](03-safety-and-licenses.md): the rules that shape everything else
4. [Build and release](04-build-and-release.md): GitHub, test plan, updates
5. [Open decisions](05-open-decisions.md)
6. [Stack: Rust, Tauri, Svelte, TypeScript](06-stack.md)
7. [Content manifest](07-content-manifest.md)
8. [Safety tests](08-safety-tests.md)
9. [Optional passphrase lock](09-passphrase-lock.md)
10. [First slice](10-first-slice.md)
11. [Navigation](11-navigation.md)
12. [User guide outline](12-user-guide-outline.md)
13. [Run the first slice](13-run-the-first-slice.md)
14. [Content plan: real books and our own writing](14-content-plan.md)
15. [Stockpiling websites as .zim files](15-stockpiling-websites.md)
16. [Vault server on an old Fire tablet](16-vault-on-a-fire-tablet.md)
17. [Three products, one code base](17-three-products.md)
18. [Maps](18-maps.md): saved maps, the tile viewer, getting map data
19. [Music](19-music.md): the music player, formats, the converter, playlists
20. [Archive reader](20-archive-reader.md): reading ZIM archives inside the app

Mockups of every screen are in [`../mockups`](../mockups).
- [21. Academy](21-academy.md)
- [22. Real test](22-real-test.md)
- [23. Intel, search and settings](23-intel-search-settings.md): the offline internet tab, what the assistant searches, every setting
- [26. Plans that carry on](26-plans.md): repeating checks, learning plans, emergency plans, several at once
- [25. Testing Ask](25-ask-test.md): a 30-question check of the small models
- [24. Hosts, libraries and browser clients](24-hosts-libraries-browser-clients.md): Vault and Hub as hosts for one drive, asking before reading a drive, browser tabs as clients
