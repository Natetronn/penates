# Configuration

Each app is one TOML file, `~/.config/penates/apps/<id>.toml` (other systems: see [Architecture](ARCHITECTURE.md#files)). Edit it by hand (`penates edit <id>`), in the manager (Edit), or create it with `penates create`. Changes apply the next time the app starts. Missing fields use the defaults below; fields Penates no longer uses are ignored.

## App fields
| Field | Default | Meaning |
|---|---|---|
| `id` | from name | file name and profile id: lowercase letters, digits, `-`, `_`; at most 64 |
| `name` | from site | shown in menus and the window title |
| `url` | required | the site to open (http/https) |
| `icon` | found automatically | file name inside `~/.config/penates/icons/` |
| `custom_icon` | `false` | `true` once you chose the icon; `update` never replaces it |
| `catalog_id`, `catalog_version` | none | preset this app follows and its last applied version |
| `user_agent` | default | override the browser identity |
| `width`, `height` | 1200, 800 | window size (size when restored if maximized); 200 to 10000 |
| `zoom` | 1.0 | page zoom (0.25 to 5) |
| `start_maximized` | `true` | open full frame |
| `start_fullscreen` | `false` | open fullscreen |
| `resizable` | `true` | |
| `always_on_top` | `false` | |
| `decorations` | `true` | native title bar and borders (`false` = frameless) |
| `devtools` | `false` | enable the web inspector |
| `single_instance` | `true` | launching again focuses the open window |
| `external_links_in_browser` | `true` | links to other sites (not this one, its `www.` twin, or `internal_hosts`) open in your browser |
| `internal_hosts` | `[]` | extra hosts that stay in the app, e.g. a sign-in page. `*.example.com` matches it and every subdomain. Saved lowercase without scheme or path; wildcards over a whole top-level domain (`*.com`) are refused |
| `inject_css`, `inject_js` | none | injected into every page (JS runs before page scripts) |
| `theme` | `"system"` | `"system"`, `"light"` or `"dark"` window theme (Windows and macOS; see [THEMING](THEMING.md)) |
| `allow_notifications` | `true` | let the site show desktop notifications (Linux; see below) |
| `allow_camera_microphone` | `false` | let the site use the camera and microphone, for video calls (Linux; see below). Never screen sharing |
| `tray` | `false` | show a system tray icon |
| `tray_grayscale` | `false` | draw the tray icon in black and white |
| `close_to_tray` | `false` | closing the window hides it; quit from the tray menu |
| `launcher` | `true` | keep a launcher in the application menu (`false` for apps made with `--no-install`) |

### Site permissions by platform
- **Linux (WebKitGTK):** enforced by Penates. Notification and camera/microphone requests are answered from these two settings; screen capture, location and everything else are denied. Camera/microphone also switches on WebKit's media-stream and WebRTC support, which are off by default. As in browsers, a site can only ask for notification permission after a click.
- **Windows (WebView2)** and **macOS (WKWebView):** these settings are not applied yet. The webview shows its own prompts; desktop notifications from sites are not supported by these webviews. The generated macOS bundles include the camera and microphone usage descriptions macOS requires before a site may ask.

## Global settings
`~/.config/penates/settings.toml`:
```toml
[updates]
catalog_url = ""         # app catalog: https:// or file:// URL, or a path; empty = the Penates app catalog
check_on_start = true

[appearance]
theme = "system"         # system | light | dark
skin = "default"         # default | nord | solarized | mono
accent = "violet"        # violet | blue | green | orange | rose | slate
```
`catalog_url` was called `manifest_url` before 0.1.0; the old name is still read. Penates also keeps
`launchers_version` and `launchers_bin` here, to know when launchers need rewriting.

If the file stops parsing (a hand edit gone wrong), Penates moves it to `settings.toml.broken`, says so
on stderr and uses defaults, so the next save cannot overwrite your broken edit.

## Environment
- `PENATES_DESKTOP`: path to the `penates-desktop` binary (the CLI uses it to run apps; launchers point at it).
- `APPIMAGE`: set by AppImage itself; launchers then point at the AppImage file rather than its temporary mount.
- `VISUAL`, `EDITOR`: the editor for `penates edit` (arguments allowed).
- `XDG_CONFIG_HOME`, `XDG_DATA_HOME`: honored on Linux, handy for testing in a scratch directory.
