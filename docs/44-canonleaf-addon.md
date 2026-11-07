# 44. Canonleaf add-on (built 2026-11-07, not yet run by a person)

Canonleaf is the owner's separate offline Bible study program (repository `Stormchyld2/canonleaf`). It stays its own
program. This add-on lets Cindrix Ops **read** the Bible text Canonleaf keeps in its own folder, so Ask can quote and
cite verses and the Bible screen can show chapters, also to guests on the share server.

## What it does
- **Settings > Assistant > Canonleaf**: a Browse button picks the Canonleaf folder (the one with `resources/`). The setting is
  `canonleafFolder` (one folder). A line under it says how many Bible texts were found, or what is wrong, in plain words.
  `searchCanonleaf` turns the verses off for Ask without clearing the folder.
- **Ask**: a named verse ("What does John 3:16 say", "Romans 8:28-30", "Psalm 23") is quoted whole from the default
  edition. Other questions look for verses that contain at least two of the question's words, preferring words in a row.
  Every verse source names the edition ("Canonleaf: Berean Standard Bible") and links to the Bible screen at that verse.
  **Honest limit:** this finds wording, not ideas. A question about a theme may find nothing, or only partly matching verses;
  it never makes a verse up.
- **Bible screen** (Intel menu, shown only when Bible text is found): book, chapter, Back and Next. Opened from an Ask
  source it scrolls to the verse. Guests on the share server can read it (`canonleaf_chapter`, `canonleaf_books` are in
  `GUEST_COMMANDS`). `canonleaf_status` is **not**, because it shows the host's folder path.

## What it reads
`resources/bundled/<edition>/manifest.json` (`title`, `kind`, `default`) and `books/<CODE>.json` (verses with `number` and
`text`). The default edition is the one Canonleaf marks `"default": true` (BSB). Book file names are only ever made from the
fixed three-letter table in `core/src/canonleaf.rs`, never from typed text. Read-only: nothing is written to Canonleaf's folder
and nothing in it is run.

## Not done
- Commentary, cross references, Strong's words and the study packages (they are Canonleaf's own, and some are private
  development material; see Canonleaf's licensing notes). Only Bible text is read for now.
- Searching the other editions (ASV, KJV) from Ask; the default edition only.
- A faster search: every topic question reads the whole Bible text (about half a second in a debug build on this machine).
- Canonleaf's explain-a-verse feature; that belongs in Canonleaf.
- A real run on the owner's drive and on Android.

## Tests
`core/src/canonleaf.rs` has tests for finding editions, reading references, quoting a verse, topic search and chapter reading.
The scanner was also run on Canonleaf's real BSB, ASV and KJV files (31,000-odd verses) and found a bug (a curly quote cut in
half) that the tests had not; that is fixed. The Tauri and Svelte parts were written but **could not be compiled or type-checked
in the session that wrote them** (no crates or `node_modules` available); CI is their first check.
