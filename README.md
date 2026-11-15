# Penates

Turn any website or PWA into a native desktop app: its own window, icon, taskbar entry and
isolated login profile. Built with Rust and [Tauri 2](https://v2.tauri.app/), an alternative to
browser "install as app" shortcuts, with a command line tool and a desktop manager.

> **Status: 0.1.0 development snapshot.** Linux (KDE/Wayland) is built and exercised. Windows and
> macOS code exists but has never been compiled or run. See [docs/STATUS.md](docs/STATUS.md) for exactly what is verified.

## What it does
- **One command or a few clicks:** `penates create github.com` finds the site's name and icon (PWA manifest, apple-touch, favicon) and adds it to your application menu.
- **A real app window per site:** own taskbar entry and icon, separate cookies and storage per app, single-window mode, optional system tray icon (color or grayscale).
- **Per-app options:** window size, zoom, maximized/fullscreen/frameless, always on top, user agent, extra in-app hosts, custom CSS/JS, devtools.
- **Smart links:** links to other sites open in your browser; same-site popups (sign-in flows) stay in the app; downloads go to your Downloads folder.
- **Housekeeping:** clear cache (keeps you signed in) or clear cookies and data, update checks with versioned presets, icons from a file or URL.
- **Themeable manager:** light/dark/system, six accents, four looks, all from plain CSS tokens.

## Install
Build from source (Arch/CachyOS shown; see [docs/PACKAGING.md](docs/PACKAGING.md) for `.deb`, `.rpm`, AppImage and tarball):

```sh
sudo pacman -S --needed rustup webkit2gtk-4.1 libayatana-appindicator
rustup default stable
git clone <this repo> && cd penates
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

## Docs
[Architecture](docs/ARCHITECTURE.md) · [CLI](docs/CLI.md) · [Configuration](docs/CONFIGURATION.md) · [Theming](docs/THEMING.md) · [Updates](docs/UPDATES.md) · [Packaging](docs/PACKAGING.md) · [Status](docs/STATUS.md) · [Changelog](CHANGELOG.md)

## License
MIT. See [LICENSE](LICENSE).
