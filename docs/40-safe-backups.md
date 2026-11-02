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

## The receipt (so the phone knows)

When a backup is kept, Ops adds a line to `user/outbox/safe/received.csv` (`#cindrix-safe-receipts v1`, columns
`name,size,kept_at` with `kept_at` in Unix seconds; newest last; the last 200 kept). A failed or set-aside file gets no
line. Cindrix Safe reads this file to tell the person "Ops has your backup" (it matches the file name and size it wrote).
If the phone and the drive share a folder, that is all it takes; otherwise copy or send the file back by hand.

## Host only

None of these commands is in the guest list in `share.rs`, so guests cannot reach them.

## Why Ops keeps but never opens

A backup is only as private as the passphrase. Keeping it on the drive is safe for that reason, but anyone who
copies the file can try to guess the passphrase offline, so the drive's own protection still matters. Ops never
asks for the Safe passphrase and never stores it.

## The format

The exact file format is written in Cindrix-Formats, `docs/SAFE_BACKUP_FORMAT.md`.

## Linking the phone to the folder (Safe 0.8.0)

Safe can be linked once to the folder that holds `inbox/` and `outbox/` (the drive's `user` folder when the phone hosts Ops,
or a Cindrix folder that AirLink or a shared drive keeps in step). After that, "Save to the linked folder" writes straight
to `inbox/safe/` and "Check if Ops has it" reads `outbox/safe/received.csv`. Safe asks for access to that one folder only.

## Not built yet

- Carrying the folders between a phone and a computer that are not sharing storage (AirLink's receive folder and Hub Wi-Fi
  still need their own work; the files themselves are ready for either).
