# 60. Search inside articles (no Kiwix program)

Until now the program searched archives by **title** by itself, and by the words inside articles only while `kiwix-serve` ran. This adds the program's own full-text search. It does not read Kiwix's Xapian index: it makes a word index of its own, so it also works for websites saved with "Save a website", which have no Kiwix index.

## What the person sees

- **Archives > Search inside articles.** Each archive shows its state: not made, made (how many articles, how much of each, size), stopped part-way, or out of date. Buttons: Make search index (with a choice of "Opening part of each article" or "Whole articles", and a rough size), Stop, Carry on, Remove index. One index is made at a time, in the background, with a progress bar.
- **Intel search** shows a second list, "Inside articles", under the title matches, each with the passage that matches best. The reader's own results page (and "Search inside" in an open archive) lists titles first, then articles by their words.
- **Ask** uses the same index: passages come from inside articles, not only from titles. Guests can search (library content); only the host can make, stop or remove an index.

## How it works (`core/src/ftindex.rs`)

- One index is a folder, `.search-index/<archive>-<id>/`, in the library folder the archive is in (so each drive keeps the indexes of its own archives; backups of `user/` never include them). The archive is never changed.
- Words are lower-cased, split at anything that is not a letter or digit, lightly stemmed (plurals, "ing", "ed"), and the commonest English words are left out. Title words count four times as much. The last word of a search also matches words that start with it. Every word must match; if too few articles match them all, articles with most of the words follow.
- **Depth.** `lead` reads the first 4,000 characters of each article (about 700 bytes of index per article; the default for archives over 100,000 articles). `full` reads up to 60,000 characters (about 2.5 KB per article).
- **Building** reads articles in order, keeps up to 3 million words in memory, then saves a sorted segment and notes progress. Stopping, or turning the computer off, loses nothing: the next start carries on from the last saved point. When every article is read the segments are merged, and `meta.txt` is written last. A folder without `meta.txt` is not an index.
- **Fingerprint.** The index remembers the archive's size and own id. A replaced file makes it "out of date" and it is not used.
- A damaged or cut-short index is refused when opened and never crashes a search.

## Not done

- Reading Kiwix's own Xapian index (would save the wait on official archives; large and cannot be tried against real archives here).
- Phrase search ("exact words in order") and per-language word rules (only English common words are dropped).
- Making an index for the Bible books or loose files (they have their own search).

Tests: `core/src/ftindex.rs` (words, numbers, finding, every-word rule, typing a start, endings, pieces equal one piece, stop and carry on, depth, out-of-date, damaged, empty), `core/src/library.rs` (search with and without an index, assistant passages, results page).
