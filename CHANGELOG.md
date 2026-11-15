# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[Semantic Versioning](https://semver.org/) (the same scheme Cargo and Tauri use).

## [Unreleased]

## [0.1.0] - 2026-11-14
First release. Verified on Linux (KDE Plasma, Wayland); Windows and macOS pass CI smoke tests.

### Highlights
- Turn websites into desktop apps with their own window, icon, taskbar entry, tray icon and isolated login.
- In-app updates for AppImage, .deb, .rpm, Windows and macOS installs; AUR users update with their package manager.

### Security
- App ids are validated wherever they become file or folder names; a bad id such as `..` can no longer reach a path (previously `remove --purge` could have deleted the wrong folder).
- Links a site sends to the OS are limited to `http`, `https`, `mailto` and `tel`; `file:` and custom schemes are dropped instead of handed to `xdg-open`.
- Update presets no longer apply `inject_js`, so a manifest URL cannot push script into apps.
- App names lose control characters before reaching launcher files.

### Fixed
- `penates create` and the manager's Create now validate like Edit (zoom, size and theme are clamped; non-http URLs are rejected).
- "Remove app and data" now asks for a second click, like "Clear cookies & data".
- Release workflow: Intel macOS is cross-compiled on the Apple Silicon runner (the `macos-13` runner is retired).

### Added
- **Site permissions** (Linux): per-app "Allow notifications" (on by default) and "Allow camera and microphone" (off by default). Previously WebKitGTK denied both outright. macOS bundles now carry camera/microphone usage descriptions.
- **Smoke tests in CI** that launch the real apps on Windows and macOS through their launchers.
- **Export, import and backup**: `penates export <id>... | --all [--settings] [-o file]` writes a `.penates` file (configs and icons, never cookies or logins); `penates import <file-or-url>` reviews it and recreates the apps. Conflicts: skip, rename (keep both) or overwrite. Custom JavaScript is stripped unless `--allow-js`; settings (including the update source) apply only with `--settings`; ids and URLs from a file are validated. The manager has Import, Export all, per-app Export and a Backup section in Settings.

### Added
- **Core library** (`penates-core`): TOML app config in `~/.config/penates/apps/`, per-app isolated profiles, launcher integration for Linux (`.desktop`), macOS (`.app` bundle) and Windows (Start Menu shortcut).
- **CLI** (`penates`): `create`, `list`, `show`, `path`, `run`, `edit`, `install`, `icon`, `icons`, `clear`, `remove`, `check`, `update`, `gui`.
- **Desktop app** (`penates-desktop`, Tauri 2): manager window with Apps, Create, Updates and Settings tabs, and a per-app runner (`--run <id>`).
- **Per-app options**: window size, zoom, start maximized/fullscreen, resizable, always on top, frameless, hide menu bar, devtools, user agent, extra in-app hosts, custom CSS/JS, window theme.
- **Icons**: found automatically (PWA manifest, apple-touch, `link rel=icon`, favicon), or chosen from a file or URL; stored as our own 256px PNG copy in `~/.config/penates/icons/` and referenced by file name.
- **Names**: read from the site (`og:site_name`, `application-name`, PWA manifest, `<title>`), editable.
- **System tray** with color or grayscale icon and a Show / Hide / Quit menu; optional close-to-tray.
- **Single-window mode**: relaunching an app focuses the open window.
- **Links and files**: other sites open in the system browser, same-site popups stay in-app, downloads save to the Downloads folder without overwriting.
- **Cache and data**: clear cache (keeps logins) or clear cookies and data; refused while the app is running.
- **Updates**: one JSON manifest (semver) for Penates releases and a catalog of per-site presets synced into apps; bundled fake sample manifest until a real URL exists.
- **Theming**: `ui/tokens.css` and `ui/skins.css`; light/dark/system, six accents, four looks (Penates, Nord, Solarized, High contrast).
- **Logo**: globe in a rounded app tile.
- **Packaging**: `.deb`, `.rpm`, `.AppImage`, Arch `PKGBUILD`, portable tarball with installer; CI and release workflows (Windows and macOS builds run in CI only).
- **Docs**: README and `docs/`.

### Fixed during development
- Frames embedded from other sites (videos, checkouts, captchas) were blocked and opened as browser tabs on page load; they now load, and only real clicks on links to other sites go to the browser.
- Same-site popups opened without the app's link, download and popup rules; they now get the same rules as the main window.
- A link-opening page could flood the desktop with browser tabs; hand-offs to the OS are limited to one per second.
- Choosing an icon in Edit discarded unsaved changes to the other fields.
- `penates edit` did not validate the edited config, and a bad URL crashed the app at launch; broken configs are now reported.
- Long names produced an invalid id; macOS and Windows launchers of two apps with the same name overwrote each other.
- After a Penates update, macOS app bundles kept the old binary; launchers are now rewritten after every update.
- A second launch during a slow start could open a second window.
- Releases now refuse to build when the tag does not match the version in Cargo.toml.
- macOS build failed with a duplicate `_EMBED_INFO_PLIST` symbol because `generate_context!` was expanded twice (found by CI on the first push).
- App list and Updates tab printed `[object HTMLDivElement]` (rows passed as an array).
- Yellow fallback icon in the taskbar and title bar: launcher file name now matches the window's app ID, and the launcher cache is refreshed when launchers are written.
- Selected state missing on theme buttons and swatches (ARIA attributes set as properties).
- Create tab layout overlap, Edit field alignment, checkbox colors.
- Linux tray: Tauri emits no click events there, so a click opens the menu.
- AppImage launchers pointed at a temporary mount path; they now use the AppImage file.
- Bundle category rejected by the Tauri bundler.

### Known limitations
- Per-app window theme cannot force light or dark for sites on Linux (WebKitGTK follows the desktop). It should work on Windows and macOS.
- Site permissions (notifications, camera and microphone) are enforced on Linux only; Windows and macOS use their web view's own prompts.
- macOS: clearing cookies, data or cache is not supported yet (it says so instead of pretending); Penates needs macOS 14 or later.
- Windows: the Start Menu shortcut does not carry the app's AppUserModelID yet, so pinning a running app to the taskbar pins the manager. A passive update closes running apps.
- Windows and macOS are smoke-tested in CI (launcher, launch, manager, export/import) but have not been checked by hand.
- A page's own script can still navigate the window to another site (only link clicks are sent to the browser).
- Two apps whose ids hash to the same loopback port lose single-window mode and the "is it running" check (rare).

