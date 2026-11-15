# Architecture

```
            ┌────────────────────┐        ┌──────────────────────────────┐
 penates ───▶│  penates-core (lib) │◀───────│ penates-desktop (Tauri 2)      │
 (CLI)      │  store · icons ·   │        │  manager window  (no args)    │
            │  site · integration│        │  app runner      (--run <id>) │
            │  updates · single  │        └──────────────────────────────┘
            └────────────────────┘                   ▲
                      ▲                              │ spawns
                      └── config + icons ────────────┘
                          ~/.config/penates/
```

## Crates
- **`penates-core`**: everything that is not UI. `store` (paths, config files, cache folders), `icon` (find, fetch, normalize), `site` (name from meta tags and PWA manifest), `integration` (launchers per OS), `updates` (manifest, semver, presets), `ops` (shared create/edit/update flows), `single` (presence and focus over a loopback port).
- **`penates-cli`**: a thin `clap` front end over core. `run` and `gui` start the desktop binary, which it looks for next to itself, on `PATH`, or in `$PENATES_DESKTOP` (or `$APPIMAGE`).
- **`penates-desktop`** (`src-tauri`): one binary, two modes.
  - No arguments: the **manager** window. It loads `ui/index.html` and calls Rust commands (list, create, save, update, icons, cache, appearance).
  - `--run <id>`: the **runner**. It opens one external URL in a webview with that app's options.

## The runner
- **Isolated profile**: each app's cookies/storage live in `~/.local/share/penates/profiles/<id>` (Linux, Windows). macOS uses a per-app WKWebView data-store id instead of a folder.
- **Identity**: the window's app ID / class is `penates-<id>`, which equals the launcher file name `penates-<id>.desktop`, so the desktop shows the right icon and groups windows. Windows sets a per-app AppUserModelID.
- **Navigation**: `on_navigation` keeps the app's own host plus `internal_hosts` in the window and sends everything else to the system browser. `on_new_window` lets same-site popups open (keeping `window.opener` for sign-in flows) and sends other sites to the browser. `on_download` saves to the Downloads folder without overwriting.
- **Single window**: each running app owns a loopback port derived from its id. A second launch asks the first to focus and exits. The manager and CLI use the same port to tell whether an app is running (cache clearing refuses when it is).
- **Isolation of the host**: only the `manager` window has a capability. A wrapped site sees a `window.__TAURI__` object but every call is refused, and the manager's commands are not registered in the runner process.
- **Tray**: optional icon built from the app's stored icon, converted to grayscale on request.

## Launcher identity
| OS | What is written | Matching rule |
|---|---|---|
| Linux | `~/.local/share/applications/penates-<id>.desktop`, then the KDE app cache is refreshed | file name = window app ID |
| macOS | `~/Applications/Penates/<Name>.app` containing a copy of the runner so the Dock shows the app's name and icon | bundle |
| Windows | Start Menu shortcut created through PowerShell, `.ico` generated from the icon | AppUserModelID |

The manager's own launcher is `penates.desktop` (app ID `penates`); packages ship it, dev runs write it.

## Files
| What | Where |
|---|---|
| App configs | `~/.config/penates/apps/<id>.toml` |
| Icons (our copies) | `~/.config/penates/icons/<id>.png` |
| Manager and update settings | `~/.config/penates/settings.toml` |
| Browsing profiles | `~/.local/share/penates/profiles/<id>/` |
| Launchers | `~/.local/share/applications/` (Linux) |
