# 36. Morale, Comms and Agro pages, and a folder for each person

Built 2026-10-23 from the owner's discussion. Menus: `docs/35-top-bar.md`. Logic: `core/src/people.rs`, `core/src/guides.rs`, `core/src/shelf.rs`.

## Reading Corner (Morale)

Books to enjoy, kept apart from the manuals so the two shelves never overlap.

- A book is a Reading Corner book when its category is **Reading Corner** (`shelf::LEISURE`).
- On the drive, anything in a folder called `leisure` (also `library/leisure`, `books/reading corner` and so on) is put there when the drive is scanned. `shelf::category_for` decides.
- The Shelf under Intel hides those books; the Reading Corner shows only them. Both are `Shelf.svelte` (`corner` switches it).
- "Move to Reading Corner" and "Move to the Shelf" buttons change one book's category. Your own books only.
- A source link in Ask opens a book even when it sits on the other shelf.
- Ask still searches both shelves.

## A folder for each person (`user/people/<name>/`)

| What | Where | Who sees it |
|---|---|---|
| Journal | `journal/YYYY-MM-DD.md` | this computer only |
| Comms plan | `comms-plan.md` | this computer only |
| Own photos | `photos/` | this computer only |
| Shared album | `user/shared/photos/` | everyone, including guests (look only) |

- Names come from the Family page; with none there is one person, "Me". The choice is remembered in the browser (`person.svelte.ts`).
- A name is made safe as a folder name first (`people::folder_name`): no `..`, slashes, drive letters or Windows reserved names.
- Files are never replaced silently: the earlier copy goes to `backups/` first. Emptying a day removes it but keeps a copy.
- Pictures are taken only when the first bytes say JPEG, PNG, GIF or WebP (not by file name; SVG is refused because it can carry scripts), up to 40 MB, and an existing name is never overwritten.
- `user/people` is never read by Ask's file search, for the host or for guests (`filesearch.rs`).
- A guest on the hotspot can list and open the **shared album only**. Journal, Comms plan, own photos and every write are refused (`GUEST_COMMANDS`, tested in `core/src/share.rs`).
- It is private from other devices, not from people using the laptop: there is no sign-in on the host, and the folder is plain files. The screens say so.

## Not decided yet

Guests cannot sign in as a person or write to their own folder. That needs a PIN per person and changes the share server's "read only" promise, so it waits for the owner's answer (docs/34).

## Guides (Agro and Radio reference)

Plain pages in `content/guides/<section>/*.md`, written like emergency cards (title, kind, status header, a `Source:` line at the end). Sections: crop-guides, planting-calendar, food-preservation, livestock, soil-and-water, radio-reference. Read by `cards::load_dirs`, so a page naming a medicine amount is left out. The drive's `guides/<section>` and `user/guides/<section>` are read too (the second not for guests). `core/tests/guides_content.rs` checks every shipped page has a Source line and none claims to be reviewed.

All pages are drafts written from general practice and public guidance. Home canning times and pressures are left out on purpose: the page sends people to the USDA Complete Guide to Home Canning.
