# Vault server on an old Fire tablet (plan, not tested)

Idea: the 4 TB Vault drive plugged into an old Amazon Fire tablet, which serves the ZIM archives
over Wi-Fi to phones and laptops. Notes from research; nothing here has been tried on real hardware.

- **What runs there:** the Kiwix Android app (it can host its library to other devices on the
  network), installed from Google Play, which is already working on the tablet. The full Cindrix
  program and the AI model will not run on an old Fire tablet; it is only the archive server.
- **Drive format:** the tablet must read the drive. Fire TV devices have been FAT32-only on USB;
  Fire tablets are unconfirmed. FAT32 cannot hold a file over 4 GB, so large ZIMs would not fit.
  Test with the real drive before relying on this.
- **Power:** a 4 TB portable drive can draw more than the tablet's port supplies. Expect to need a
  powered USB hub or Y-cable, and charging the tablet at the same time is awkward on many models.
- **Hotspot:** Fire OS has no built-in hotspot. The safe route is a small travel router/access point
  on a power bank: the tablet and phones all join its Wi-Fi, no internet needed.
- **Later:** a "use a remote archive server" setting in Cindrix, so the assistant on a PC can search
  the archives served by the tablet (the program already talks to Kiwix over a local web address).
