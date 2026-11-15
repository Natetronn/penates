# Configuration

Each app is one TOML file, `~/.config/penates/apps/<id>.toml`. Edit it by hand (`penates edit <id>`), in the manager (Edit), or create it with `penates create`. Changes apply the next time the app starts. Missing fields use the defaults below.

## App fields
| Field | Default | Meaning |
|---|---|---|
| `id` | from name | file name and profile id (letters, digits, `-`, `_`) |
| `name` | from site | shown in menus and the window title |
| `url` | required | the site to open (http/https) |
| `icon` | found automatically | file name inside `~/.config/penates/icons/` |
| `custom_icon` | `false` | `true` once you chose the icon; `update` never replaces it |
| `catalog_id`, `catalog_version` | none | preset this app follows and its last applied version |
| `user_agent` | default | override the browser identity |
| `width`, `height` | 1200, 800 | window size (size when restored if maximized) |
| `zoom` | 1.0 | page zoom (0.25 to 5) |
| `start_maximized` | `true` | open full frame |
| `start_fullscreen` | `false` | open fullscreen |
| `resizable` | `true` | |
| `always_on_top` | `false` | |
| `decorations` | `true` | native title bar and borders (`false` = frameless) |
| `hide_menu_bar` | `true` | |
| `devtools` | `false` | enable the web inspector |
| `single_instance` | `true` | launching again focuses the open window |
| `external_links_in_browser` | `true` | links outside this site and `internal_hosts` open in your browser |
| `internal_hosts` | `[]` | extra hosts that stay in-app; `*.example.com` wildcards allowed |
| `inject_css`, `inject_js` | none | injected into every page (JS runs before page scripts) |
| `theme` | `"system"` | `"system"`, `"light"` or `"dark"` window theme (Windows and macOS; see [THEMING](THEMING.md)) |
| `allow_notifications` | `true` | let the site show desktop notifications (Linux; see below) |
| `allow_camera_microphone` | `false` | let the site use the camera and microphone, for video calls (Linux; see below) |
| `tray` | `false` | show a system tray icon |
| `tray_grayscale` | `false` | draw the tray icon in black and white |
| `close_to_tray` | `false` | closing the window hides it; quit from the tray menu |

### Site permissions by platform
- **Linux (WebKitGTK):** enforced by Penates. Requests are answered from these two settings and everything else (location, etc.) is denied. Camera/microphone also switches on WebKit's media-stream and WebRTC support, which are off by default. As in browsers, a site can only ask for notification permission after a click.
- **Windows (WebView2)** and **macOS (WKWebView):** these settings are not applied yet. The webview shows its own prompts; desktop notifications from sites are not supported by these webviews. The generated macOS bundles include the camera and microphone usage descriptions macOS requires before a site may ask.

## Global settings
`~/.config/penates/settings.toml`:
```toml
[updates]
manifest_url = ""        # https:// or file://; empty = bundled sample manifest
check_on_start = true

[appearance]
theme = "system"         # system | light | dark
skin = "default"         # default | nord | solarized | mono
accent = "blue"          # blue | violet | green | orange | rose | slate
```

## Environment
- `PENATES_DESKTOP`: path to the `penates-desktop` binary (the CLI uses it to run apps).
- `XDG_CONFIG_HOME`, `XDG_DATA_HOME`: honored on Linux, handy for testing in a scratch directory.
