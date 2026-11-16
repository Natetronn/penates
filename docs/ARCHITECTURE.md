# Architecture

```
             ┌──────────────────────┐        ┌───────────────────────────────┐
 penates ───▶│  penates-core (lib)  │◀───────│ penates-desktop (Tauri 2)     │
 (CLI)       │  ops · store · site  │        │  manager window  (no args)    │
             │  icon · integration  │        │  app runner      (--run <id>) │
             │  updates · settings  │        └───────────────────────────────┘
             │  export · single     │                    ▲
             └──────────────────────┘                    │ spawns
                       ▲                                 │
                       └── config + icons ───────────────┘
                           ~/.config/penates/
```

## Crates
- **`penates-core`**: everything that is not UI.
  - `ops`: the flows both front ends call: create, edit, update, remove, launcher refresh. The CLI and the manager go through the same code, so an app ends up the same whichever one made it.
  - `store`: paths, config files, the id rule, cache folders.
  - `site`: downloads a page once (following redirects) and reads its name and icon links; `icon`: picks, downloads and normalizes icons (decoding is size-capped).
  - `integration`: launchers per OS. `updates`: the preset catalog and the Penates version check. `settings`: `settings.toml`. `export`: `.penates` files. `single`: presence and focus over a loopback port. `http`: the one HTTP client (timeout, size cap, connection reuse).
- **`penates-cli`**: a `clap` front end over `ops`, one small function per command. `run` and `gui` start the desktop binary, which it looks for in `$PENATES_DESKTOP`, `$APPIMAGE`, next to itself, or on `PATH`.
- **`penates-desktop`** (`src-tauri`): one binary, two modes.
  - No arguments: the **manager** (`main.rs`, `commands.rs`). It loads `ui/` and calls Rust commands; anything that touches the network or disk runs on a blocking thread. One manager at a time: a second start focuses the open window.
  - `--run <id>`: the **runner** (`runner.rs`). It opens one site in a webview with that app's options. It registers no plugins, and one command for the optional toolbar.

## The Runner
- **Isolated profile**: each app's cookies and storage live in its own folder (see Files). macOS uses a per-app WKWebView data-store id instead of a folder.
- **Identity**: the window's app ID / class is `penates-<id>`, which equals the launcher file name `penates-<id>.desktop`, so the desktop shows the right icon and groups windows. Windows sets a per-app AppUserModelID.
- **Navigation**: web pages load in the window, including frames embedded from other sites.
  - A click on a link to another site goes to the system browser: an injected script turns those clicks into `window.open`, which `on_new_window` hands to the browser, so only real clicks can open browser tabs.
  - "Another site" means: not the app's own host (with or without `www.`), not the page currently shown, and not in `internal_hosts`. The JavaScript rule and the Rust rule (`WebApp::is_internal`) are checked against each other by a test.
  - Popups to the app's own sites, or to a site the window has already loaded (a redirect target, a sign-in page), open as app windows with the same rules.
  - Other schemes such as `mailto:` go to the OS, at most one per second; `file:` and custom app schemes are dropped. `on_download` saves to the Downloads folder without overwriting.
- **Navigation keys**: an injected script gives every window Alt+Left/Right (Cmd+[ and ] on macOS), the mouse's back/forward buttons, F5/Ctrl+R, Alt+Home (Cmd+Shift+H) for the app's start page, and F1 or Ctrl+/ (Cmd+/) for a list of these keys. It only acts when the page did not handle the key itself; for the key list it waits until the page's own handlers have run, since sites often use Ctrl+/ for their own help. The list sits in a closed shadow root with constructed styles, so the site's CSS cannot restyle it and its content security policy cannot block it.
- **Toolbar** (on unless `toolbar = false`): the window holds two web views, the local page `ui/toolbar.html` (40 px) above the site. Its buttons call the runner's one command, `toolbar_action` (back, forward, reload, stop, start page, open in browser, the key list), which refuses any caller but the toolbar. The app's keys work in the toolbar too. While a page loads, Reload becomes Stop and a line runs along the bottom: the runner forwards the site's load events (on Linux "started" means the response began, so Reload and Home also show it at once; it clears after 30 seconds at most). The Keys button sends the site's script a `penates-keys` event and moves the focus into the site. The address is read from the site twice a second, so it also follows pages that change it without loading. On Linux both views sit in the window's GTK box (the toolbar keeps its height, the site takes the rest); on Windows and macOS they are placed by their bounds, updated on resize. Two web views in one window need Tauri's `unstable` feature. Its color: with `toolbar_color` "site" (or the default set to it), the runner reads the site's `theme-color` or background each time it reads the address (a script run in the site whose result comes back to the runner, so the site still has no way to reach Penates) and sends changes to the toolbar, which picks light or dark text by contrast.
- **Single window**: each running app owns a loopback port derived from its id and Penates's config folder (so two people on one computer, or a second Penates with its own folders, keep apart). A second launch asks the first to focus and exits. The manager and CLI use the same port to tell whether an app is running (clearing data refuses when it is).
- **Isolation of the host**: only the `manager` window has a capability and the manager's IPC commands. Wrapped sites get no Penates API at all (`withGlobalTauri` is off, and Tauri refuses IPC from remote origins, including the toolbar's command: "not allowed by ACL"). The manager page has a content security policy that allows only its own files.
- **Permissions (Linux)**: notifications and camera/microphone are answered from the app's settings; screen capture, location and everything else are denied.
- **Tray**: optional icon built from the app's stored icon, converted to grayscale on request. Close-to-tray is only switched on when the tray icon was created.

## Launcher Identity
| OS | What is written | Matching rule |
|---|---|---|
| Linux | `~/.local/share/applications/penates-<id>.desktop`, then the desktop's app caches are refreshed (once per batch) | file name = window app ID |
| macOS | `~/Applications/Penates/<Name>.app` containing a copy of the runner so the Dock shows the app's name and icon | bundle id `dev.penates.app.<id>` |
| Windows | Start Menu shortcut created through PowerShell, `.ico` generated from the icon | AppUserModelID on the running app (the shortcut itself does not carry it yet, see Status) |

The manager's own launcher is `penates.desktop` (app ID `penates`); packages ship it, dev runs write it. After Penates is updated or moved, the manager rewrites every app's launcher in the background.

## Files
| What | Linux | macOS | Windows |
|---|---|---|---|
| App configs | `~/.config/penates/apps/<id>.toml` | `~/Library/Application Support/penates/apps/` | `%APPDATA%\penates\apps\` |
| Icons (our copies) | `~/.config/penates/icons/<id>.png` | `…/penates/icons/` | `%APPDATA%\penates\icons\` |
| Settings | `~/.config/penates/settings.toml` | `…/penates/settings.toml` | `%APPDATA%\penates\settings.toml` |
| Browsing profiles | `~/.local/share/penates/profiles/<id>/` | WKWebView data stores | `%LOCALAPPDATA%\penates\profiles\<id>\` |
| Launchers | `~/.local/share/applications/` | `~/Applications/Penates/` | Start Menu `Programs\Penates\` |
| Popular-apps icon cache (safe to delete) | `~/.cache/penates/catalog-icons/` | `~/Library/Caches/penates/catalog-icons/` | `%LOCALAPPDATA%\penates\catalog-icons\` |

Config and settings files are written through a temporary file and a rename, so a crash never leaves half a file.
