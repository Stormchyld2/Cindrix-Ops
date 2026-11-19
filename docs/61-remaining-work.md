# 61. Remaining work (checklist)

Written 2026-11-19 from the owner's roadmap, after reading the architecture docs against the source.
Nothing below has been run on real hardware. "Checked" means a test in `core/` or a CI job covers it.
Doc 57 is the short status page; this is the working list. Update both when something closes.

**Development completion: 0 / 8 phases.** This checklist is for managing development, not a claim that a phase is
complete. Tick a phase only after every "done when" line under it is true and checked. Earlier items (severity
High and Medium, below the phases) are filed under the phase they belong to.

## The eight phases

### 1. Consolidate approved branches and commands
- [ ] Every command has one reach. *Built:* `core/tests/access_classes.rs`; roles and named phones (`permissions.rs`).
- [ ] All approved work from `hub-any-drive` is merged to `main` (only when the owner says), with CI green on `main`.
- **Done when:** one branch carries everything approved; no unclassified command; CI green on that branch.

### 2. Finish ZIM and multi-library integration
- [x] Own full-text index inside archives, used by Intel, results page and Ask (doc 60).
- [ ] Large libraries and many devices: multi-gigabyte archives and several guests at once are untested; time and size
  of an index on a real 50 GB archive are unknown. Archives and Shelf from the second drive are not fully shared yet (doc 24).
- **Done when:** a multi-gigabyte archive indexes, searches and resumes after a stop on real hardware; four guests
  read at once without the host stalling.

### 3. Complete Android Hub platform support
- [ ] Not started. Waits for the real Windows test (doc 22) and the share server's hardware test (doc 49).
- **Done when:** the Hub runs on an Android phone from the drive, with the assistant, library and share server.

### 4. Complete Operations, Maps and communications
- [ ] Structured PACE plan tied to free-text plans and the Security comms log (doc 50).
- [ ] Due-date reminders and notifications (docs 37, 48); watch and patrol schedules may generate reminders (doc 44 section 15).
- [ ] GPS track recording and sensor logging in the main program (docs 45, 55). Pocket's tracks are kept (`tracksheet.rs`).
- [ ] Garden beds screen (doc 53); map data gaps (doc 57).
- **Done when:** a due item reminds the host with no network; a recorded track is saved and shown; PACE answers
  appear in the plan and in Security comms.

### 5. Complete Security Operations integration
- [x] Phases 1 to 4 built (command, watchbill, incidents, comms, reviews, qualifications, gear).
- [x] Named people on phones and roles (Reporter, Approver); who accepted a report is recorded; passcode recovery code.
- [ ] The larger role split (planner, watch supervisor, member, observer) after these two roles have been used.
- [ ] Overwatch import (format undecided, doc 44 section 15); Security data sync (phase 6).
- **Done when:** the acceptance matrix in doc 44 section 12 passes, row by row, in tests or on hardware.

### 6. Finish Pocket/Hub interchange interfaces
- [x] Mailbox, count sheets, check-ins, tracks, erase orders, approvals on a phone's browser.
- [ ] Vault–Hub two-way sync (inventory, gear, security logs): decided direction (owner, 2026-11-19): a phone sends a
  change sheet, the other side accepts it; changes on both sides are shown to a person, never merged silently. Not built.
- [ ] Pocket's own side beyond the field tools (on hold).
- **Done when:** a change made on a Vault reaches a Hub, and back, with a conflict shown rather than lost.

### 7. Close media and supporting-module gaps
- [x] Videos, podcasts, music, Bible study, Gear list built.
- [ ] Review the shipped reference content (cards, Academy, guides) with a person; Academy is thin (doc 57).
- [ ] Backups and interrupted writes: restore drill on a real Windows drive. *Built and checked:* torn writes, unique
  backup names, half-made backups, restore drill with the passcode lock (core tests).
- [ ] Standalone `.cmap` viewer (doc 57); `Soon.svelte` placeholders removed.
- **Done when:** a person has reviewed each shipped card and course; a restore drill passes on a real drive.

### 8. Pass the coding release gate and produce builds
- [ ] Every architecture doc reconciled with the source and CI (this pass did 44, 45 and 57; the rest still to do).
- [ ] The real Windows test (doc 22) and hardware plan (doc 49) done; findings fixed.
- [ ] Windows installer signed; licenses and `content/sources.json` complete (doc 57).
- [ ] Android build produced (phase 3).
- **Done when:** CI is green on `main`, the Windows build passes the hardware test, and the installer is signed.
  Then, and only then, call it "CindrixOps Code Complete, Release Candidate 1".

## Detail behind the phases (kept from the first version of this list)

### Detail: High

- [x] **Backend permissions for each way in.** The share server refuses everything not on `GUEST_COMMANDS`
  or `PERSON_COMMANDS` (`core/src/share.rs`), and nobody reaches anything before the PIN.
  `core/tests/access_classes.rs` now fails when a registered command is answered for a guest or phone without
  being on a list, when a list names a command nobody serves, when a guest command looks like a write or a
  private file, and when the phone list changes without the test being changed on purpose.
- [x] **People inside the host (owner decision 2026-11-19).** Each trusted phone has a named person (typed or picked
  when the invite is made, editable afterwards in Settings > Sharing > Rename). Roles are `core/src/permissions.rs`,
  file `user/roles.txt`: two start (**Reporter** may send reports; **Approver** may also accept or set aside the
  waiting reports), the host adds or removes roles and gives each person one. A person with none written down gets the
  first role; if the file cannot be read, nothing that needs a permission is allowed. The server checks it for every
  request (`permissions::allowed`, used by `api_call` and `device_call`). Built: backend, tests, Settings screen.
  Also built (2026-11-19, second part): a phone's browser shows **Reports to approve** (Comms menu) only when its person
  may approve; every accept or set-aside is written with the person's name (or "host") to
  `user/security/pocket-decisions.txt`; an invite can be made with **Let them type their own name**, and the phone
  asks for it when it opens the invite (a name already on another phone or already holding a role is refused, so
  a typed name can never inherit someone else's role). The larger doc 44 split (planner, watch supervisor, member,
  observer) waits until these two roles have been used. The passcode lock itself is one lock for the host's security
  records, not one per person: only the host holds the recovery code.
- [x] **Forgotten passcode.** Turning the lock on shows a one-time recovery code; Settings > Protection has "Forgot the
  passcode?" (also on the locked Security screen) and "New recovery code". The code only re-wraps the key, records are
  not touched. Locks made before this have no code until one is made while they are open. Checked in CI only (the
  Tauri side cannot be built here).
- [ ] **Vault–Hub / Pocket synchronization.** Hub side of Pocket is built (mailbox, count sheets, check-ins,
  tracks). Not built: the two-way copy between a Vault laptop and a Hub phone (inventory, gear, security logs),
  conflict rules, and the answer to "may a phone host write records" (doc 44 section 15). Needs the owner's
  decision first; doc 44 section 11 lists what must exist before it is called secure.
- [x] **Backups and interrupted writes.** Done and checked (2026-11-19): a crash leftover `.tmp` never replaces the
  real file; two saves in the same second no longer overwrite each other's backup (a real bug, fixed in
  `fsio::backup_then_write`); a cut-off last line in a log no longer swallows the next good line (fixed in
  `fsio::append`); a backup that stops half way is removed, so it is never mistaken for a good one; links are left out
  of copies. Index builds already stop, resume and refuse damaged files (doc 60 tests). Restore drill done: a whole `user/` folder with passcode-sealed files is backed up, damaged (files lost, one cut short, crash leftovers), and brought back while locked and unlocked; restoring now writes the saved bytes exactly (it would have sealed an already-sealed file a second time). Still to do: the same drill on a real Windows drive.
- [ ] **Large libraries and many devices.** Not tested with multi-gigabyte archives or several guests at
  once. The full-text index builds in pieces and can be stopped and resumed (doc 60), but its time and size on
  a real 50 GB archive are unknown.

### Detail: Medium

- [ ] **GPS and FieldLog in the main program.** Track sheets from Pocket are kept (`tracksheet.rs`); recording
  tracks and sensor logs on the host is not built (docs 45, 55).
- [ ] **Reminders and notifications.** The connecting layer (doc 37) and doc 48 have no due-date alerts yet.
- [ ] **Structured PACE plan** is not tied to free-text plans or the Security comms log (doc 50).

### Detail: not built, by design or waiting

- Android Hub, Pocket beyond the field tools, Overwatch import, plugin system (declined by the owner;
  extra content is added as `.md` files and Academy packs), garden beds screen (doc 53), signed installer.

## Checked in CI on `hub-any-drive` (2026-11-19)

Rust core tests, Windows build, Svelte type check, screenshots. Three real-kiwix jobs depend on outside
downloads and fail now and then for that reason alone.

## Docs corrected in this pass

- Doc 44: status line no longer says "implementation pending".
- Doc 45: status line no longer says "nothing is built".
