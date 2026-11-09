# 47. Backups and data protection

Two things the audit found missing: nothing copied `user/` anywhere, and the security records sat on the drive as
plain text for anyone who found it.

## Backups (Settings > Backups)

- `core/src/userbackup.rs`: copies the `user/` folder to a place the host chooses (another drive is best), as a folder
  named by date and time. Each file is read back and compared with an FNV-1a checksum before the copy counts.
- Nothing is ever overwritten or deleted. "Bring back missing files" only adds files that are gone from `user/`.
  Replacing everything is done by hand with the program closed.
- Files that are locked stay locked in the copy. Files that are not can be read by whoever holds the copy.
- The Dashboard suggests a backup when none exists or the newest is old (`core/src/suggest.rs`, `backup_days`).

## Protection at rest (Settings > Protection)

- What it covers: everything under `user/security/` (including old copies in `security/backups`), and the device
  sign-in files `devices.txt`, `devices-log.txt`, `trusted-devices.txt` and `share-pin.txt`.
  What it does not cover: `user/inbox/` (reports from phones waiting to be accepted), `user/exports/` (the readable
  records copy the host asks for), the Family page and per-person journals (own passcode, doc 33), and the other plain
  files in `user/`. Say this to people; do not let "locked" be read as "everything".
- How: one random key seals each file (`CXL1` header, same sealing as the per-person lock). The passcode only wraps
  that key (`user/seclock.json`), so changing the passcode never touches the records. The key is in memory only.
  It is dropped by "Lock now" and when the program closes. A lock that is on starts every run closed.
- The rule in `core/src/fsio.rs`: while the lock is on and not opened, reading or writing a covered file fails with a
  plain message. It never writes plain text as a fallback and never reads an unreadable file as an empty one. Files
  written before the lock was turned on are still read as they are, so turning it on is safe at any time.
- Turning it on and off seals or opens each file only after a round trip check (`core/src/seclock.rs`). A file that
  does not check out stops the change with nothing lost.
- While locked: every `security_*` command refuses (each checks first; `core/tests/security_rules.rs` fails if one
  does not), the Security Operations screen shows only the unlock box, and the share server answers everyone with
  "The Hub is locked" (no sign-in, no PIN made up from a sealed file). Unlocking, locking or removing the lock signs
  guests out so the PIN and trusted list are read again.
- The device check now fails closed: if the device list cannot be read, the device is stopped, not let through.
- A forgotten passcode cannot be recovered. Keep a copy of the passcode, and a backup (backups of locked files
  stay locked and need the same passcode).

## Still open

- Device sign-in tokens are not hashed in the trusted list.
- Reports from phones wait in `user/inbox/pocket/` as plain files until accepted.
- No timed re-lock yet (the per-person lock has one); the records stay open until Lock now or closing the program.
- Not tested on real hardware (see the hardware test plan).
