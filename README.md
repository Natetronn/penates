<p align="center"><img src="docs/images/lockup.png" width="600" alt="Penates"></p>

Turn any website or web app into a desktop app with its own window, icon, taskbar entry, tray icon and
separate login. Named after the Roman household gods: every site gets a home of its own.
Built with Rust and [Tauri 2](https://v2.tauri.app/), with a command line tool and a desktop manager.

**[getpenates.com](https://getpenates.com)**: the website, with the docs and a clickable demo of the manager.

**[Download the latest release](https://github.com/Natetronn/penates/releases/latest)**:
AppImage, .deb, .rpm, Arch package (`sudo pacman -U`), Windows installer, macOS .dmg, or on Arch from the AUR: `penates-bin` (prebuilt) or `penates` (built from source), installed with `makepkg -si` or an AUR helper such as paru or yay ([how](https://getpenates.com/getting-started/)).

<p align="center"><img src="docs/images/manager-apps.png" width="640" alt="The Penates manager listing five apps: Discord, GitHub, Notion, Proton Mail and YouTube Music"></p>

> **Status: 0.1.1.** Verified by hand on Linux (KDE Plasma, Wayland). Windows and macOS are built, launched
> through their launchers and smoke-tested in CI on every push. See [docs/STATUS.md](docs/STATUS.md) for exactly what is verified.

## What it does
- **One command or a few clicks:** `penates create github.com` finds the site's name and icon (PWA manifest, apple-touch, favicon) and adds it to your application menu.
- **A real app window per site:** own taskbar entry and icon, separate cookies and storage per app, single-window mode, optional system tray icon (color or grayscale), and it can start as a window, minimized or only in the tray.
- **Per-app options:** window size, zoom, maximized/fullscreen/frameless, always on top, user agent, extra in-app hosts, custom CSS/JS, devtools, notification and camera/microphone permissions.
- **App catalog:** add known sites (GitHub, Notion, Discord, Proton Mail, YouTube and more) in one click from Create → Popular apps, with small fixes such as in-app sign-in from the shared [penates-apps](https://github.com/Natetronn/penates-apps) catalog.
- **Smart links:** links to other sites open in your browser; same-site popups (sign-in flows) stay in the app; downloads go to your Downloads folder.
- **Browser keys without a browser:** Alt+←/→ or the mouse's side buttons go back and forward, F5 or Ctrl+R reloads.
- **Share and back up:** export one app or all of them to a `.penates` file and import it elsewhere; logins are never included. Propose an app's fixes for the shared catalog from its ⋮ menu. See [docs/SHARING.md](docs/SHARING.md).
- **Housekeeping:** clear cache (keeps you signed in) or clear cookies and data, update checks with versioned presets, icons from a file or URL.
- **Themeable manager:** light/dark/system, six accents, four looks, all from plain CSS tokens.

## Screenshots
| Your apps (light) | Edit an app |
|---|---|
| <img src="docs/images/manager-apps-light.png" width="400" alt="The app list in light mode"> | <img src="docs/images/manager-edit.png" width="400" alt="Editing GitHub: name, URL, id, icon picker and window options"> |
| **Settings** | **About** |
| <img src="docs/images/manager-settings.png" width="400" alt="Settings: four looks, light or dark, accent colors, update checking and the app catalog"> | <img src="docs/images/manager-about.png" width="400" alt="About Penates: logo, version, links and Credits"> |

## Install
Get an installer from the [releases page](https://github.com/Natetronn/penates/releases/latest). macOS needs version 14 or later.

**First launch on macOS and Windows:** the installers are not signed with an Apple or Microsoft certificate yet.
On macOS, open the app once, then allow it under System Settings → Privacy & Security → **Open Anyway**.
On Windows, SmartScreen may say it protected your PC: choose **More info → Run anyway**. Penates's own
updates are signed and checked before they install.

Or build from source
(Arch/CachyOS shown; see [docs/PACKAGING.md](docs/PACKAGING.md) for `.deb`, `.rpm`, AppImage and tarball):

```sh
sudo pacman -S --needed rustup webkit2gtk-4.1 libayatana-appindicator
rustup default stable
git clone https://github.com/Natetronn/penates && cd penates
cd packaging/arch && makepkg -si        # installs penates + penates-desktop
```

Or just run it from the checkout: `cargo build --workspace`, then `target/debug/penates gui`.

## Quick start
```sh
penates create github.com            # name + icon found automatically
penates create https://mail.proton.me --name "Proton Mail" --icon https://example.com/logo.png
penates list
penates run github
penates gui                          # the manager window
penates check && penates update --all # preset + icon updates
penates clear github                 # clear cache; add --data to sign out too
```
Full reference: [docs/CLI.md](docs/CLI.md).

## Layout
| Path | What |
|---|---|
| `crates/penates-core` | shared logic: config store, icons, site names, launchers, updates, single-instance |
| `crates/penates-cli` | the `penates` command |
| `src-tauri` | `penates-desktop`: the manager window and the per-app runner (Tauri 2) |
| `ui` | manager front end (plain HTML/JS, `tokens.css`, `skins.css`; no build step) |
| `packaging`, `scripts` | Arch PKGBUILD, desktop file, package and tarball scripts |
| `docs` | architecture, configuration, theming, updates, packaging, status |
| `design` | logo sources, lockups, colors and type, and the script that rebuilds the app icons |

## Docs
[Architecture](docs/ARCHITECTURE.md) · [CLI](docs/CLI.md) · [Configuration](docs/CONFIGURATION.md) · [Theming](docs/THEMING.md) · [Updates](docs/UPDATES.md) · [Sharing](docs/SHARING.md) · [Packaging](docs/PACKAGING.md) · [Releasing](docs/RELEASING.md) · [Status](docs/STATUS.md) · [Changelog](CHANGELOG.md)

## License
MIT. See [LICENSE](LICENSE).
