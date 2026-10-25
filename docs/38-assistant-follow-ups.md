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
Every page is marked draft with its `Source:` line; nothing here claims a medical check. Kitchen amounts
(a teaspoon of honey, a cup of water) stay. Medicine-style amounts (mg, mcg, units, ml with a medicine) are
refused by the same loader rule as everywhere else, so three pages (boswellia, valerian, salt-water gargle)
were changed to "follow the product label". NOMAD's FDA drug reference is not included.
