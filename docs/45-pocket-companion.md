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

## Open questions for the owner

- Does each phone belong to one person (picked from the family list when it is set up), or can a phone be shared?
  Reports need a "who wrote it".
- Should Pocket keep a read-only copy of the watchbill and posture so they can be seen offline?
- The whole map pack, or one small area chosen for camp and the route?
- Which book formats must the reader open on day one?
