# 23. Intel, what the assistant searches, and settings

## Intel: the offline internet

Intel is its own tab in the top menu (it goes with the library, so Go, Vault and Hub all have it).
It is a small browser for the ZIM archives on the drive. Nothing in it touches the network.

- **Start page.** One card per archive (title, the archive's own description, number of pages, size),
  plus your bookmarks. A new tab opens here, or in the first archive (Settings, Intel).
- **Search every archive.** The box at the top searches the titles in all archives at once; matches are
  shown grouped by archive and alternate between archives, so a huge encyclopedia does not push a small
  manual off the list. While an archive is open, "Search inside" searches the words in its pages (full
  text when the library server, `kiwix-serve`, is running; titles otherwise).
- **Browsing.** Back, forward, home, text size (A−, A＋), bookmarks across all archives. Links to the
  internet are not opened; the page says so.
- **Tabs.** Up to 12. Settings can remember them for the next start.
- **New archives appear by themselves.** The list refreshes every few seconds, and the library server is
  restarted once a copied file has stopped growing (`SettleWatch` in `core/src/library.rs`), so a
  half-copied archive is not served. Settings, Library can turn the automatic restart off.
- **Plain Kiwix screen.** Still available (link under the browser) while `kiwix-serve` is running.

Archives (Library) is now the place to add, remove, save a website and share. The reader moved out of it.

Not built: find-in-page, printing, and a reader that works without `kiwix-serve` on Android (see doc 20).

## What the assistant searches

Ask answers only from material on the drive. For each question it looks in:

1. the emergency cards and sample content (always);
2. the Shelf's books, once their text has been read (the Shelf reads a book's text when it is opened);
3. the archives: by title always, by full text when `kiwix-serve` is running;
4. saved websites (the plain-text copies the website saver makes);
5. **loose files and folders** (`core/src/filesearch.rs`): `.md`, `.txt`, `.html`, `.htm` anywhere on the
   drive, plus any folders added in Settings. Program folders (`engines`, `models`, `maps`, `music`,
   `backups`, `content`, `scripts`) and hidden folders are skipped.

Loose-file search is a fresh scan per question, held to a time budget (about 1, 2.5 or 8 seconds:
Quick, Normal, Thorough) and a count of files and folders, so a very large drive never hangs the
assistant; it answers from what it saw. A stored index would remove that limit and is the next step if
the drive is huge. **PDFs are not read by this scan:** their text is read by the Shelf, so put PDFs in
`library`, `books` or `manuals` and they are added to the Shelf automatically.

## Settings

One file, `user/settings.json`, on the drive. The list, defaults and ranges are in
`src/lib/settings.svelte.ts`; the few settings the Rust side acts on are read in
`src-tauri/src/settings.rs` and must keep the same defaults. Every setting changes something real.

| Tab | Settings |
|---|---|
| General | interface size (85 to 150%), start screen, status bar |
| Assistant | model choice; what it may search (books, archives, files, saved websites); passages read (1 to 6); answer length; how long to look through files; show sources; extra folders |
| Library | put drive books on the Shelf automatically; pick up new archives by themselves |
| Intel | what a new tab opens on; remember open tabs |
| Music | small player at the bottom; scan the music folder at start |
| Maps | miles or kilometres |
| Plan | act-now days, check days, use-by window in months (Inventory and What fails) |
| Drive | where the drive is; reset every setting |

Sharing the library on the network is deliberately **not** a setting: it is off at every start and is
turned on, with a warning, in Archives.
