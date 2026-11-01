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

## Not built yet

The way back: Ops writing its stock list for Scan (names, units, places, how many on hand) so Scan's catalog
matches Ops and shows what is already in stock.
