# Status

What has actually been run, and where. Last updated 2026-11-14, version 0.1.0.

| Platform | Build + unit tests | App launched | Checked by hand |
|---|---|---|---|
| Linux (KDE Plasma, Wayland) | yes | yes | yes |
| Windows | CI | CI smoke test | no |
| macOS (Apple Silicon) | CI | CI smoke test | no |
| macOS (Intel) | release build only | no | no |

## Verified on Linux (KDE Plasma, Wayland)
| Area | How it was checked |
|---|---|
| Build and tests | `cargo build --workspace`; 18 unit tests (config, URLs, host matching, semver, presets incl. no-JS, site names, manifest icons, grayscale, single-instance, export file format, id validation, name cleaning, create-time validation) |
| CLI | create (name and icon from real sites), list, check, update, clear (refuses while running), icon from file and URL, `icons` against GitHub, Squoosh, YouTube Music |
| Icons | PWA manifest preferred; local file deleted after import still works; config copied to another path still resolves |
| Manager window | launched and captured for Apps, Create, Updates, Settings, Edit; title-bar icon is the Penates logo |
| App window | launches, GitHub loads, title-bar icon is the site icon, registered with the window manager under the launcher's name, shown in the taskbar list |
| Taskbar and tray (seen on screen) | confirmed by eye: taskbar entry and tray icon show the app icon; tray item registered; tray icon read back in color and grayscale (equal RGB on every visible pixel) |
| Downloads | saved to `~/Downloads`, second download gets a new name |
| Same-site popups | a real click on a `target=_blank` link to the app's own site opened a second app window (confirmed on screen) |
| External links | real clicks sent to the browser (logged by a stand-in `xdg-open`); `steam:` blocked, `mailto:` and `https:` passed, `file:` refused by WebKit |
| Wrapped-site isolation | a page inside an app window cannot call Tauri commands (all refused) |
| Site permissions | notifications: real clicks gave "granted" and the notification was shown; camera and microphone: "granted: audio, video". With both settings off, both are denied (automated check). |
| Cache | clearing frees the cache and keeps cookies and storage |
| Bad ids | `..`, `../x`, `a/b` rejected by `remove --purge` and `clear --data` |
| Export and import | CLI round trip in a scratch config: export one and all, import with rename/skip/overwrite, custom JavaScript stripped by default and kept with `--allow-js`, no cookies in the file, path-like id and `javascript:` URL rejected or sanitized |
| Packages | see [PACKAGING](PACKAGING.md) |

## Not verified
- Import and Export buttons in the real manager window (only the preview and the CLI were exercised).
- File chooser in the icon picker; the Save and Update buttons end to end in the real window.
- Site permissions on Windows and macOS (not enforced there yet).
- **Windows and macOS by hand.** CI smoke tests (every push) create an app, check its launcher (Start Menu shortcut with icon on Windows, `.app` bundle on macOS), launch it through that launcher, open the manager, and export/remove/import it. Tray, popups, downloads and site permissions there are untested.
- Per-app window theme on Windows and macOS.

## Known limits
See "Known limitations" in the [changelog](../CHANGELOG.md).
