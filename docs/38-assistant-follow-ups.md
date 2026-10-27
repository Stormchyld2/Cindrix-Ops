# Follow-up questions (doc 38)

The assistant remembers the last few questions in the same chat, so "and for children?" or
"what about in winter?" still finds the right pages.

- `core/src/followup.rs` decides whether a question is a follow-up (it starts with "and", "what about",
  "how about", "what if", "but", "so", "or"; it uses a pointer word such as "it", "that" or "this"; or it
  is very short) and, if so, adds the words of the most recent earlier question to the search.
- The words sent to the safety check include the earlier question, so a follow-up to an insulin
  question is still refused (`core/tests/safety/dosing.json` rules apply unchanged).
- Only the last 2 earlier questions are used and each is cut to 300 characters. Nothing is stored:
  the interface sends the earlier questions with each ask (`src/lib/api.ts`, `ask(..., history)`),
  and the share server accepts up to 4 in `body.history`.
- Idea taken from Project NOMAD (Apache-2.0), written fresh for this code base.

## Answer style

Settings > How it answers > Answer style: Exact, Steady (default) or Natural. It changes the model's
sampling numbers (`core/src/llama.rs`, `Sampler::named`): temperature 0.1 / 0.2 / 0.5 with matching
top-k, min-p and repeat penalty. Exact sticks closest to the pages; Natural reads more like talk.
The safety rules are not affected by the style.

## Golden questions

`core/tests/golden/questions.tsv` lists plain questions and the page each answer should come from.
`core/tests/golden_questions.rs` runs the assistant's own keyword search over the shipped pages and
fails if a page is not in the top three. To add one, add a line (question, tab, page file name).
Writing it found two real gaps, now fixed in `core/src/retrieval.rs`: a card's `title:` now counts like
a heading, and "burns" matches "burn". (The test idea comes from Project NOMAD's eval folder.)

## Archive list and download script

`content/library-catalog.json` lists 61 Kiwix archives in six topics (medicine, survival, education, how-to,
farming, computing), each in three tiers (essential, standard, comprehensive). `scripts/get-library.ps1`
(double-click `Get-Library.bat`) fetches them into the drive's `library` folder: `-List` shows what there is,
`-Category` and `-Tier` choose, the total size is shown and confirmed first, finished files are skipped, and a
broken download leaves a `.part` file that is replaced on the next run. It runs outside the program, so the
program never goes to the internet. The list is adapted from Project NOMAD (Apache-2.0).
Left out on purpose: NOMAD's "FDA Drug Reference" (drug dosing) and its creator-pack videos (a separate
license that does not allow copying). The medical archives are other people's books; the assistant's
no-dose rule still applies to everything it says.

## Home remedies

`content/guides/home-remedies` holds 38 pages (20 simple home measures, 18 herbal products) reworded from
Project NOMAD's remedy lists, which summarize CDC, MedlinePlus and NCCIH pages. Menu: Intel > Home remedies.
Every page is marked draft with its `Source:` line; nothing here claims a medical check.
The no-dose rule is for prescription and over-the-counter drugs only (owner, 2026-10-26). Herbal,
homeopathic, holistic and kitchen amounts (a teaspoon of honey, 300 mg of valerian, a cup of water) are
allowed: `core/src/safety.rs` `herbal_amount` ignores an amount that sits next to a herb or kitchen word
when no named drug is close by. A named drug (aspirin, insulin, ibuprofen) is still refused. NOMAD's FDA
drug reference is not included.

## Drug labels (reading only)

Intel > Drug labels shows the FDA's own label text for medicines. It is a reference to READ: the page says
it is not advice, and the assistant still refuses to state or change a drug dose (decided 2026-10-26: the
no-dose rule is for prescription and over-the-counter drugs, and a published label is a source the person
reads for themselves).

- `scripts/get-druglabels.ps1` (double-click `Get-DrugLabels.bat`) reads the FDA's file list
  (`api.fda.gov/download.json`) and saves the label files to `<drive>/drug-labels/raw`. It runs outside the
  program, so the program never goes online.
- The screen's "Get the labels ready" button runs `core/src/druglabels.rs` `build`: it reads each file one
  record at a time, keeps the boxed warning, what it is for, dosage (as published), do-not-use, warnings and
  interactions (each shortened to 8,000 characters), and writes `<drive>/drug-labels/drug-labels.txt`.
- Search matches brand and common names first, then what a medicine is for. Host-only: the four commands
  are not in the share allowlist (a test lists them as denied).
- Not tested here: the real FDA download and the Windows run (CI builds the code; the script needs a real try).

Update 2026-10-26: the list now also has Wikipedia (NOMAD's `wikipedia.json`: Quick Reference, Popular
Articles, Complete without images; pick one) and each entry says `"source": "nomad"`. Archives that
are NOT from NOMAD live in `to_find`: name, a search word for https://browse.library.kiwix.org, and why. They have
no download address yet because none could be checked from here (Khan Academy is first); paste an address
into the list, or use `get-library.ps1 -Url`. `-List` prints that checklist. Food for Preppers was listed
twice and now appears once.

## Private drive and the version to sell (owner, 2026-10-27)

The owner builds a private library first and will strip it to public-domain and openly licensed material for
a version to sell. Every archive in `content/library-catalog.json` therefore has a `license` and a `sell_ok`
of `yes` (public domain or openly licensed, credit where it is required), `check` (not verified) or `no`
(non-commercial or copyrighted). `get-library.ps1 -SellOnly` fetches only the `yes` ones; `-List` shows the flag.
The flags are Claude's reading of each license, not legal advice: check them before selling. A test refuses a
`yes` next to anything marked non-commercial. Defense and weapons books are allowed for the private drive.

## Get-Library is for the sell version only (owner, 2026-10-27)

`scripts/get-library.ps1` and `content/library-catalog.json` stay in the repository, but the CI-built drive
no longer carries them: the owner's private library is built by hand. For the sell version, add these lines back
to the "Include the drive and map scripts" step in `.github/workflows/ci.yml` and run the script with `-SellOnly`:

    cp scripts/get-library.ps1 scripts/Get-Library.bat out/Cindrix/scripts/
    mkdir -p out/Cindrix/content && cp content/library-catalog.json out/Cindrix/content/

## Archive categories from folders (owner, 2026-10-27)

An archive's category is the first folder it sits in below the library folder: `library/Medicine/x.zim` is
"Medicine", `library/Survival/Water/y.zim` is "Survival", and a file straight in `library` has none
(`core/src/library.rs` `category_for`, tested in `core/tests/library.rs`). It works like the Shelf, which takes
its category from the folder too. Intel shows a category picker next to the search box (only when there are
categories); it narrows the search and the list of archives. Archives lists each archive's category. Other
libraries' `library` folders work the same way. Two archives with the same file name in different folders
still count as one (the first is used). The guest search accepts the same optional `category`.

## Word guardrails (`core/src/senses.rs`)

Many everyday words have more than one meaning ("dress a cut", "dress for the cold", "dress a board";
"charge a phone", "charge in court"; "burn", "shock", "stock"). `senses.rs` holds a small table of rules.
Each rule reads the rest of the question for a clue, and only then:

1. rewrites the search words to the right meaning ("dress a cut" searches for wound, dressing, bandage, clean);
2. pushes passages that are plainly about the wrong meaning down to a fifth of their score (a dulcimer
   chapter has "saw", "jointer" and "glue" and nothing about wounds).

A question with no clue is searched exactly as written. A second guard: with four or more different
words in the question, a passage that holds only one of them is halved. Both apply to the
shipped pages, the Shelf, saved websites and the Kiwix archives (`retrieval::score` and `kiwix::relevance`).
To add a word, add one `Rule` in `senses.rs` (ambiguous word, clue words, words to add, words of the wrong
meaning, words of the right meaning) and a golden question in `core/tests/golden/questions.tsv`.
Rules now: wound care, clothing for weather, burn injury, electric shock, charging a battery, food stores, cold (illness or exposure), strain (muscle or filter), pressure (blood or canner), stroke, plant (garden), drill (practice), bug-out.
The coverage guard only halves a passage that holds just one of four or more question words. Markdown bold inside a word (`**S**top`) no longer splits the word when pages are read.

## "How do I dress a cut" (search fix)

The library search matched "dress" and "cut" in a dulcimer-making chapter. `retrieval::query_tokens` now
reads a question that has a wound word (cut, scrape, gash...) together with dress/wrap/cover/clean as
a question about bandaging a wound, and looks for "dressing", "bandage", "clean" and "wound" instead.
Page text is still read as written. Golden question added (`wound-care`).

## Growing, livestock, soil and water, radio, comms plan, seeds, settings (this round)

- **Crop guides** went from 18 to 39 pages (broccoli family, greens, roots, legumes, fruit, herbs, grains, mushrooms and more).
  **Planting calendar** now has "Every crop: when to start, sow and plant out (weeks from frost)", "Soil temperature for sowing", and
  month-by-month pages for zones 3-4, 5, 6, 7, 8, 9 and 10 (generated from the same week offsets, so they agree).
- **Livestock** has cattle, pigs, sheep, other poultry, fish, worms and insects, fencing, pasture, hay, daily health check, predators,
  milk to cheese, butchering, work animals, dogs and cats, a breeding table, feed and land per animal, livestock without power, records
  and "which animals first". No animal medicine amounts: pages send people to a vet.
- **Soil and water** adds texture and pH tests, manure safety, worm bins, no-dig beds, deficiency signs, weeds, drip and ollas, swales,
  wells and springs, ponds, greywater, household water treatment, drought and a small-plot plan.
- **Seed vault**: "Seeds to add" tab with 138 suggested crops (`core/src/seedlist.rs`), added to My seeds in one tap with planting words the
  Calendar understands (a test checks every line parses). Seven more seed guides. More crops have a seed-life figure.
- **Comms plan** is now a full 19-section plan (`core/src/commsplan.txt`): activation levels, roles, contacts, PACE, equipment, channels,
  schedule, authentication and duress words, plain messages, meeting places, no-radio signals, power, news, neighbourhood network,
  safety on air, children and elderly, emergency call format, practice, and a log. "Add the sections I am missing" appends sections an
  older saved plan lacks without changing anything the person wrote. Stored per person in `user/people/<name>/comms-plan.md`.
- **Radio reference** went from 30 to 55 pages (frequency chart, FRS/GMRS channels, CB, marine, aircraft and beacons, licences, HF,
  digital modes, repeaters, Q-codes, antenna lengths, lightning and EMP, weather radio, satellite, phones, net script, groups, glossary).
- **Settings**: a Calendar tab (week start, 12/24-hour time, month or week view, and which automatic items show), and "Earlier questions
  it remembers" under Assistant (0 to 4).
- **Privacy**: journal, comms plan and personal photos are plain files in the person's folder under `user/people/<name>/`. Guests on the
  hotspot cannot use any journal or comms plan command and see only the shared photo album. On the host computer the person picker lets
  anyone using the program choose a name unless the Family page lock is on, so the folders separate people but are not a password.
