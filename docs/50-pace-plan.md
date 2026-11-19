# 50. The comms PACE plan

PACE is four ways to reach each other, tried in order: Primary (phones, internet and power work),
Alternate (phones down, internet up), Contingency (all down: radio) and Emergency (the last resort).

## How it works

- Settings > PACE plan asks 15 plain questions (the group, then one set per level), each with a hint and an
  example, and an answer box. Nothing is required.
- The answers are kept in `user/pace.txt`, one `key: answer` line each (a line break inside an answer is
  written `\n`). The earlier file is copied into `user/backups/` before every save. Answers are limited to
  1000 characters. Host-only: `pace_load` and `pace_save` are not allowed for guests.
- The Comms plan page shows the structured plan built from the answers (the group, Primary, Alternate,
  Contingency, Emergency), with what is still unanswered, and a Print button. With no answers it shows a
  placeholder with a link to Settings > PACE plan.
- The free-text Comms plan (per person, can be locked) is unchanged and still has its own PACE section.

## Where the code is

- `core/src/pace.rs`: the questions, load/save, `build`, `to_text`; tested in the same file.
- `src-tauri/src/lib.rs`: `pace_load`, `pace_save`. `src/lib/api.ts`: `paceLoad`, `paceSave` (browser preview keeps answers in memory).
- `src/views/PaceSettings.svelte` (the questions), `src/views/CommsPlan.svelte` (the plan and placeholder).

## Not done yet

- Tied to Security so far: `pace::looks` compares the Contingency radio answer with each site's channel plan and adds a line to "needs a look" (no answer, site still on the starting channels, or the site's main channel not mentioned). Still not tied: the free-text Comms plan text, and the comms log entries themselves.
- The screens have not been type-checked or looked at in a browser (the tooling was not available).
