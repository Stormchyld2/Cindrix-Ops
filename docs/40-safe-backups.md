# 40. Safe backups (the phone's document vault, kept at base camp)

Cindrix Safe is the phone app that keeps family documents in an encrypted vault. It can make a backup file
(`.csbackup`). This program keeps those files safe. **It cannot open them, on purpose.**

## Where the file waits

`user/inbox/safe/` is the mailbox, same idea as the Scan inbox (docs/39). A backup arrives by:

- Safe's "Save to the Cindrix Ops inbox" (it saves to `Documents/Cindrix/inbox/safe` on the phone; when the phone
  hosts Ops, that is the same place, otherwise AirLink's receive folder points at `user/inbox/safe/`).
- Cindrix AirLink, with its receive folder set to `user/inbox/safe/`.
- Copying the file there by hand.

## What the person sees (Settings > Safe backups)

- **Waiting:** each file with its size, date and how many saved items it holds. "Keep" moves it to
  `user/safe-backups/`. "Not now" moves it to `user/inbox/safe/done/`. A file that is not a real Safe backup is
  shown with a plain reason and cannot be kept; it stays where it is.
- **Kept:** the list of kept backups. "Copy to outbox" puts a copy in `user/outbox/safe/` to send back to the phone.
  "Delete" asks once, then deletes that one file.
- Nothing is ever deleted by itself. Two files with the same name are both kept.

## What the check does (`core/src/safebackup.rs`)

Standard library only. It reads the zip's file list and the first bytes of `vault.bin`:

- Every file name must be `vault.bin` or `items/<16 hex characters>.item` or `.att`. Anything else refuses the file.
- `vault.bin` must be there and must start with the four bytes `CSV1`.
- At most 5000 entries. Names cannot contain folders that climb out, because only the names above are allowed.
- It does not decrypt or unpack anything. The files inside are sealed with a key only the Safe passphrase (or recovery code) opens.

## Host only

None of these commands is in the guest list in `share.rs`, so guests cannot reach them.

## Why Ops keeps but never opens

A backup is only as private as the passphrase. Keeping it on the drive is safe for that reason, but anyone who
copies the file can try to guess the passphrase offline, so the drive's own protection still matters. Ops never
asks for the Safe passphrase and never stores it.

## The format

The exact file format is written in Cindrix-Formats, `docs/SAFE_BACKUP_FORMAT.md`.

## Not built yet

- Moving the files without hand-carrying them (Hub Wi-Fi, AirLink receive folder set up automatically).
- Telling Safe, on the phone, that its backup arrived.
