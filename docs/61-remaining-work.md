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
