# 48. Alerts

**BUILT (not run on real hardware).** The audit found nothing told the host when something needed a look unless the
Security screen happened to be open.

- `core/src/security_api.rs` `overview` now also returns `alerts`: one item per "needs a look" line, one per open
  incident, one for unreadable record lines and one for phone reports waiting. Each has a steady `key`, the words, and
  `urgent` (open incidents only). Tested in `security_api`.
- `src/lib/Alerts.svelte` (shown at the top of every screen on the host when the product has Security): checks every
  minute, shows a bar with what needs a look, beeps for something new (three notes for urgent, one otherwise), and puts
  the count in the window title. "Got it" hides what is shown until something new comes up. "Sound off" is remembered.
  The first check after start only fills the bar (no beep for old things). While the records are locked it shows nothing.
- Nothing leaves the computer. No push messages, no network.

## Limits (say these out loud)

- It only works while the program is open and the computer is awake. A closed program or a sleeping laptop tells
  nobody. For the hours nobody is at the screen, the watchbill and the radio plan still do the real work.
- The beep needs the sound on and a window the system lets play audio. Test it.
- Phones joining the Hub do not get these alerts (host only). Pocket will show its own, when it is built.
