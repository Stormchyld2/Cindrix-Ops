# 51. Survivalist and prepper hybrid

**DESIGN (owner approved all eight parts, 2026-11-15). Nothing here is built yet.**

Cindrix Ops has counted readiness the prepper way: what is on the shelf and when it runs out. The owner plans for
a long disruption, where supplies are a bridge and skills carry the group afterwards. This doc gives skills and
long-term self-reliance the same weight as supplies. It adds to the program; it takes nothing away.

## What is already there

The program is further along this road than its name suggests. These stay and get built on:

- **Supplies:** Inventory, shelf life and places (doc 42), "How much to have" (`stockplan.rs`), and the "What fails"
  page (`cascade.rs`, `Cascade.svelte`) that says which supply runs out first.
- **Skills:** the Academy with tiers 3, 2, 1 (`academy.rs`), per-person sign-off (`training.rs`,
  `Training.svelte`), 22 drills and 22 skills cards. The basics course already covers shelter, fire, water and
  navigation, and there are Tier 2 courses for food production, the seed vault, water and land navigation.
- **Long term:** the Seed vault, planting and frost dates on the calendar (`seasons.rs`), and a "Weeks to months"
  plan (`content/plans/long-emergency.md`), which is short and still marked `status: draft`.

## What is missing

1. Nothing measures skills or self-reliance next to supplies. The Dashboard only knows about stock.
2. Sign-off is per course. Nothing shows the group at a glance, or that only one person can do a thing.
3. "What fails" says when a supply runs out, not what takes its place.
4. The fieldcraft lessons assume a pack. There is no track for when the gear is gone.
5. Wild food is one card ("Wild food: be very careful"). There is nothing for the owner's own region.
6. There is nothing on trades and repair.
7. Planning stops at "weeks to months".
8. The README and docs describe a library and a stockpile, not skills and the long haul.

## The eight parts

### 1. Readiness row on the Dashboard

A row of three tiles, in the same style as the security row:

- **Supplies:** days until the first supply runs out (from `cascade.rs`, as now).
- **Skills:** how many must-have skills are covered by two or more people, out of how many (part 2).
- **Self-reliance:** how many needs have a renewable source in place, out of how many (part 3).

Each tile shows a plain count and opens its page. No single blended score: one number would hide which of the
three is weak. New `core/src/readiness.rs` gathers the three counts; tested there.

### 2. Skills matrix

A grid of people (from the Family page) against skills, on a new "Skills" view inside Training.

- `content/skills.csv` ships the list: `skill, area, proof, must_have`. `proof` names the course, drill or card
  that shows the skill (for example a signed course, or a drill done in the last 12 months).
- A skill with no course behind it can be signed by hand: `user/skills-signoff.csv`
  (`person,skill,signed_by,signed_date`), the same shape as the course sign-off.
- The family can add its own skills in `user/skills.csv` (template shipped, as for cards and plans).
- Each box is one of: signed, learning, not started. Each row is flagged in plain words when it needs a look:
  "Only one person can do this", "Nobody can do this yet", "Drill not done this year".
- New `core/src/skills.rs` builds the grid from the list, the course sign-offs and the drill log. Host-only.

### 3. What replaces it (on the "What fails" page)

Each supply on "What fails" gains a second line: what takes its place when it is gone, what skill that needs,
and how long it takes to set up.

- `content/replacements.csv` ships the pairs: `need, stored_as, replaced_by, skill, lead_time`. Examples: stored
  water to a spring or rain catch plus boiling; canned vegetables to a garden plus canning (lead time: one
  season); matches to a ferro rod, then friction fire; store soap to soap made from fat and ash lye.
- `user/replacements.csv` keeps the family's own state for each need: not started, learning, or in place.
  Edited on the page; earlier file backed up before each save.
- The Self-reliance tile counts the needs marked "in place".
- A supply that runs out before its replacement's lead time is flagged: "Start now: this takes a season".
- Logic goes in `cascade.rs` (matching) and a new `core/src/replace.rs` (the two files); tested in both.

### 4. Fieldcraft without gear (Academy)

Three new courses, one per tier, walked in order like the rest:

- **Tier 3, Fieldcraft essentials:** shelter from what the woods give, fire from a spark and natural tinder,
  finding and boiling water, direction from sun and stars, signalling.
- **Tier 2, Living off the land:** friction fire, cordage from plants, traps and snares, fishing without tackle,
  containers and cooking without pots, tracking and sign.
- **Tier 1, The long stay:** a camp that lasts seasons, making and mending tools, hides, winter, moving a group.

Each lesson ends with a hands-on check that is signed off, not only a quiz, because these are skills of the
hands. Matching drills go in `content/drills` and feed the skills matrix. Course folders only; no code.

### 5. Living off the land in West Virginia and Tennessee

- One Tier 2 course, **Wild food of the Appalachians and the Tennessee hills:** plants, nuts, fungi, game and
  fish by season, and how to clean, cook and keep them (smoking, drying, salting, with links to the canning
  guides already in the library).
- A skills card for each main plant, animal and method.
- `content/forage-seasons.csv` (`item, region, from_month, to_month`) lets the calendar show "In season now",
  through `seasons.rs`, beside the planting dates.
- Safety rules, written into every plant and fungus card: eat nothing that is not identified for certain; the
  dangerous look-alikes are named on the same card; when in doubt, leave it. The existing "be very careful" card
  stays as the first card in the group.
- Trapping, hunting and fishing lessons say that state seasons and licences apply in normal times.
- Sources are public ones (USDA, state extension services, state wildlife agencies), each on a `Source:` line
  and in `content/sources.json`.

### 6. Trades and repair

One Tier 2 course with a module for each trade: sharpening and tool care, sewing and mending, shoes and
leather, soap, candles and lamps, hand-tool woodwork, and basic metalwork. A card and a drill for each. These
are the "skill" column for many rows in part 3.

### 7. The long term in Plan

- Rewrite `long-emergency.md` as a full plan and take off `status: draft`.
- Add two plans: **The first year** (a season-by-season walk: plant, harvest, preserve, winter) and **Year two
  and after** (seed saving, livestock, replacing tools, teaching the next person).
- Steps link to the Seed vault, "What fails", the skills matrix and the courses above, and tick themselves
  where `planfacts.ts` can tell (for example, a seed inventory exists; two people signed for water).
- The five emergency-management principles the owner asked for (prevention, preparedness, response,
  mitigation, recovery) shape the sections.

### 8. Wording

- The splash already reads "survival, preparedness, self-reliance" and stays.
- README first line and the docs: "An offline survival and preparedness platform: supplies, skills, and the long
  haul." Update the README status paragraph, which is out of date.
- Names of other people's archives in the library catalog (for example "Urban Prepper") are their titles and
  are not changed.

## Rules carried over

- All of it works on Go, Vault and Hub. Anything that can be switched off goes through `Features` in
  `product.rs`.
- People's data (the skills grid, sign-offs, replacement state) is host-only and lives under `user/` as plain
  text. Courses, cards and drills are library content and guests can read them.
- Nothing uses the internet. No medicine doses. Every card has a `Source:` line.
- Lessons are written in full the first time, not as drafts.
- The left nav is not changed. The new pages sit under the entries that exist (Dashboard, Training, What fails,
  Academy, Plan).
- A test in `core/` for every new piece of logic.

## Order of work

Built on the working branch and handed over as one complete build:

1. The three list files and their templates (`skills.csv`, `replacements.csv`, `forage-seasons.csv`).
2. Core logic with tests: `skills.rs`, `replace.rs`, the `cascade.rs` and `seasons.rs` additions, `readiness.rs`.
3. Screens: Skills view, the second line on "What fails", the Dashboard row, "In season now" on the calendar.
4. Content: the five courses, their cards and drills, the three plans.
5. Wording.

## Open decisions

- **Which skills are must-have.** The shipped list needs the owner's eye, since it decides what gets flagged.
- **Two people per skill** as the bar for "covered". With a group of four, is two right, or three for a few?
- **Hunting and firearms depth** in part 5: game care only, or marksmanship and hunting method too?
- **Pocket:** should the phone show the skills grid and "In season now", or the host only?
