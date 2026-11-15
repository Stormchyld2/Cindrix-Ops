# 53. Archive reader and Kiwix: how they fit, and the compatibility rule

Status: BUILT (fixes below); the Windows result is **TO TEST** on real hardware.

## How the pieces split

- Pages, pictures and title suggestions come from our own reader (`core/src/zimread.rs`, `library.rs`), through the `cindrix-zim` address (`src-tauri/src/zimview.rs`). The share server uses the same code. No Kiwix program is involved.
- Full-text search (`/search`) and Kiwix's style files (`/skin/`) come from `kiwix-serve` when it runs. The catalog reader and the "plain" mode also use it.
- Tabs, bookmarks and cross-archive search are ours and do not depend on kiwix-serve.

## What went wrong (found 2026-11-15)

1. The reader built its frame address as `origin + path` where the origin kept a trailing slash, so the request was `//content/<book>`. The route check failed and the request fell to the last line, an empty 404. It hit every archive. Fixed in `src/lib/api.ts` (`zimOrigin`) and the server now folds a doubled leading slash (`library::clean_request_path`).
2. The same trailing slash made the reader's own `toPath` return a path with no leading `/`, so tab history and bookmarks were never updated from the page. Fixed by the same change.
3. A query string on a page address (`page?x=1`, common in crawled sites such as libretexts) was dropped. Now `Library::page_with_query` tries `page?x=1` as the address first, as Kiwix does, then the plain address.
4. Pages that ask for `/assets/app.js` (single-page-app archives) matched no route. They are now looked up in the archive named by the asking page (`library::book_from_referer`).
5. An archive that failed to open was silently missing. `Library.failed` keeps the reason, and Intel shows a plain-words notice (`zim_unreadable`).

6. Found by the new official-catalog job on the real `ready.gov_en_all_2026-10` archive: current Kiwix archives write the header's title-list position as "none" (all bits set) and keep the title order in the entry `X/listing/titleOrdered/v1`. Our reader added to that position (an overflow panic in a test build, a wild read in the real one), so title suggestions and title search in the reader were broken for such archives. `zimread.rs` now treats a position outside the file as "none" and reads the listing entry instead. Test fixture: `tests/fixtures/zims-reader-only/new-nolist-zstd.zim` (made by `make_zims.py`; kept out of `zims/` because zimcheck rejects this hand-made layout, so it only tests our reader).

Not the cause (tested): book names. kiwix-serve names a book after the file name, the same as our reader. The official `libretexts.org_en_bio_2025-01` archive (ZIM 6.2, zstd) opened and matched kiwix-serve byte for byte in the sampled pages.

## The permanent rule

Archives from the official Kiwix catalog must read the same in our reader as in kiwix-serve. CI enforces it:

- `official-catalog` job in `ci.yml`: downloads a small official archive and kiwix-serve and runs `cross_check` (fails on any disagreement) and `diagnose_zim`.
- `scripts/check-zim-urls.mjs`: the frame address has one slash.
- `core/tests/library.rs`: doubled slash, referer, query, failed archives.

`core/examples/diagnose_zim.rs` is read-only and prints names, start page, namespaces, redirects, and a sampled byte comparison for any `.zim`: `cargo run --release --example diagnose_zim -- file.zim [kiwix-serve]`. Never delete it. Do not modify or replace a user's ZIM files.

Intel has a "Check the reader" button. It prints the address the reader uses, the archives found and any that could not be opened, and for up to four archives what the page server answers (status, type, size) and whether title suggestions work. If a page ever fails to open on a real machine, send that text.
