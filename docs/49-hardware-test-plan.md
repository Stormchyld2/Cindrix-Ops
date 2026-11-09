# 49. Real-hardware test plan

Nothing in Hub, the share server, the erase order, the lock, backups or alerts has run on real hardware. CI proves the
logic; it cannot prove a hotspot, a drive or a phone. This page puts every real test in one order, with what to write
down. The older lists stay where they are: doc 22 (first Windows run), doc 34 (hotspot and sharing), doc 24 (Android).

Write yes/no for each line and copy any red text. Do Part A first; nothing later is worth trying if it fails.

## A. Windows host (laptop)
1. Doc 22 section 4, all twelve checks.
2. Doc 34 "To test", all six steps (hotspot, PIN, guest refusal, wrong PIN wait).
3. Close the program while sharing is on, start it again: sharing is off at start (it must be).

## B. Security records and the lock (new)
1. Security Operations: make a site, set a posture, add a watch, open and close an exercise incident. Close the
   program, open it: everything is still there.
2. Settings > Protection: turn the lock on with a passcode. Open `user/security/` in Explorer with Notepad: the files
   are unreadable. Security Operations still works (you are unlocked).
3. Press Lock now. Security Operations shows only the unlock box. A phone on the hotspot gets "The Hub is locked".
4. Wrong passcode: plain message. Right passcode: everything is back, the PIN is the same as before.
5. Close and reopen the program: it starts locked. Unlock, then take the lock off: the files are plain again.
6. Pull the drive during step 2 once (after pressing Turn the lock on, before it finishes): reopen, unlock, and check
   nothing is missing. (The lock round-trips each file before replacing it.)

## C. Phones and trusted devices (docs 34, 45)
1. Make an invite, join from a phone, check it appears in Settings > Devices.
2. Order that device erased; the phone must show only the erase notice. Cancel the order on a second phone.
3. Lock the records; the trusted phone is refused; unlock; it works again without re-joining.
4. Pocket (when built): send a check-in sheet and a count sheet; both wait in Command until accepted.

## D. Backups
1. Settings > Backups: choose a folder on another drive, Back up now, then Check it.
2. Delete one file from `user/` (move it away), Bring back missing files: only that file returns.
3. Unplug the backup drive: the screen says it cannot be reached; the Dashboard reminder shows when the copy is old.

## E. Alerts
1. With a site with no posture set: a bar appears within a minute, count in the title.
2. Open an exercise incident: three-note beep. Got it hides the bar; a new look brings it back.
3. Lock the records: the bar goes.

## F. Archives, assistant, maps on the host
Doc 22 items 4 to 10 cover these. Also: time to answer on a cold drive, memory in Task Manager.

## G. Android Hub (when started; doc 24 list)
Large ZIM from exFAT, hotspot plus power, assistant size and speed, serving several readers. Archives without
`kiwix-serve` on Android depend on the direct reader (see doc 24 and the archive notes).

## What to send back
For each failure: the line number, what you saw, a photo or the red text, and the Windows version. Do not fix the
drive's files by hand; copy `user/` somewhere first.
