# 63. Restore drill on a real drive

The program's own tests prove the restore works on a test folder (`core/src/userbackup.rs`, `core/src/fsio.rs`). This drill proves it on the
real Windows drive with the real files. It takes about ten minutes and changes nothing you care about if you follow it.

**You need:** the program running from the real drive, a second drive or USB stick for the backup copy, and a few minutes.

1. Settings > Backups: choose the second drive as the place for copies. Press **Back up now**. Expect: "Backed up" with a file count, no red message.
2. Press **Check it** on the new copy. Expect: every file reads back and matches.
3. Open the drive in Windows. In `user\`, make a note of the size of `inventory.csv` and `gear.txt`. Then **move** (do not delete) `gear.txt` to the desktop.
4. Back in Backups, press **Bring back missing files** on the copy. Expect: one file brought back, none overwritten.
5. Open Gear. Expect: the same list as before. Compare `gear.txt` on the drive with the one on the desktop: they should be identical.
6. If the passcode lock is on: lock, press Back up now (it should tell you to unlock), unlock, then repeat steps 1 and 2. Expect the copy to read back fine after the lock is off or on.
7. Pull the stick out in the middle of step 1 once, on purpose. Run Back up now again. Expect: the half-made copy is not listed as a good copy, and the next copy is complete.
8. Write the date and what happened at the bottom of this page (or in doc 61 under phase 7). Any message that is not plain words, or any file that is wrong, is a bug: copy the message exactly.

**Result:** not run yet.
