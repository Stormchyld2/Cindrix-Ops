# 37. The connecting layer: to-do list, calendar, projects, one search

**Built 2026-10-24** (owner request: "the layer that connects everything"). Code: `core/src/ops.rs` (all the logic, tested),
`core/src/userdata.rs` (saving), `src-tauri/src/ops.rs` (commands), `src/lib/api.ts`, and the screens `Todo`, `Calendar`, `Projects`
plus the shared pieces `src/lib/Connected.svelte` (links), `MonthGrid.svelte` (the month picker, also used by the Journal) and `nav.svelte.ts`.

It follows the design in the architecture proposal (section 3): shared records with stable ids, links between modules, a change log,
deterministic rules and one retrieval service. It does not move anything into one database: each module still owns its own files.

## What it is

* **Tasks** (`user/tasks.txt`): a title, an optional day, an optional repeat (every N days; ticking moves the day on instead of closing it),
  a project, notes, and links.
* **Events** (`user/events.txt`): a day, an optional time, an optional repeat (weekly, monthly, yearly; a repeat on the 31st lands on the last
  day of a shorter month), a project, notes, links.
* **Projects** (`user/projects.txt`): a name, open or finished, notes, and the saved Ask chats filed under it (a chat belongs to one project).
* **Links**: `kind:id` from any task or event to an inventory item (`item:12`), a plan (`plan:water-storage`), a card, a guide page
  (`guide:crop-guides/potatoes`), a person, a course, a drill, a project, another task or event, or a chat. Unknown kinds and unsafe ids are dropped.
* **The agenda**: one date-ordered list built by plain rules from tasks, events, the dates other modules supply (Inventory items that run out or
  expire, followed plan checks that fall due) and nothing else. Late things sit on the first day, longest overdue first. No guessing, no AI.
* **Project settings** (Projects > Settings tab): standing **instructions** for the assistant (up to 4000 characters) and up to **10 documents**
  (`MAX_DOCS`). A document is either a **link** to a file where it already is (a path on this computer or the drive; never a web address) or an
  **upload**, a copy kept in `user/projects/<id>/` (25 MB each, never overwrites, a number is added to a clashing name). Allowed: pictures
  (png, jpg, gif, webp, bmp, heic), pdf, txt, md, csv, Word, Excel, OpenDocument, rtf; nothing that can run. Pictures show as small previews, so
  a scan or photo from a phone can be dropped straight in. Taking a copy out deletes it; taking a link out leaves the file alone. When a chat
  is filed in a project, Ask gives the assistant the project's instructions (they rank first) and the text of its `.txt`/`.md` documents; other
  file types are listed by name only. Guests never get any of it. The assistant's safety rules (no doses) stay in force whatever the instructions say.
* **One search** over cards, plans, guides, inventory, tasks, events, projects, and the **Shelf's books and manuals** (by title, author and
  shelf; the Reading Corner, which comes from the `leisure` folders, is left out). Every word must match; a title match counts more.
  Music, and the words inside books and loose files, are not in it.
* **Suggested links**: while a task or event is being edited, words in its title and notes are compared with the titles of everything above.
  A thing is offered when it shares two subject words (plural endings ignored, common words ignored), or its only subject word. The person
  presses **Link** or **Not this one**; nothing is ever linked by itself, and a refused suggestion stays hidden until the program is closed.
* **"What is connected"**: everything that points at a thing, and a project's own tasks, events and chats.
* **The change log** (`user/changes.log`, newest 300 lines): what was added, changed, ticked or removed, shown on the Dashboard.

## Why nothing needs an index or a number of its own

Everything that can be linked already has a stable id (inventory item, plan, card, guide page, book, task, event, project). The search and the
suggestions look at those entries when asked, so there is nothing to build, keep up to date or repair. Loose files (scans, notes, PDFs) are the
exception: they have no id, so they are not searched or suggested; to connect one, add it to a project as a document. Searching inside every file
would need a real index, which is a separate, larger job.

## Project memory (what the assistant remembers about a project)

A chat inside a project gets, besides the project's instructions and its text documents, `ops::memory_passages`: one summary passage
(the project's notes, its open tasks with due days, its dates for the next 90 days, the documents it keeps, how many chats are filed in it)
that always ranks first, and up to two passages from the project's earlier chats (the newest 30, first 6000 characters each) that best match the
question. A project chat is allowed two more sources than other chats so this does not push the library out. Guests get none of it.
The assistant's safety rules (no medicine doses) are unchanged.

## Filled in by itself

* **Birthdays.** The Family page has a Birthday for each person. Saving the page keeps one yearly "<Name>'s birthday" event for each (`ops::sync_birthdays`,
  command `birthdays_sync`). Only events carrying the note "Added from the Family page." are made, changed or removed, so the person's own events are never
  touched. The Calendar is not passphrase-locked, so a locked Family page still puts the name and day on the Calendar; the page says so.
* **Planting times and frost days.** Settings > My details has the usual last spring frost and first fall frost (`profile.txt`: `last_frost`, `first_frost`, as `MM-DD`).
  `seasons::seasonal_dates` turns each seed's "When to plant" words into a day (a named month, or so many days from a frost date; "start indoors before last
  frost" is six weeks before). Words it cannot read, or a frost date that is not entered, leave that seed off rather than guessing. They are rules of thumb and
  say "about now" in their title. Only days inside the range asked for are made, so they never show as late.
* The Calendar shows all of it, colour-coded, as a large month or week.

## Rules

* All files are plain text, tab-separated, one record per line, written with `fsio::backup_then_write` (the old copy is kept in `user/backups`).
  A damaged line is skipped, not fatal. Dates that are not real days are cleared when read.
* These are the person's own notes, so **they are host-only**: none of the commands is in the guest allowlist (tested in `core/src/share.rs`).
* Nothing here phones out. Tasks and events never contain medicine amounts by design of the screens, but they are the person's own words, so no check is made.

## Where it shows

Operations menu: Calendar, To-do list, Projects. Dashboard: COMING UP (the agenda for 60 days) and WHAT CHANGED LATELY. Journal: a month calendar picks the day.
Ask: Projects group chats (see doc 25 and the Ask screen).

## Not built yet

Reminders when something falls due (there are no notifications yet), moving tasks between people, sync between Vault and Hub, and the dependency planner from the proposal.
