# 41. Theme (charcoal and blue, 2026-11-03)

The owner asked for the same look as the other Cindrix apps (AirLink and friends): charcoal surfaces and one blue
accent, instead of the old near-black with an amber accent.

- All colours live as variables at the top of `src/app.css`. `--accent` and `--blue` are the one blue accent.
- Amber stays only where it means something: `--warn` (check, overdue, "late"), the `.check`/`.badge`/`.warn`
  pieces, map route lines and the "Transport" map colour, and `--brand`.
- **Logo branding is untouched on purpose**: the flame, the "Cindrix Ops" wordmark colour (`--brand`), the splash
  screen (`src/lib/Splash.svelte`, `public/cindrix-logo.png`, `public/splash-bg.jpg`) and the app icon.
- The group colours in the top bar (`--cat-*`) are unchanged.
- The mockups in `mockups/` still show the old amber look; the screens now follow this page, except `mockups/SecurityCommand.html`, which already uses the charcoal-and-blue theme.

## Touch-ups taken from the Security Operations mockup (2026-11-08)

The owner liked the look of `mockups/SecurityCommand.html` and asked for the rest of the program to follow it. Only
small, theme-level changes were made; no layout or behaviour changed.

- **Tabs.** Rows of tabs are bordered buttons: mono capitals, blue-tinted with a blue underline mark when selected.
  Applied to Music's subtabs, the Seed vault tabs and the Projects tabs (before: a bare underline, or a round pill).
  Security Operations' six subtabs use the same look and add a small icon and a colour mark per tab. Settings' side
  list is a menu, not a tab row, and is unchanged.
- **Dashboard tiles.** Each tile takes a faint tint of its own colour from the left stripe (`--c`).
- **One set of status colours.** `src/app.css` now has `--ok-bg/-line/-text` and `--danger-bg/-line/-text`, next to the
  existing `--warn*`. About 40 hard-coded greens, reds and ambers in the screens (Ask, Dashboard, Plans, Start,
  Inventory, Seed vault, Reader, Settings, Archives, Sharing, Binder, Training, Safe backups, Shelf) now use them, so
  every "done", "problem" and "check" box looks the same. Two leftovers of the old warm-brown look are gone: the
  `.warn` and `.badge` boxes in `app.css`, and the chat bubble for your own messages in Ask (now blue-tinted).
- **Left alone on purpose.** The Maps screen's own colours (map layers, markers, route lines, on-map overlays), the
  Intel browser's Chrome-like bar, the Binder's white printable sheet, the Shelf's book-cover colours, and the logo,
  flame and splash screen. These are content or branding, not interface theme.
- **Already matched.** The body text font, the faint grid background, the header, panels, headings and buttons.
- **Still to do when Security Operations is built:** the posture banner glow, the coloured chips and the watch-strip
  styling exist only in the mockup so far.
