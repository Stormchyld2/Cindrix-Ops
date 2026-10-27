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
