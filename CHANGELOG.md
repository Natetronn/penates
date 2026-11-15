# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[Semantic Versioning](https://semver.org/) (the same scheme Cargo and Tauri use).

## [Unreleased]

## [0.1.0] - 2026-11-15
First release. Verified by hand on Linux (KDE Plasma, Wayland); Windows and macOS pass CI smoke tests.

### Highlights
- Turn websites into desktop apps with their own window, icon, taskbar entry, tray icon and isolated login.
- A command line tool (`penates`) and a desktop manager (`penates-desktop`) that do the same things.
- In-app updates for AppImage, .deb, .rpm, Windows and macOS installs; AUR users update with their package manager.

### Features
- **Create**: `penates create github.com` or the manager's Create tab. The name comes from the site (`og:site_name`, `application-name`, PWA manifest, `<title>`) and the icon from its PWA manifest, apple-touch icon or favicon; both can be changed.
- **Launchers**: a `.desktop` entry on Linux, an `.app` bundle on macOS and a Start Menu shortcut on Windows, each with the app's own icon and taskbar identity.
- **Per-app options**: window size, zoom, start maximized or fullscreen, resizable, always on top, frameless, hide menu bar, devtools, user agent, extra in-app sites, custom CSS and JavaScript, window theme.
- **Isolated profiles**: each app keeps its own cookies and storage.
- **System tray**: optional, in color or grayscale, with Show / Hide / Quit and close-to-tray.
- **Single-window mode**: launching an app that is already open focuses its window.
- **Links and files**: clicking a link to another site opens your browser; same-site popups (sign-in flows) open in an app window with the same rules; frames from other sites (videos, checkouts, captchas) load normally; downloads go to your Downloads folder without overwriting.
- **Site permissions** (Linux): per-app "Allow notifications" (on by default) and "Allow camera and microphone" (off by default).
- **Cache and data**: clear the cache (keeps you signed in) or clear cookies and data.
- **Icons**: picked automatically, or chosen from a file or URL; Penates keeps its own copy in `~/.config/penates/icons/`.
- **Export, import and backup**: one app or all of them to a `.penates` file (configs and icons, never cookies or logins); imports show what they would change before applying.
- **Updates**: Penates checks for new versions of itself, and syncs small per-site fixes (presets) into your apps from the shared [penates-apps](https://github.com/Natetronn/penates-apps) catalog (GitHub, YouTube, YouTube Music, Notion, Proton Mail, Discord, WhatsApp, Spotify, Cloudflare, Tailscale), with a built-in copy for offline use. Both checks are in Settings → Updates.
- **Manager**: Apps, Create, Updates and Settings tabs, an About dialog, light/dark/system, six accents and four looks (Penates, Nord, Solarized, High contrast), all from plain CSS tokens.
- **Packages**: AppImage, `.deb`, `.rpm`, Windows installer, macOS `.dmg`, an AUR package (`penates-bin`) and a portable tarball with an install script.

### Security
- Updates of Penates are signed and verified before they install.
- App ids are validated wherever they become file or folder names, so an id such as `..` cannot reach a path outside Penates's folders.
- Links a site hands to the system are limited to `http`, `https`, `mailto` and `tel`, at most one per second; `file:` and custom schemes are dropped.
- Imported `.penates` files never bring custom JavaScript unless you pass `--allow-js`, and only change settings with `--settings`. Preset catalogs can never add JavaScript.
- App names lose control characters before they reach launcher files.

### Known limitations
- Per-app window theme cannot force light or dark for sites on Linux (WebKitGTK follows the desktop). It should work on Windows and macOS.
- Site permissions (notifications, camera and microphone) are enforced on Linux only; Windows and macOS use their web view's own prompts.
- macOS: clearing cookies, data or cache is not supported yet (it says so instead of pretending); Penates needs macOS 14 or later.
- Windows: the Start Menu shortcut does not carry the app's AppUserModelID yet, so pinning a running app to the taskbar pins the manager. A passive update closes running apps.
- Windows and macOS are smoke-tested in CI (launcher, launch, manager, export/import) but have not been checked by hand.
- A page's own script can still navigate the window to another site (only link clicks are sent to the browser).
- Two apps whose ids hash to the same loopback port lose single-window mode and the "is it running" check (rare).

