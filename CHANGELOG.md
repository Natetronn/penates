# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[Semantic Versioning](https://semver.org/) (the same scheme Cargo and Tauri use).

## [Unreleased]

### Added
- In app windows, Alt+Home (Cmd+Shift+H on macOS) goes to the app's start page, and F1 or Ctrl+/ (Cmd+/) shows a list of the keys. A site that uses one of these keys itself keeps it.
- The website, [getpenates.com](https://getpenates.com), with the docs, the app catalog and a clickable demo of the manager. About links to it.
- 12 more apps in the app catalog, 22 in all: ChatGPT, Claude, Figma, Gmail, Google Calendar, Google Drive, Linear, Outlook, Slack, Telegram, Todoist and Trello. The catalog is live for every install; this release refreshes the built-in copy used when it cannot be reached.

- An Arch Linux package on each release, `penates-bin-<version>-1-x86_64.pkg.tar.zst`, installed with `sudo pacman -U` (no AUR helper needed). It was also added to the 0.1.1 release.

### Changed
- Creating an app uses its catalog preset's icon address first, as updating already did, so sites that refuse to be fetched (such as Claude and Cloudflare) still get an icon.
- The update notice for package-manager installs no longer assumes an AUR helper: it says to update the way Penates was installed (the package manager, or makepkg or an AUR helper for the AUR).

## [0.1.1] - 2026-11-15
Share apps with the catalog, start apps in the tray, browser keys in app windows, Credits with full license texts, and a round of fixes.

### Added
- Share an app with the app catalog: ⋮ → **Share with the app catalog…** (or `penates submit <id> --notes "…"`) shows the entry for review and opens GitHub with it filled in, as a new file for a new site or an issue suggesting a change for a site already in the catalog. Custom JavaScript is never included, and Penates sends nothing itself.
- "When it starts" per app (Edit → Window): open the window, start minimized, or start only in the tray. Without a tray icon, "only in the tray" starts minimized so the app is never unreachable.
- App windows have browser-style navigation: Alt+Left/Right (Cmd+[ and ] on macOS) and the mouse's back and forward buttons go back and forward; F5 and Ctrl+R reload. A page that uses one of these keys itself keeps it.
- Popular apps show each site's real icon, fetched once and cached for 30 days (`~/.cache/penates/catalog-icons/`, safe to delete).
- Credits in the About dialog: who made Penates, what it is built on, every open-source component with its license, and the full license texts. Every package carries `THIRD-PARTY-LICENSES.txt`; the AppImage also carries `SYSTEM-LIBRARIES.txt`, the license notices and source references of the system libraries it bundles.
- A second app for the same site gets a numbered name ("GitHub 2") when its name comes from the site, so menus and taskbars tell them apart; a name you type is kept as typed. Edit shows each app's id.
- Development builds show their commit next to the version (`0.1.1+8b4141a`) in the manager and `penates --version`; release builds show the plain version.
- AUR package `penates`, built from the tagged source (beside `penates-bin`, the prebuilt release); x86_64 and aarch64.

### Changed
- Updates are in one place: the Updates tab has a Penates section and a Your apps section, each with its own check, and Update all apps under Your apps. Settings → Updates holds only settings.
- App list: every row has Open, Edit and ⋮ in the same places. Update is in the ⋮ menu, and an "update available" pill (linking to Updates) shows when an app has a newer preset. Double-click an app's icon or name to open it.

### Fixed
- Choosing "System" in the manager's Light or dark setting forced light on Linux after picking Light or Dark; it follows the desktop (read from the XDG settings portal).
- Favicons in ICO files whose largest image is a grayscale or palette PNG (Tailscale's, for one) were rejected; Penates reads the file's images one by one.
- After installing from the tarball, KDE's menu shows the Penates icon right away.
- Import's file chooser showed no `.penates` files on Linux; it shows all files and checks the one you pick.
- Apps opened from the manager lingered as defunct processes after you closed them.
- The CLI panicked when its output was cut short, e.g. `penates check | head -1`; it exits quietly like other command line tools.

## [0.1.0] - 2026-11-15
First release. Verified by hand on Linux (KDE Plasma, Wayland); Linux, Windows and macOS pass CI smoke tests.

### Highlights
- Turn websites into desktop apps with their own window, icon, taskbar entry, tray icon and isolated login.
- A command line tool (`penates`) and a desktop manager (`penates-desktop`) built on the same core, so an app made in one behaves the same in the other.
- In-app updates for AppImage, .deb, .rpm, Windows and macOS installs; AUR users update with their package manager.

### Features
- **Create**: `penates create github.com` or the manager's Create tab. The name comes from the site (`og:site_name`, `application-name`, PWA manifest, or the part of `<title>` that names the site) and the icon from its PWA manifest, apple-touch icon or favicon; both can be changed. Names in any alphabet work.
- **Launchers**: a `.desktop` entry on Linux, an `.app` bundle on macOS and a Start Menu shortcut on Windows, each with the app's own icon and taskbar identity. They are rewritten automatically after Penates updates or moves.
- **Per-app options**: window size, zoom, start maximized or fullscreen, resizable, always on top, frameless, devtools, user agent, extra in-app sites, custom CSS and JavaScript, window theme.
- **Isolated profiles**: each app keeps its own cookies and storage.
- **System tray**: optional, in color or grayscale, with Show / Hide / Quit and close-to-tray.
- **Single-window mode**: launching an app that is already open focuses its window. The manager is single-window too.
- **Links and files**: clicking a link to another site opens your browser; the site's own pages (with or without `www.`), its sign-in popups and sites it redirects to stay in the app; frames from other sites (videos, checkouts, captchas) load normally; downloads go to your Downloads folder without overwriting.
- **Site permissions** (Linux): per-app "Allow notifications" (on by default) and "Allow camera and microphone" (off by default).
- **Cache and data**: clear the cache (keeps you signed in) or clear cookies and data.
- **Icons**: picked automatically, or chosen from a file or URL; Penates keeps its own copy in `~/.config/penates/icons/`.
- **Export, import and backup**: one app or all of them to a `.penates` file (configs and icons, never cookies or logins); imports show what they would change before applying.
- **Popular apps**: browse the app catalog under Create and add a ready-made app in one click (`penates catalog` lists it on the command line).
- **Updates**: Penates checks for new versions of itself, and syncs small per-site fixes (presets) into your apps from the shared [penates-apps](https://github.com/Natetronn/penates-apps) catalog (GitHub, YouTube, YouTube Music, Notion, Proton Mail, Discord, WhatsApp, Spotify, Cloudflare, Tailscale), with a built-in copy for offline use. Both checks are in Settings → Updates.
- **Manager**: Apps, Create, Updates and Settings tabs, an About dialog, light/dark/system, six accents and four looks (Penates, Nord, Solarized, High contrast), all from plain CSS tokens, keyboard friendly. Configs that no longer load are listed and can be removed.
- **Packages**: AppImage, `.deb`, `.rpm`, Windows installer, macOS `.dmg`, an AUR package (`penates-bin`) and a portable tarball with an install script.

### Security
- Updates of Penates are signed and verified before they install.
- Wrapped sites get no Penates or Tauri API; the manager's page only runs its own scripts (content security policy).
- App ids are validated wherever they become file or folder names, so an id such as `..` cannot reach a path outside Penates's folders.
- Links a site hands to the system are limited to `http`, `https`, `mailto` and `tel`, at most one per second; `file:` and custom schemes are dropped.
- "Allow camera and microphone" never grants screen capture.
- Imported `.penates` files never bring custom JavaScript unless you pass `--allow-js`, always arrive with camera, microphone and developer tools switched off, list the extra sites they keep in the window, and only change settings with `--settings`. Preset catalogs can never add JavaScript.
- Icons are decoded with size limits; app names lose control characters before they reach launcher files.

### Known limitations
- Per-app window theme cannot force light or dark for sites on Linux (WebKitGTK follows the desktop). It should work on Windows and macOS.
- Site permissions (notifications, camera and microphone) are enforced on Linux only; Windows and macOS use their web view's own prompts.
- macOS: clearing cookies, data or cache is not supported yet (it says so instead of pretending); Penates needs macOS 14 or later.
- Windows: the Start Menu shortcut does not carry the app's AppUserModelID yet, so pinning a running app to the taskbar pins the manager. A passive update closes running apps.
- macOS and Windows installers are not signed with a platform certificate yet (see the README for the first launch).
- Windows and macOS are smoke-tested in CI (launcher, launch, manager, export/import) but have not been checked by hand.
- A page's own script can still navigate the window to another site (only link clicks are sent to the browser).
- Two apps whose ids hash to the same loopback port lose single-window mode and the "is it running" check (rare).

[Unreleased]: https://github.com/Natetronn/penates/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/Natetronn/penates/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Natetronn/penates/releases/tag/v0.1.0
