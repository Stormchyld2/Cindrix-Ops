# 61. Remaining work (checklist)

Written 2026-11-19 from the owner's roadmap, after reading the architecture docs against the source.
Nothing below has been run on real hardware. "Checked" means a test in `core/` or a CI job covers it.
Doc 57 is the short status page; this is the working list. Update both when something closes.

## High

- [x] **Backend permissions for each way in.** The share server refuses everything not on `GUEST_COMMANDS`
  or `PERSON_COMMANDS` (`core/src/share.rs`), and nobody reaches anything before the PIN.
  `core/tests/access_classes.rs` now fails when a registered command is answered for a guest or phone without
  being on a list, when a list names a command nobody serves, when a guest command looks like a write or a
  private file, and when the phone list changes without the test being changed on purpose.
- [ ] **People inside the host (doc 44 section 11 roles).** There is still only the host, a guest and a trusted
  phone, no named people with roles (planner, watch supervisor, member, observer). Open decision, doc 44
  section 15: needs an identity per person before roles can be enforced. Not built.
- [ ] **Vault–Hub / Pocket synchronization.** Hub side of Pocket is built (mailbox, count sheets, check-ins,
  tracks). Not built: the two-way copy between a Vault laptop and a Hub phone (inventory, gear, security logs),
  conflict rules, and the answer to "may a phone host write records" (doc 44 section 15). Needs the owner's
  decision first; doc 44 section 11 lists what must exist before it is called secure.
- [~] **Backups and interrupted writes.** Done and checked (2026-11-19): a crash leftover `.tmp` never replaces the
  real file; two saves in the same second no longer overwrite each other's backup (a real bug, fixed in
  `fsio::backup_then_write`); a cut-off last line in a log no longer swallows the next good line (fixed in
  `fsio::append`); a backup that stops half way is removed, so it is never mistaken for a good one; links are left out
  of copies. Index builds already stop, resume and refuse damaged files (doc 60 tests). Still to do: a restore drill
  of the whole `user/` folder after a simulated crash, and the sealed (passcode) files under the same cuts.
- [ ] **Large libraries and many devices.** Not tested with multi-gigabyte archives or several guests at
  once. The full-text index builds in pieces and can be stopped and resumed (doc 60), but its time and size on
  a real 50 GB archive are unknown.

## Medium

- [ ] **GPS and FieldLog in the main program.** Track sheets from Pocket are kept (`tracksheet.rs`); recording
  tracks and sensor logs on the host is not built (docs 45, 55).
- [ ] **Reminders and notifications.** The connecting layer (doc 37) and doc 48 have no due-date alerts yet.
- [ ] **Structured PACE plan** is not tied to free-text plans or the Security comms log (doc 50).

## Not built, by design or waiting

- Android Hub, Pocket beyond the field tools, Overwatch import, plugin system (declined by the owner;
  extra content is added as `.md` files and Academy packs), garden beds screen (doc 53), signed installer.

## Checked in CI on `hub-any-drive` (2026-11-19)

Rust core tests, Windows build, Svelte type check, screenshots. Three real-kiwix jobs depend on outside
downloads and fail now and then for that reason alone.

## Docs corrected in this pass

- Doc 44: status line no longer says "implementation pending".
- Doc 45: status line no longer says "nothing is built".
