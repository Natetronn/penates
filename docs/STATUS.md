# Status

What has actually been run, and where. Last updated 2026-11-14, version 0.1.0.

## Verified on Linux (KDE Plasma, Wayland)
| Area | How it was checked |
|---|---|
| Build and tests | `cargo build --workspace`; 14 unit tests (config, URLs, host matching, semver, presets, site names, manifest icons, grayscale, single-instance, export file format) |
| CLI | create (name and icon from real sites), list, check, update, clear (refuses while running), icon from file and URL, `icons` against GitHub, Squoosh, YouTube Music |
| Icons | PWA manifest preferred; local file deleted after import still works; config copied to another path still resolves |
| Manager window | launched and captured for Apps, Create, Updates, Settings, Edit; title-bar icon is the Penates logo |
| App window | launches, GitHub loads, title-bar icon is the site icon, registered with the window manager under the launcher's name, shown in the taskbar list |
| Tray | tray item registered; icon file read back in color and grayscale (equal RGB on every visible pixel) |
| Downloads | saved to `~/Downloads`, second download gets a new name |
| External links | sent to the browser (logged by a stand-in `xdg-open`) |
| Wrapped-site isolation | a page inside an app window cannot call Tauri commands (all refused) |
| Cache | clearing frees the cache and keeps cookies and storage |
| Export and import | CLI round trip in a scratch config: export one and all, import with rename/skip/overwrite, custom JavaScript stripped by default and kept with `--allow-js`, no cookies in the file, path-like id and `javascript:` URL rejected or sanitized |
| Packages | see [PACKAGING](PACKAGING.md) |

## Not verified
- Same-site popup windows opened by a real click.
- The taskbar entry and tray icon **as seen on screen** (the panel auto-hides in the test setup).
- Import and Export buttons in the real manager window (only the preview and the CLI were exercised).
- File chooser in the icon picker; the Save and Update buttons end to end in the real window.
- Notifications from sites, camera and microphone prompts.
- **Windows and macOS: the code has never been compiled.** A Windows compile check needs `mingw-w64-gcc`.
- Per-app window theme on Windows and macOS.

## Known limits
See "Known limitations" in the [changelog](../CHANGELOG.md).
