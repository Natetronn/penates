# Theming

There are three independent layers.

## 1. Manager look (Settings tab)
Saved in `settings.toml` under `[appearance]` and applied instantly by setting two attributes on `<html>`: `data-theme` and `data-accent` (plus `data-skin`).

- **Light or dark**: `system` follows the OS (`prefers-color-scheme`), or force `light` / `dark`.
- **Accent**: `blue`, `violet`, `green`, `orange`, `rose`, `slate`. Each has a light and a dark value.
- **Look (skin)**: `default`, `nord`, `solarized`, `mono` (high contrast, square corners).

## 2. Tokens and skins (plain CSS)
- `ui/tokens.css` is the single source of truth: fonts, radii, shadow, surfaces (`--bg --card --line --chip --fg --muted`), status colors, and the accent palette. Light is the default; dark applies for the OS dark setting or `data-theme="dark"`, and `data-theme="light"` forces light.
- `ui/skins.css` layers named token sets over the base, each defining light and dark values. A skin selector works on any element, so the Settings preview cards show real skin colors.
- **Add a skin**: copy a block in `skins.css`, change `data-skin="<name>"` and the colors, add the name to `SKINS` in `ui/index.html` and to the allow-list in `set_appearance` (`src-tauri/src/main.rs`).
- **Add an accent**: add a `[data-accent="<name>"]` line to `tokens.css` and the name to `ACCENTS` and the allow-list.

## 3. Per-app window theme
`theme = "system" | "light" | "dark"` in an app's config (Edit → Window). It sets the native window theme and what the site reports through `prefers-color-scheme`.

**Platform note:** this works through Tauri's window theme API, which is honored on Windows and macOS. On Linux, WebKitGTK takes light or dark from the desktop (KDE reports it through the settings portal), so the setting does not change what a site sees. This was tested: a probe page reported dark with the app set to light, with and without a GTK theme override.
