# Status

What has actually been run, and where. Last updated 2026-11-16, version 0.1.4.

| Platform | Build, tests, lint | App launched | Checked by hand |
|---|---|---|---|
| Linux (KDE Plasma, Wayland) | yes, and CI | yes, and CI smoke test | yes |
| Windows | CI | CI smoke test | no |
| macOS (Apple Silicon) | CI | CI smoke test | no |
| macOS (Intel) | release build only | no | no |

## Automated (Every Push)
- **Unit tests** (38): config and URL handling, the id rule, host patterns and matching, names from pages and
  addresses (including other alphabets), icon ranking, size limits, ICO files with grayscale PNG entries, grayscale tray icons, catalog parsing and presets
  (including "never JavaScript"), the catalog listing, catalog submissions, settings migration, export format and import safety, single-instance
  messaging, download names, CLI argument rules, and the CLI exiting quietly on a closed pipe. One test runs the runner's JavaScript link rule in Node and
  checks it against the Rust rule.
- **Clippy** with warnings as errors, on Linux, Windows and macOS.
- **Smoke tests** on Linux (under Xvfb), Windows and macOS: create an app, check its launcher (`.desktop`,
  Start Menu shortcut or `.app` bundle), launch it through that launcher, open the manager, export, remove and
  import it. On Linux also: a second launch hands over to the first, a second manager exits, and removing a
  running app's data is refused.

## Verified on Linux by Hand
| Area | How it was checked |
|---|---|
| CLI | create (name and icon from real sites, offline, non-Latin names), list (with a broken config), check, update, clear (refuses while running), icon from file and URL, `icons`, import with camera/devtools switched off, remove of a broken config |
| Manager window | the release build launched and captured for Apps, Updates, Edit and Settings with the content security policy on; the Catalog tab lists the live catalog; with no apps and no published release, Updates reads "No apps yet" and "up to date" |
| Icons | PWA manifest preferred (monochrome icons skipped, maskable ones after ordinary ones, one-tone icons only as a last resort: checked on all 22 catalog sites, only YouTube's changed); the App Catalog's cached icon is fetched again when the entry's icon address changes (a cache seeded with Gmail's old icon got the new one, and the old file was removed); local file deleted after import still works; config copied to another path still resolves |
| App window | launches, the site loads, title-bar icon is the site icon, registered under the launcher's name, shown in the taskbar |
| Taskbar and tray | taskbar entry and tray icon show the app icon; tray icon read back in color and grayscale |
| Start modes (automated, KWin + tray registry) | "only in the tray" with a tray: no window, tray icon registered; "minimized": minimized window; "only in the tray" without a tray: starts minimized instead; launching a tray-started app again shows its window |
| Downloads | saved to `~/Downloads`, a second download gets a new name |
| Toolbar | Linux, automated in an Arch Linux container on a virtual display (clicks there, never on a real desktop): Back, Forward, Start page and Reload act on the site, the address follows it, the layout keeps a 40 px toolbar with the site below after a resize, and the site's own page cannot call the toolbar's command (refused by Tauri). Windows and macOS: the CI smoke test starts an app with the toolbar; its screenshots show the toolbar below the title bar with the page's address and the site filling the rest. The Keys button opens the key list (Esc closes it); F1 and Alt+← work with the focus in the address field; Stop and the progress line show while a page's content loads, at once after Reload or Home, and clear when it finishes; new apps get the toolbar unless created with `--no-toolbar`. Open in browser not clicked yet Toolbar colors, in the same container: a site's `theme-color`, its light-mode color of a light/dark pair, a page background, no color (Penates's colors), a color changed while open, a custom color and the Penates setting all showed as expected, with readable icons and text |
| Memory | Linux, in an Arch Linux container: an app with the toolbar loading a page every 150 ms and opening the key list on each (1,580 loads in 4 minutes) and the manager left open with its update checks. Memory went down, not up: Penates's process 167 → 129 MB, WebKit's page process 232 → 147 MB; the manager 131 → 68 MB. No growth from the toolbar, the key list or the link rules |
| Navigation keys | automated on a local test site: Alt+Left went back, the mouse's forward button went forward, F5 reloaded; Alt+Left also confirmed with a real keypress (real mouse side buttons still to be tried). Alt+Home and the key list (F1, Ctrl+/), automated in a real app window on a local test site: F1 and Ctrl+/ show and hide the list, Esc closes it, a page that handles Ctrl+/ itself keeps it, F1 still works there, and Alt+Home went to the start page |
| Links (automated, 2026-11-15) | a local test site: an embedded frame from another site loads, a same-site link navigates in the app, and a link to another site does **not** load in the app |
| Links (real clicks, 2026-11-15, current code) | on a local test site: a link to another site opened the browser; a same-site `target=_blank` link opened a Penates popup window, not the browser; a link to another site inside that popup opened the browser |
| Sign-in | GitHub and Tailscale signed in inside their app windows (both redirect within the window) |
| Preset update in the manager (real clicks) | a test catalog with a newer GitHub preset: Updates listed 1.1.0 → 1.1.1, Update applied it (version and the new in-app host saved), nothing else in the app changed; with the live catalog, Cloudflare 1.0.0 → 1.0.1 applied and the icon the user had picked was kept |
| Your own catalogs | the release build in a Linux container: a catalog of mine with its own GitHub entry replaced the Penates one in `penates catalog` (23 apps); a missing catalog was reported as a warning (CLI) and in red under Settings → Updates while the rest were used; with only a missing catalog, a clear error; an old `catalog_url` was read as the only catalog. In the manager, a path typed into Add and Enter was checked, saved to `settings.toml` and listed. Remove, the Penates switch, the duplicate and bad-address messages were checked in the demo. **Choose file…** opens the system file dialog and is not yet tried |
| File pickers | the release build in a Linux container: Create → Choose file… opened the system dialog in the home folder, listing a folder and a PNG and hiding a text file. Import and Edit's Upload image… use the same command; in the demo, all three hand the chosen file on (Create shows its name, Edit sets the icon, Import goes to review) |
| App Catalog | in the demo (the real `ui/` files in a browser, driven over the DevTools protocol): Popular shows the 8 picks and hides while searching or filtering; search found 6 apps for "chat" and 1 for "google mail", and none for "zzz" with a message; the Media chip showed 3; Add turned a card into Open; with 30 extra test entries, 52 apps made 3 pages of 24, Next and a category change worked, and entries without a category went under Other. `penates catalog` with a search word, `--category` and an unknown category, on the catalog with categories. In the real manager window: see the screenshot in `docs/images/manager-catalog.png` |
| App Catalog Edit | in the demo: Edit on an added app's card opened that app's edit view on Apps, and Cancel returned to the list; in the real manager, the cards show Edit next to Open (`docs/images/manager-catalog.png`) |
| Log and problem reports | the CLI with scratch folders: a failed command and a warning were logged with time, level, version and source; `penates report` showed them with the description's web address cut to its site, and `--clear-log` deleted the log. The desktop program logged a failed start (`--run` of a missing app). Rotation, reading across both files, dates and the redaction of addresses, emails, IP addresses, the home folder and the user name are unit tests. The release build in a Linux container: the manager's Report a Problem page showed the version, system, WebKitGTK version and an empty log (`docs/images/manager-report.png`). Settings → Report a Problem, Back, Copy and Open on GitHub were checked in the demo; the filled-in GitHub form is not yet opened from a real report, and Copy and Open log folder are not yet tried in the real manager |
| Export in the manager (real click) | saved a `.penates` file to Downloads |
| Wrapped-site isolation | a page inside an app window cannot call Tauri commands |
| Site permissions | notifications and camera/microphone granted by real clicks when allowed, denied when not |
| Cache | clearing frees the cache and keeps cookies and storage |
| Export and import | CLI round trips in a scratch config: rename/skip/overwrite, custom JavaScript stripped unless allowed, camera/devtools switched off, no cookies in the file, bad ids and `javascript:` URLs refused or sanitized |
| Packages | see [PACKAGING](PACKAGING.md); the tarball installs into `~/.local` and the installed manager runs; the Arch package (0.1.1, built from the release tarball in an `archlinux:base-devel` container) installs with `pacman -U` in a clean Arch container, pulls in its dependencies, `penates --version` runs and the desktop program's libraries all resolve |

## To Check by Hand
- Settings → Export Folder: the folder picker (it needs real clicks), and Export, Export all and the backup saving into the chosen folder. Automated: a chosen folder is used while it exists; a deleted folder or a relative path falls back to Downloads.
- In the real manager window: Import (the file chooser was fixed to show `.penates` files), Save in Edit, and the icon file chooser.
- The mouse's back and forward side buttons with a real mouse.

## Not Verified
- Site permissions on Windows and macOS (not enforced there yet).
- **Windows and macOS by hand.** Tray, popups, downloads and site permissions there are untested.
- Per-app window theme on Windows and macOS.

## Known Limits
See "Known limitations" in the [changelog](../CHANGELOG.md).
