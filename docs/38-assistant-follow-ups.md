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
