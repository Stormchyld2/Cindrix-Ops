# 45. Cindrix Pocket: the phone companion

Status: design agreed with the owner on 2026-11-08. Nothing is built. Items marked **OPEN** wait on a decision.
The Pocket repo (`Stormchyld2/Cindrix-Pocket`) holds the first notes (`README.md`, `docs/ARCHITECTURE.md`, 2026-11-01);
this page records what was decided afterwards and what Cindrix Ops has to provide.

## What Pocket is

A stripped-down Android companion to the Hub. It replaces the idea of a separate "Go for Android". The line is now
Go (thumb drive), Vault (Windows laptop plus the big drive), Hub (phone host) and Pocket (the companion app).

- **On its own** (no Hub, no network): emergency cards, situation-response cards, checklists, the Academy, a map area
  the person chose, the books they marked for download, and Security's printable checklists. It opens and reads
  everything after one fill.
- **Connected** (on the Hub's or the Vault laptop's hotspot): it behaves like the Hub screens in a browser, but as a
  **trusted device**, so it can reach the whole drive. Ask is shown only while connected to a host that has the
  assistant running. When not connected, the Ask tab is not shown at all.
- **Looks:** the same CindrixOps logo and the charcoal-and-blue theme (doc 41).

## Decisions

1. **Pocket writes no files on the drive.** Its only writes are logs: security logs, incident entries and other
   reports. These are kept on the phone in an **outbox** (a mailbox folder). They stay there, untouched, until the
   phone reaches the Hub (mainly) or the Vault laptop. Then they are sent to the host, which stores them and uses
   them. The outbox is never emptied until the host says it has them (acknowledged by file name and hash).
2. **Reuse the Ops screens.** Pocket is built from the same Svelte interface and the same `core/` logic, wrapped for
   Android (Tauri). Do not write a second interface. Keep `src/lib/api.ts` the only place the screens reach their
   backend; on Pocket a second transport (HTTP to the host, local files when alone) sits behind it.
3. **AirLink stays a separate app.** Pocket picks up files from AirLink's receive folder (bundles, maps). Reports in the
   outbox may also be handed on through AirLink to another phone that reaches the Hub sooner (store and forward). The
   reports are plain files, so either route works. **OPEN:** whether that relay is wanted.
4. **A deliberate exception to "other devices use a browser tab, never a client app."** Guests still use a browser and
   a PIN, read-only, exactly as now (doc 34). The companion app is the one trusted client. It joins the Hub with the
   existing one-use invite QR (doc 34), then works like the Hub's browser but with trusted permissions.
5. **Situation-response cards** (gate, intruder, camp alert, missing person, missed check-in, after an incident) are
   written in Ops as ordinary card files (`kind: response`, `content/cards`). Pocket receives them in the bundle.

## The outbox and the Hub's mailbox

Same pattern as the Cindrix Safe mailbox (`core/src/safebackup.rs`, doc 40):

```text
phone:  outbox/<date>-<kind>-<id>.txt        one plain text file per report, written once, never edited
host:   user/inbox/pocket/                   files arrive here
        user/security/ (and other records)   the host moves an accepted report into the records
```

- A report is a plain text file in the same line format as `incident-log.txt` and the posture and watch files, with the
  phone's device id and a hash, so it can be checked and de-duplicated.
- The Hub checks every file as untrusted input (size, type, ids, times, no path tricks) and never opens anything that
  fails. Rejected files stay in the mailbox and are listed for the host.
- **Source and permissions.** A report from Pocket enters the incident log with source **reported**. It may add notes,
  decisions and tasks. It **cannot change an incident's status or activate a plan**: those need the host's own action
  (doc 44 section 14, imported alerts).
- **OPEN:** whether accepted reports go straight into the records or wait in the mailbox for the host to accept them
  (proposed: wait, with a count on Command). 
- Guests never write. A PIN never makes a named person (doc 34).

## What Cindrix Ops has to provide

1. A **bundle** the Hub can publish: cards, checklists, Academy, chosen map areas, marked books, plus a manifest with
   hashes (format in Cindrix-Formats).
2. The **trusted-device** join and a way to tell a Pocket's connection apart from a guest's. The share allowlist
   (`GUEST_COMMANDS`) stays as it is; a trusted device gets its own, larger list, kept in its own tested module.
3. The **mailbox**: `user/inbox/pocket/`, a checker, and a "reports waiting" count on Command and Settings.
4. `Features` flags so Pocket's build switches parts on and off (Ask only when connected) through `product.rs`.

## Answers from the owner (2026-11-08, second round)

- **The mailbox waits.** Reports from a phone wait in the Hub's mailbox until the host accepts them. Built:
  `core/src/security_pocket.rs` (see below).
- **The QR links a phone to the Hub as before** and gives better permissions than a browser. Browsers that were
  trusted before stay trusted. Pocket is the full CindrixOps program, just faster because it does not run in a
  browser, and starts with only the core parts (**field view**). Once connected to the Hub it shows everything the
  Hub has (**hub view**).
- **No stale posture.** In field view Pocket shows no posture, watchbill or incident list, because after days away they
  would be out of date. They appear when connected.
- **Battery.** In field view Pocket sends no requests and runs no refresh timers. It makes one attempt to reach the
  Hub when the app opens and when the phone joins a network (a network-change event, not polling), and keeps a light
  link only while connected.
- **Direct connect.** When the phone joins the Hub's Wi-Fi, Pocket notices, sends the outbox, and when the Hub confirms
  each file it clears that file from the outbox and switches to hub view (Ask and the rest appear). The AirLink relay
  between phones also works, because reports are plain files.
- **Map.** One small area chosen in camp, like the offline map reader. See below.

## Field view and hub view

| | Field view (not connected) | Hub view (connected) |
|---|---|---|
| Emergency cards and situation-response cards | yes | yes |
| Academy | yes (downloaded courses) | yes, all of it |
| Plans and checklists | yes, read-only copies | yes, with progress |
| Security | printable checklists and response procedures; **write a log or report** (goes to the outbox) | the full Security screen: posture, watchbill, incidents, Operations |
| Maps | the saved small area(s) | the whole map pack and directions |
| Books (Shelf) | only the ones marked for download | the whole library |
| Radio reference and comms plan guides | yes | yes |
| **Ask** | **hidden** | shown (only if the host's assistant is running) |
| Dashboard | a small "field" page: cards, saved map, outbox count | the full Dashboard |
| Inventory | **shown (owner, 2026-11-08):** scan barcodes and enter weekly counts; the counts wait in the outbox as a count sheet and the host accepts them (see below) | shown, as the person's permissions allow |
| Calendar, To-do, Projects, Family, Journal | hidden (they would be stale, and are private) | shown, as the person's permissions allow |
| Evacuation plans and protocols | yes, read-only copies, with the evacuation destination's map | yes |
| Archives, offline browser, drug labels | hidden (too big) | shown |
| Music | hidden | shown if the host has it |
| Sharing, Settings | a short Settings page (outbox, storage, connect) | the full Settings |

Pocket never shows a tab it cannot fill. A hidden tab is not greyed out; it is simply not there.

## Maps and tracking (owner, 2026-11-08)

- **Small areas per site, agreed.** Use the phone map cutter (`core/src/phonepack.rs`, doc 32). One named area per security
  site, and the owner wants **West Virginia and Tennessee** maps kept in the same place: each site carries its own area, and
  the evacuation destination carries the Tennessee one. Show the file size before saving.
- **Reuse the standalone Cindrix map viewer's code** (`docs/CINDRIXOPS_CMAP_VIEWER_ARCHITECTURE.md`) for the map in Pocket.
- **Full-screen map for navigation.**
- **Track your own movement with GPS,** the way a wrist navigator does: distance, elevation (gain and loss), time, and a
  line on the map. The track is a log: it is kept on the phone, sent through the outbox as a file, and never needs the
  internet. GPS on the phone is the one thing that costs battery, so it is only on while a track is being recorded, and the
  person chooses how often it samples. Not built; to be designed when Pocket starts.

## Inventory counts from phones (built on the Hub side, 2026-11-08)

Phones may scan barcodes (Cindrix Scan already does this for the Hub's Inventory) and enter weekly counts. **Decided (owner,
2026-11-08):** counts use the same waiting mailbox as the reports and must be accepted before any count changes. The "Pocket
writes only logs" rule still holds: a count goes out as a **count sheet** in the outbox, like a report.

`core/src/countsheet.rs`. A sheet (`# format: count-sheet 1`, device, person, date, then `item uid, number the phone saw,
number counted` per line) lands in `user/inbox/pocket/` beside the reports and is handed in the same way (`pocket_upload`; the
phone's code must match its sign-in). Command's "Waiting for connection" lists it with **Look at the numbers** (item, where, what
the phone saw, what the Hub has now, what was counted) and **Accept the counts**.
- A sheet only sets the number on items that already exist. It never adds, renames or deletes a line, and it stamps the day it
  was counted.
- All lines or none. If any item is no longer in the Inventory, nothing changes and the host is told to set the sheet aside.
- If a Hub number moved since the phone looked, the sheet is flagged and the button becomes **Accept anyway**.
- The same sheet twice changes nothing. Sheets and reports share one accepted-ledger, so the phone clears both.
- **The scanner (owner, 2026-11-08):** reuse the barcode-scanner code from Cindrix Scan (repo Stormchyld2/Cindrix-Scan) inside Pocket, so Pocket scans and counts itself. Cindrix Scan stays a separate app.
- Not built: new items from a scanned barcode that is not in the Inventory (it needs a way to name the item), and Pocket's side.

## What the Hub mailbox does now (built)

`core/src/security_pocket.rs`: files arrive as plain text in `user/inbox/pocket/`. Each is checked (name, size, text, header,
every line). Nothing enters the records until the host accepts. Accepted entries are always marked **reported**: they can
add notes, decisions, tasks and a new incident, never change a status or activate a plan. All entries or none are
added; the same file twice changes nothing; every file is kept (moved to `done/` or `set-aside/`, never deleted);
accepted names and fingerprints go in `user/security/pocket-accepted.txt` for the phone to read. Command shows
"Waiting for connection" with Accept and Set aside, and counts waiting reports under "Needs a look".

## Trusted devices, handing in reports, and erasing a lost phone (built, 2026-11-08)

- **Trust follows the device, not the person.** A phone is known by its own short code, made when its invite QR was used.
  The person's name is only a label; two phones for one person are two devices (`core/src/devices.rs`,
  `user/devices.txt`, an append-only `user/devices-log.txt`).
- **What a trusted device may do** beyond a guest: say hello and learn what is waiting for it (`device_check_in`), say it has
  erased itself (`device_erased`), hand a report to the mailbox (`pocket_upload`), and the picture commands it already had.
  It may not run any host screen. Guests get none of these. `core/tests/security_rules.rs` proves both.
- **Hand-in rules.** The Hub takes the phone's code from its sign-in, never from what it sends. The file name must start with
  that code, the report's own `# device:` line must match it, and the report goes through every check an accept makes; one that
  fails is removed again. The same report twice changes nothing; a different one under the same name is refused.
  When the host accepts a report, the next hello returns the names accepted, so the phone may clear exactly those.
- **Settings > Devices** lists every device (kind, person, code, when it joined, when it last reached the Hub) with
  **Erase this device**, **Lock out**, and **Found it: cancel the erase**.
- **Erase is an order that waits.** The Hub cannot reach a phone that is off. The device stays known; until it connects it can
  do nothing but ask what is waiting and answer. When it connects it is told to erase; when it says it has, or the host
  locks it out by hand, its sign-in is removed for good and it needs a new invite. A device that is locked out is refused
  everything. Every step is logged.
- **What erase cannot do (said plainly on the screen).** An app can erase only its own data and sign-in: saved cards, unsent
  reports, saved maps. It cannot wipe the rest of a phone. A browser can only be locked out. It does not keep a phone off the
  Wi-Fi or hotspot; changing the hotspot password does that, and a new PIN signs out anyone who knew the old one. If the lost
  phone is never switched on again the order never arrives, but lock-out still stops it signing in to the Hub.
- **Pocket's side (when it is built):** on "erase" it deletes its cards, outbox, saved maps and sign-in, then sends
  `device_erased`. It must do this before showing anything else.

Not built: the bundle, GPS tracks, new items from scans, and Pocket itself.

## Still open

- ~~Does each phone belong to one person?~~ Answered: yes, each phone has its own owner, and trust is by device code.
- ~~A trimmed emergency-contact card on the phone~~ Answered (owner, 2026-11-08): the phone carries the **full** emergency contact card, not a trimmed one. The Family page is host-only today, so the bundle the host builds for a trusted phone must include the card and the phone must keep it on the phone only (never in a guest-readable place); it is wiped by the erase order like everything else Cindrix keeps there.
- ~~Which book formats the reader must open on day one~~ Answered (owner, 2026-11-08): **the same as the Windows version**. Today that is PDF, Markdown (.md) and plain text (.txt) (`core/src/shelf.rs`). If the Windows Shelf gains a format, Pocket gains it too; the reader must not have its own list.

## Check-ins and patrols from the phone (later)

The Hub side of outings and patrols is built (doc 46, `core/src/security_outing.rs`) and is written by the person at the base,
from a radio call or a runner. A phone in the field could later put its own check-ins and patrol checkpoints in the outbox as
outing lines, marked **reported**, and the host would accept them like a report. Not built. The base's own log stays the
record, and a missing phone message never counts as bad news by itself.

## Check-in sheets (Hub side built)

A phone can hand in a **check-in sheet** to the same waiting mailbox: contacts, checkpoints and notes about someone who is already out. File format and rules are in `core/src/checkinsheet.rs`: `# format: checkin-sheet 1`, header `device`, `person`, `created`, then one line per check-in (`id`, outing, `contact`/`checkpoint`/`note`, time, two fields, words). Ids start with the phone's code. The host accepts or sets aside; accepted lines enter the outing log as "reported". A sheet cannot start, end, extend or cancel an outing. The phone side is not built (Pocket repo).



## GPS tracks (built on the Hub side 2026-11-09)

A phone can hand in a **track sheet** to the same waiting mailbox: where its person walked, from the phone's GPS. File format and rules are in `core/src/tracksheet.rs`:

```text
# format: track-sheet 1
# device: <the phone's code>
# person: <who walked it>
# created: 2026-11-08T10:00-05:00
# track: <track id, starting with the device code and a dash>
# outing: <outing id, optional>
<seconds since 1970 UTC> TAB <latitude> TAB <longitude> TAB <metres above sea level, or empty>
```

Rules: two points or more; times never go backwards; positions in range; all or nothing; the same file twice changes nothing; a different file claiming a kept track's id is refused. The host accepts or sets aside (Command). A kept track lives in `user/security/tracks/` (covered by the passcode lock) and changes no other record. Security > Operations > Walked tracks lists each with distance, time and climb (a climb or drop counts only after 3 m of change, so GPS wobble does not add up) and can copy one out as a GPX file (a plain copy in `user/exports/`).

## New items from a scan

Pocket sends scans the way Cindrix Scan does: an inventory import file in `inbox/inventory/` (`scan_id`, `action`, name, barcode, qty, unit, use-by; `core/src/scaninbox.rs`, docs/39). A barcode the phone has never seen is named by the person and arrives as a new line; the host accepts it in Inventory. A count of items already in the Inventory goes in a count sheet instead (above). Nothing changes until the host accepts.
