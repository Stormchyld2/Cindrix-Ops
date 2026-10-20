# 20. The archive reader

The reader is the screen inside the app (Library, then Archives) where ZIM files are read:
Wikipedia, medical guides, repair manuals and the websites you saved yourself.

## How it works

Kiwix's own program, `kiwix-serve`, still does the hard part: opening ZIM files, decompressing them
and searching them. The app starts it in the background on this computer only. The reader is **our own
screen on top of it**, not Kiwix's page:

- Back, Forward and Home buttons that follow your path through the archive.
- A search box with suggestions as you type (article titles), and "Search text" for a full-text search.
- Bookmarks (the star), kept per archive in `user/zim-bookmarks.json`.
- Text size (A− and A+, 70 to 200 percent), remembered.
- An archive picker when more than one is on the drive.
- Links that go to the internet are not opened; the reader says so. The library works offline.

Pages reach the reader through a private address (`cindrix-zim`) that passes only the library parts of
`kiwix-serve` (articles, its style files, search, suggestions, the list of books) and nothing else
(`core/src/kiwix.rs`, `proxy_path_ok`, with tests). A small script is added to each page so the
reader knows which page is open. The web view plays no part in finding files on the disk.

"Use the plain Kiwix screen instead" (under the reader) switches to Kiwix's own page, in case a
particular archive misbehaves in ours.

## Not built, and why

- **Our own ZIM reader without `kiwix-serve`.** The format is compressed with Zstandard and XZ, which
  need two outside libraries, and full-text search needs Kiwix's search index format. Possible later;
  it is what the Android Hub would need, because `kiwix-serve` does not run on Android.
- Several pages open at once (tabs), find-in-page, printing an article.
- Large videos inside an archive: they are passed whole, up to 64 MB, so very big ones will not play.

## Tested and not tested

Tested: the address filter, redirect handling (including a loop and an attempt to leave the library),
reading the book list, adding the script and base address to a page, and the whole reader flow in a
browser with a sample archive. The check against a **real kiwix-serve** on GitHub (`probe_kiwix`) tests
that the book list, main page redirect, suggestions and search answer the way the reader expects.

**Not tested by a person yet:** the reader in the Windows app with a real Wikipedia archive.
