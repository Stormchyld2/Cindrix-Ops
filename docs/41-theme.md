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
  Applied to Music's subtabs and the Seed vault tabs (before: a bare underline, and a round pill). Security Operations'
  six subtabs use the same look and add a small icon and a colour mark per tab. Projects' document tabs and Settings'
  side list are not tab rows and are unchanged.
- **Dashboard tiles.** Each tile takes a faint tint of its own colour from the left stripe (`--c`).
- **Not changed.** The body text font, the faint grid background, the header, panels, headings and buttons already
  matched the mockup.
- **Still to do when Security Operations is built:** the posture banner glow, the coloured chips and the watch-strip
  styling exist only in the mockup so far.
