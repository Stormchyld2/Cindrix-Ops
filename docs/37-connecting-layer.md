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
* **One search** over cards, plans, guides, inventory, tasks, events and projects. Every word must match; a title match counts more.
* **"What is connected"**: everything that points at a thing, and a project's own tasks, events and chats.
* **The change log** (`user/changes.log`, newest 300 lines): what was added, changed, ticked or removed, shown on the Dashboard.

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
