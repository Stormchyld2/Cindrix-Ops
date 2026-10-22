# 28. Street address search

The Maps screen finds "123 Main St, Denver CO" with no internet. The program holds no addresses; they are plain text on the drive.

## Where
`maps/addresses/<pack>/<X>.tsv` where X is the first letter or digit of the street's key (`A`-`Z`, `0`-`9`, `_`). A pack is one
download: `us` for the lower 48, Alaska and Hawaii, or a name of your own for one area. Every pack is searched.
Line: `street TAB number TAB zip TAB state TAB lat TAB lon`.

## How it is searched (`core/src/addresses.rs`)
- The street is stored in a **key form** (`norm_street`): capitals, no punctuation, short words (STREET to ST, NORTH to N).
  The same rules are in the download's SQL; `core/tests/address_keys.tsv` is checked against both (CI job `addresses`).
- Lines are sorted by street, then house number digits, then number text. A search is a binary search by file position,
  so a file of millions of lines is searched by reading a few places in it.
- Typed text that starts with a house number is read as an address ("123 N Main Street, Denver CO 80202"); a state, ZIP or
  town narrows it. The exact number is shown if present; otherwise nearby numbers on the same street, marked "(nearby)".
  The same street in many towns: the ones nearest the map's centre come first.
- If nothing is found the Maps search falls back to the places list as before.

## Download (`scripts/get-addresses.ps1`, `Get-Addresses.bat`)
Overture Maps addresses (GeoParquet on S3) read with DuckDB (fetched once to `engines/duckdb`). Step 1 saves the US rows into one
working file on the drive; step 2 writes one sorted file per letter. If it stops, running it again carries on with the letters
not yet done. `-Bbox` and `-Name` make a small pack. About 15 GB of working space; the result is a few GB.

## Honest limits
- Coverage is partial. Overture combines many open sources (about 126 million US points), so some places have most houses and some
  have none. "Not found" does not mean the house is not there. The location is the point the source gave, not a survey.
- The state is taken from the source when it gives a two-letter code; otherwise it is blank and only the ZIP narrows.
- The download could only be written, not run, in the build environment used to make it. The `addresses` CI job runs it on a
  small area of Denver; until that is green, treat the full-US run as untested.
- Licenses are mixed per source; see `licenses/README.md`.
