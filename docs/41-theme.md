# 41. Theme (charcoal and blue, 2026-11-03)

The owner asked for the same look as the other Cindrix apps (AirLink and friends): charcoal surfaces and one blue
accent, instead of the old near-black with an amber accent.

- All colours live as variables at the top of `src/app.css`. `--accent` and `--blue` are the one blue accent.
- Amber stays only where it means something: `--warn` (check, overdue, "late"), the `.check`/`.badge`/`.warn`
  pieces, map route lines and the "Transport" map colour, and `--brand`.
- **Logo branding is untouched on purpose**: the flame, the "Cindrix Ops" wordmark colour (`--brand`), the splash
  screen (`src/lib/Splash.svelte`, `public/cindrix-logo.png`, `public/splash-bg.jpg`) and the app icon.
- The group colours in the top bar (`--cat-*`) are unchanged.
- The mockups in `mockups/` still show the old amber look; the screens now follow this page.
