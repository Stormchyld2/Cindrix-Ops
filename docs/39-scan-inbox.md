# 39. Scan inbox (files waiting from the phone)

Cindrix Scan is the phone app that scans barcodes. It does not change the inventory itself. It leaves a
small file, and Ops adds it when the person says to.

## Where the file waits

`user/inbox/inventory/` is the mailbox. A file stays there as long as needed: the phone and the computer
do not have to be on at the same time. How a file gets there:

- Scan saves straight into it when the person has picked this `user` folder in Scan (phone as the host, or a shared drive).
- Or the file is sent with Cindrix AirLink and its receive folder is set to `user/inbox/inventory/`.
- Or the file is copied there by hand.

## What the person sees

Inventory shows "Waiting from your phone" when files are in the mailbox. Each can be looked at line by
line, added, or set aside ("Not now"). Nothing changes until "Add to inventory".

## What adding does (`core/src/scaninbox.rs`)

- File format: `#cindrix-inventory-import v2` (v1 files still read; all their lines are adds), then the header
  `scan_id,action,item,barcode,qty,unit,use_by,category,location,scanned_at,source`. `action` is `add` or `remove`.
- Add: a line with the same name, unit, month and place gets the quantity added. Otherwise a new item is made
  (category renamed to Inventory's names, bought month from the scan time, barcode kept in notes).
- Remove: takes from lines of that name, the one that expires first being used first. Never below zero. If there
  was less than asked for, the person is told.
- Use-by is kept as `YYYY-MM`; a day sent by Scan is dropped.
- Each line's `scan_id` is remembered in `user/inbox/applied-scan-ids.txt`, so a file that arrives twice never counts twice.
- The inventory is backed up first (`fsio::backup_then_write`). A damaged inventory is never overwritten; the file stays waiting.
- Accepted or set-aside files move to `user/inbox/inventory/done/`.

## Host only

None of these commands is allowed for guests (not in the guest list in `share.rs`).

## The way back: the stock list (`core/src/stocklist.rs`)

Every time the inventory is saved, Ops also writes `user/outbox/stock-list.csv` (`#cindrix-stock-list v1`,
columns `name,barcode,unit,location,qty,use_by,category,updated`; used-up lines left out; the barcode is the one
Scan left in the notes). Cindrix Scan reads it for its "In stock" tab and its "On hand" line. Cindrix Pocket will
read the same file. The phones never edit it.

## Who changes the inventory (owner, 2026-11-01)

Two devices may change it: the Windows computer, and the Android phone hosting the server (the Note10).
Later, when only phones are left to save power, the phone host is the only one. Scan and Pocket never change it;
Scan sends add and take-out files. Two editors need a merge so nothing is lost; the plan is in the next section.

## Merge between the two editors (planned, not built)

Each inventory line gets a stable id and a last-changed time; each change is also logged as a small delta
(add 6, take out 1) with a unique id. Merging two copies applies every delta once (like Scan's `scan_id`) and,
for edits to other fields, the newest change to a line wins. Quantities never overwrite each other.

## Not built yet

- Moving the stock list to the phones without a file picker (Hub Wi-Fi, AirLink).
- The Windows computer keeping a read-only mirror of the phone host's inventory (needs the full inventory, not just
  the stock list; to be designed with the Android host).
