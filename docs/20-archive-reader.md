# 20. The archive reader

The reader is the screen inside the app (Library, then Archives) where ZIM files are read:
Wikipedia, medical guides, repair manuals and the websites you saved yourself.

## Built in: no Kiwix program needed

The program reads ZIM files itself (`core/src/zimread.rs`, `library.rs`). It understands:

- both file layouts (the new one, with all content in one namespace, and the old A / I / - one);
- stored, **Zstandard** and **XZ** clusters, with 32-bit and 64-bit offsets (`zstd.rs`, `xz.rs`, written
  for this project with no outside libraries, so they also build for Android);
- redirects, the archive's own start page and description, title lookup, and title suggestions as you type.

It does **not** read Kiwix's full-text search index (Xapian). Searching text inside articles still needs
Kiwix's `kiwix-serve` program; when that is running the reader uses it for "Search text". Without it,
"Search text" lists articles by title.

The reader screen:
- Back, Forward and Home buttons that follow your path through the archive;
- a search box with title suggestions as you type, and "Search text";
- bookmarks (the star), per archive, in `user/zim-bookmarks.json`;
- text size (A− and A+), remembered;
- an archive picker when more than one is on the drive;
- links that go to the internet are not opened; the reader says so.

Pages reach the web view through a private address (`cindrix-zim`). It serves pages, pictures and
suggestions from the archives, and passes `kiwix-serve`'s search and style files through only when that
program is running. A small script is added to each page so the reader knows which page is open.

"Use the plain Kiwix screen instead" (under the reader, only when `kiwix-serve` is running) shows
Kiwix's own page.

## The assistant searches every archive

When you ask the assistant a question it looks in **every** ZIM file on the drive, not one. Each archive is
searched on its own and the results are mixed best-first, so a small manual is not buried by Wikipedia.
- With `kiwix-serve` running: full-text search in each archive, plus the title search below.
- Always (no Kiwix program needed): the question's words, and pairs and triples of them, are looked up in
  each archive's titles ("water purification" finds *Water purification*); the best articles are read and
  the matching part is given to the assistant, which names the archive and article it came from.
- Title search finds articles whose titles hold the question's words. It will miss an answer buried in an
  article with an unrelated title; full-text search catches those.

## Tested and not tested

Tested here against **real compressed data**: the Zstandard decoder against libzstd on 62 inputs (levels 1
to 19, many blocks, checksums, several frames, skippable frames) and the XZ decoder against `xz` on 49
inputs; both survive truncated and corrupted input. The reader against ZIM files in both layouts with
Zstandard, XZ, stored and 64-bit-offset clusters, and the library and assistant search on top of it. On
GitHub, the reader is also compared article by article with the real `kiwix-serve` (`cross_check`), and
`zimcheck` checks the test archives.

**Not tested yet:** a real multi-gigabyte Wikipedia file (speed, memory), and the reader in the Windows
app. Test archives were made by our own script, so the check against Kiwix's tools matters.

## Not built

- Full-text search without Kiwix (reading Xapian indexes). Possible later, and a big job.
- Archives compressed with zlib or bzip2 (old and rare; the reader says so).
- Tabs, find-in-page, printing an article. Big videos inside an archive.
- Sharing the archives over the hotspot **without** `kiwix-serve` (a small web server of our own). That
  is the next step for the Android tablet Hub, since `kiwix-serve` does not run on Android.
