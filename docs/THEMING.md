# Theming

How Penates looks comes from four independent layers: the manager's look, its tokens and skins, each app window's theme, and the toolbar's color.

## 1. Manager look (Settings tab)
Saved in `settings.toml` under `[appearance]` and applied by setting three attributes on `<html>`:
`data-theme`, `data-accent` and `data-skin`. The manager passes the saved look in before the page loads
(`ui/boot.js` applies it), so the window never flashes the default theme first.

- **Light or dark**: `system` follows the OS (`prefers-color-scheme`), or force `light` / `dark`.
- **Accent**: `violet` (default, from the logo), `blue`, `green`, `orange`, `rose`, `slate`. Each has a light and a dark value, chosen so text in the accent color and the label on an accent button both keep at least 4.5:1 contrast.
- **Look (skin)**: `default`, `nord`, `solarized`, `mono` (high contrast, square corners).

## 2. Tokens and skins (plain CSS)
- `ui/tokens.css` is the single source of truth: fonts, radii, shadow, surfaces (`--bg --card --line --chip --fg --muted`), status colors, and the accent palette. Light is the default; dark applies for the OS dark setting or `data-theme="dark"`, and `data-theme="light"` forces light. Colors, radii and shadows all come from here; looks come from `skins.css`.
- `ui/skins.css` layers named token sets over the base, each defining light and dark values. A skin selector works on any element, so the Settings preview cards show real skin colors.
- **Add a skin**: copy a block in `skins.css`, change `data-skin="<name>"` and the colors, and add the name to `SKINS` in `ui/app.js` and in `crates/penates-core/src/settings.rs`.
- **Add an accent**: add a `[data-accent="<name>"]` line to `tokens.css` and the name to `ACCENTS` in `ui/app.js` and `settings.rs`.

## 3. Per-app window theme
`theme = "system" | "light" | "dark"` in an app's config (Edit → Window). It sets the native window theme and what the site reports through `prefers-color-scheme`.

**Platform note:** this works through Tauri's window theme API, which is honored on Windows and macOS. On Linux, WebKitGTK takes light or dark from the desktop (KDE reports it through the settings portal), so the setting does not change what a site sees. This was tested: a probe page reported dark with the app set to light, with and without a GTK theme override.

## 4. Toolbar color
Settings → Appearance → **Toolbar color** sets the default for every app with a toolbar: **Match each site** or
**Penates colors**. Each app can choose its own under Edit → Window: the default, the site's, Penates's, or a
custom color (a color picker, or any CSS color typed in: hex, `rgb()`, `hsl()`, a name, `oklch()`; it is saved
as `#rrggbb`).

Matching a site uses its `theme-color` (the one for light or dark mode, whichever applies), else the page's
background, else Penates's colors, and follows changes while the app is open. Icons and text turn light or dark
by contrast (WCAG), so the toolbar stays readable on any color.

