# 39. Bible study (Canonleaf's files)

Intel > Bible study reads the resource files that the separate **Canonleaf** program installs, so the
Bible, commentaries, dictionaries and cross-references can be used from a Cindrix drive. Canonleaf stays
its own program; Cindrix only reads the same folders (it takes no outside crates, so `core/src/json.rs`
and `core/src/bible.rs` are our own small readers).

## Where the files go
`library\bible` on any library drive (docs/24). Copy Canonleaf's Bible folders (`bsb`, `asv`, `kjv`) and its
installed `library/resources/<id>` folders (or unzipped `.theopack` files) in. Any folder holding a
`manifest.json` (schema 2) at most five levels down is a module; folders starting with a dot, cache folders,
links, unsafe ids and duplicate ids are skipped (the first wins). A file named by a manifest cannot point
outside its folder. The list is looked at again every 20 seconds, or at once with "Look again".

## What is read
Bibles (`books/<CODE>.json`), commentaries (verse-range lists and Markdown chapters), dictionaries and topical
books (Nave, Torrey, Easton and so on), cross-references (`crossrefs/<Book>.json`, matched to the Bible's
edition) and the STEP lexicon for Strong's numbers. Not read: Song of Solomon in the "pages" shape, English
concordances, STEP verse tokens, whole-work theology files. Missing data is shown as missing, never filled in.

## Rules kept
- Read-only. Nothing is written to the drive. Nothing phones out.
- Guests on the share server may use it (nine read-only names in `GUEST_COMMANDS`; a test checks that
  every one only reads). Same screen, same commands (`src-tauri/src/share.rs`).
- The licence and attribution of each module are kept in the status and the attribution line is shown under
  the Bible text. Canonleaf marks its packs "review required" for sharing, so this is for a private collection.

## Ask
Setting **Bible study books** (Settings > Assistant, on by default). A question that names a verse brings in
that verse (score 50) and the first commentary note (40). A Bible-flavoured question brings up to three
topical or dictionary entries first (score 20). Sources open the Bible screen (`bible:JHN.3.16`,
`bible:term:<Title>`).

## Limits
Dictionary alternative names are only found inside the same-letter file. The search scans the whole Bible
(about 85 ms). BSB text can carry section headings inside a verse (that is how Canonleaf's file is).
Tested on a real library through the ignored test `real_library_smoke` (`CINDRIX_BIBLE_TEST`), not yet on
real hardware or through the hotspot (doc 22).
