//! The runner: one wrapped web app in its own window (`penates-desktop --run <id>`). The site gets
//! nothing from Penates but a window: no plugins, and its pages cannot call Penates. The only command,
//! `toolbar_action`, serves the optional toolbar (a local page above the site) and refuses anyone else.

use penates_core::{icon, integration, ops, single, WebApp};
use std::sync::{Arc, Mutex};
use tauri::webview::WebviewBuilder;
use tauri::{LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};

/// Height of the optional toolbar, in logical pixels.
const TOOLBAR_HEIGHT: f64 = 40.0;

/// Save downloads to the user's Downloads folder, never overwriting an existing file.
pub fn unique_download_path(suggested: &std::path::Path) -> std::path::PathBuf {
    let name = suggested.file_name().map(|n| n.to_os_string()).unwrap_or_else(|| "download".into());
    let dir = dirs::download_dir().or_else(dirs::home_dir).unwrap_or_else(std::env::temp_dir);
    let first = dir.join(&name);
    if !first.exists() {
        return first;
    }
    let p = std::path::Path::new(&name);
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|c| !c.exists()).expect("unbounded range")
}

pub fn theme_of(name: &str) -> Option<tauri::Theme> {
    match name {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => None,
    }
}

/// Stable 16-byte data-store id per app (macOS WKWebView profiles). Built everywhere so it is tested.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn id_bytes(id: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    for (i, chunk) in out.chunks_mut(4).enumerate() {
        let mut h: u32 = 2166136261 ^ (i as u32).wrapping_mul(0x9e37_79b1);
        for b in id.bytes() {
            h = (h ^ b as u32).wrapping_mul(16777619);
        }
        chunk.copy_from_slice(&h.to_le_bytes());
    }
    out
}

/// Hand a link the app won't load itself to the OS, but only for schemes a browser would open
/// without asking. `file:`, `smb:` and custom app schemes are dropped: a site must not be able to
/// make the desktop open local files or launch other programs.
fn open_external(u: &url::Url) {
    // At most one hand-off per second, so a page cannot flood the desktop with browser tabs or
    // mail windows.
    static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    if !matches!(u.scheme(), "http" | "https" | "mailto" | "tel") {
        eprintln!("blocked opening {}: link", u.scheme());
        return;
    }
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(1)) {
        eprintln!("ignored rapid link to {}", u.scheme());
        return;
    }
    *last = Some(std::time::Instant::now());
    let _ = open::that_detached(u.as_str());
}

/// WebKitGTK denies every permission request the app does not answer, and ships with camera and
/// WebRTC switched off. Answer notification and camera/microphone requests from the app's settings;
/// deny everything else (screen capture, location, pointer lock, etc.).
#[cfg(target_os = "linux")]
fn apply_site_permissions(win: &tauri::Webview, cfg: &WebApp) {
    let (notify, media) = (cfg.allow_notifications, cfg.allow_camera_microphone);
    let _ = win.with_webview(move |wv| {
        use gtk::glib::prelude::*;
        use webkit2gtk::{
            DeviceInfoPermissionRequest, NotificationPermissionRequest, PermissionRequestExt, SettingsExt,
            UserMediaPermissionRequest, UserMediaPermissionRequestExt, WebViewExt,
        };
        let view = wv.inner();
        if media {
            if let Some(s) = WebViewExt::settings(&view) {
                s.set_enable_media_stream(true);
                s.set_enable_webrtc(true);
            }
        }
        view.connect_permission_request(move |_, req| {
            let allow = if req.is::<NotificationPermissionRequest>() {
                notify
            } else if let Some(m) = req.downcast_ref::<UserMediaPermissionRequest>() {
                // Screen capture is also a user-media request; only real cameras and microphones count.
                media && (m.is_for_audio_device() || m.is_for_video_device())
            } else if req.is::<DeviceInfoPermissionRequest>() {
                media
            } else {
                false
            };
            if allow {
                req.allow()
            } else {
                req.deny()
            }
            true
        });
    });
}

fn toggle_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_window("main") {
        if w.is_visible().unwrap_or(true) && !w.is_minimized().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

fn build_tray(app: &tauri::App, cfg: &WebApp) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let toggle = MenuItem::with_id(app, "toggle", "Show / Hide", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &quit])?;

    let icon = cfg
        .icon_path()
        .and_then(|p| icon::rgba_for_tray(&p, cfg.tray_grayscale, 64).ok())
        .map(|(px, w, h)| tauri::image::Image::new_owned(px, w, h))
        .or_else(|| app.default_window_icon().cloned());

    let mut b = TrayIconBuilder::with_id(format!("penates-{}", cfg.id))
        .tooltip(&cfg.name)
        .menu(&menu)
        // Linux trays emit no click events (Tauri docs), so the menu must open on click there.
        .show_menu_on_left_click(cfg!(target_os = "linux"))
        .on_menu_event(|app, e| match e.id.as_ref() {
            "toggle" => toggle_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                toggle_main(tray.app_handle());
            }
        });
    if let Some(i) = icon {
        b = b.icon(i);
    }
    b.build(app)?;
    Ok(())
}

/// JavaScript `inApp(host)` with the same rule as `WebApp::is_internal` for http(s) links: the app's
/// own host (with or without `www.`), the page currently shown, and the extra in-app hosts.
fn matcher_script(cfg: &WebApp) -> String {
    format!(
        r#"const own = {own}, pats = {pats};
const bare = (h) => h.toLowerCase().replace(/^www\./, "");
const inApp = (host) => {{
  const h = host.toLowerCase();
  if (bare(h) === own || bare(h) === bare(location.hostname)) return true;
  return pats.some((p) => p.startsWith("*.") ? (h === p.slice(2) || h.endsWith(p.slice(1))) : p === h);
}};"#,
        own = serde_json::to_string(&cfg.own_host().unwrap_or_default()).unwrap(),
        pats = serde_json::to_string(&cfg.internal_hosts).unwrap(),
    )
}

/// Script that sends clicks on links to other sites to the system browser (via `window.open`, which
/// `on_new_window` hands to the OS). Only real clicks count, so a page cannot open browser tabs on its
/// own, and embedded frames from other sites load normally.
fn link_guard_script(cfg: &WebApp) -> String {
    format!(
        r#"(() => {{
{matcher}
document.addEventListener("click", (e) => {{
  if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
  const a = e.target instanceof Element ? e.target.closest("a[href]") : null;
  if (!a) return;
  let u; try {{ u = new URL(a.href, location.href); }} catch {{ return; }}
  if ((u.protocol === "http:" || u.protocol === "https:") && !inApp(u.hostname)) {{
    e.preventDefault(); e.stopImmediatePropagation(); window.open(u.href, "_blank");
  }}
}}, true);
}})();"#,
        matcher = matcher_script(cfg)
    )
}

/// Browser keys for app windows: Alt+Left/Right (Cmd+[ and ] on macOS) and the mouse's back/forward
/// buttons go back and forward, F5 and Ctrl+R reload, Alt+Home (Cmd+Shift+H) goes to the app's start
/// page, and F1 or Ctrl+/ (Cmd+/) shows these keys. A page that handles one of these itself (it calls
/// preventDefault) keeps it; for the key list, whose keys sites often use for their own help (Slack, Google
/// Docs), the decision waits until the page's own handlers have run. The key list lives in a shadow root with constructed styles, so the site's
/// CSS cannot restyle it and a strict content security policy cannot block it.
fn navigation_script(home: &str) -> String {
    const SCRIPT: &str = r#"(() => {
  if (window.top !== window) return;
  const HOME = __HOME__;
  const mac = /Mac/.test(navigator.platform);
  const keys = [
    ["Back", mac ? "⌘ [" : "Alt+←", "or the mouse's back button"],
    ["Forward", mac ? "⌘ ]" : "Alt+→", "or the mouse's forward button"],
    ["Reload", mac ? "⌘ R" : "F5 or Ctrl+R", ""],
    ["Start page", mac ? "⌘ ⇧ H" : "Alt+Home", ""],
    ["These keys", mac ? "⌘ /" : "F1 or Ctrl+/", "Esc closes"],
  ];
  let host = null;
  const close = () => { host?.remove(); host = null; };
  const toggle = () => {
    if (host) return close();
    host = document.createElement("penates-keys");
    const root = host.attachShadow({ mode: "closed" });
    const css = `:host{all:initial;position:fixed;inset:0;z-index:2147483647;display:grid;place-items:center;background:rgba(10,8,20,.45);font:14px/1.5 system-ui,sans-serif}
.box{background:#191726;color:#f4f2ff;border:1px solid #2e2b45;border-radius:12px;padding:18px 22px;min-width:300px;box-shadow:0 20px 60px rgba(0,0,0,.5)}
h2{margin:0 0 10px;font-size:15px;font-weight:600}table{border-collapse:collapse;width:100%}td{padding:5px 0;vertical-align:top}
td+td{padding-left:18px;text-align:right;white-space:nowrap}kbd{font:12px ui-monospace,monospace;background:#29263d;border:1px solid #3a3658;border-radius:5px;padding:1px 6px}
small{display:block;color:#a9a5c4;font-size:12px}`;
    try { const sheet = new CSSStyleSheet(); sheet.replaceSync(css); root.adoptedStyleSheets = [sheet]; }
    catch { const st = document.createElement("style"); st.textContent = css; root.append(st); }
    const box = document.createElement("div"); box.className = "box";
    const h = document.createElement("h2"); h.textContent = "Keys in this app";
    const table = document.createElement("table");
    for (const [what, key, note] of keys) {
      const tr = table.insertRow(), a = tr.insertCell(), b = tr.insertCell();
      a.textContent = what;
      if (note) { const s = document.createElement("small"); s.textContent = note; a.append(s); }
      const k = document.createElement("kbd"); k.textContent = key; b.append(k);
    }
    box.append(h, table); root.append(box);
    host.addEventListener("click", (e) => { if (e.composedPath()[0] === host) close(); });
    document.documentElement.append(host);
  };
  addEventListener("keydown", (e) => {
    if (e.defaultPrevented || e.repeat) return;
    const k = e.key.toLowerCase(), cmd = mac ? e.metaKey : e.ctrlKey;
    const back = (e.altKey && e.key === "ArrowLeft") || (mac && e.metaKey && e.key === "[");
    const forward = (e.altKey && e.key === "ArrowRight") || (mac && e.metaKey && e.key === "]");
    const reload = e.key === "F5" || (cmd && !e.shiftKey && k === "r");
    const home = (e.altKey && e.key === "Home") || (mac && e.metaKey && e.shiftKey && k === "h");
    const help = e.key === "F1" || (cmd && !e.altKey && e.key === "/");
    if (host && e.key === "Escape") { e.preventDefault(); return close(); }
    if (help) return void setTimeout(() => { if (!e.defaultPrevented) toggle(); });
    if (!back && !forward && !reload && !home) return;
    e.preventDefault();
    if (home) location.assign(HOME);
    else if (reload) location.reload();
    else history.go(back ? -1 : 1);
  });
  addEventListener("mouseup", (e) => {
    if (e.defaultPrevented || (e.button !== 3 && e.button !== 4)) return;
    e.preventDefault();
    history.go(e.button === 3 ? -1 : 1);
  });
})();"#;
    SCRIPT.replace("__HOME__", &serde_json::to_string(home).unwrap())
}

/// Hosts the app's main window has actually loaded (the site itself, redirect targets). Popups to
/// these stay in the app, like popups to its own site.
type Visited = Arc<Mutex<Vec<String>>>;

fn remember(visited: &Visited, u: &url::Url) {
    if let Some(h) = u.host_str().map(|h| penates_core::bare_host(h).to_ascii_lowercase()) {
        let mut v = visited.lock().unwrap_or_else(|e| e.into_inner());
        if !v.contains(&h) {
            v.push(h);
        }
    }
}

/// The rules every web view of a wrapped app gets: the site in its main window and any popup it opens.
/// A macro because the plain window (`WebviewWindowBuilder`) and the site under the toolbar
/// (`WebviewBuilder`) are different builders with the same methods.
macro_rules! with_rules {
    ($b:expr, $cfg:expr, $visited:expr, $handle:expr) => {{
        let cfg: &WebApp = $cfg;
        let mut b = $b.initialization_script(navigation_script(&cfg.url));
        if cfg.external_links_in_browser {
            b = b.initialization_script(link_guard_script(cfg));
        }
        if let Some(ua) = &cfg.user_agent {
            b = b.user_agent(ua);
        }
        let (popup_cfg, visited, handle): (WebApp, Visited, tauri::AppHandle) = (cfg.clone(), $visited.clone(), $handle);
        b.on_navigation(|u| {
            // Web pages (including embedded frames from other sites) load; anything else (mailto:,
            // custom app schemes) is handed to the OS when safe, never loaded.
            if matches!(u.scheme(), "http" | "https" | "about" | "blob" | "data") {
                true
            } else {
                open_external(u);
                false
            }
        })
        .on_new_window(move |u, features| {
            // Popups to the app's own sites (sign-in flows) open as app windows with these same rules;
            // anything else goes to the system browser.
            let seen = u.host_str().is_some_and(|h| {
                let h = penates_core::bare_host(h).to_ascii_lowercase();
                visited.lock().unwrap_or_else(|e| e.into_inner()).contains(&h)
            });
            if popup_cfg.is_internal(&u) || seen {
                match open_popup(&handle, &popup_cfg, &visited, features) {
                    Some(window) => tauri::webview::NewWindowResponse::Create { window },
                    None => tauri::webview::NewWindowResponse::Deny,
                }
            } else {
                open_external(&u);
                tauri::webview::NewWindowResponse::Deny
            }
        })
        .on_download(|_wv, ev| {
            use tauri::webview::DownloadEvent;
            match ev {
                DownloadEvent::Requested { destination, .. } => {
                    *destination = unique_download_path(destination);
                    true
                }
                DownloadEvent::Finished { path, success, .. } => {
                    eprintln!("download {}: {:?}", if success { "finished" } else { "failed" }, path);
                    true
                }
                _ => true,
            }
        })
    }};
}

/// The site's own web view settings on top of the rules: its profile, the custom CSS and JS, and
/// remembering the hosts it loads.
macro_rules! site_view {
    ($b:expr, $cfg:expr, $visited:expr, $handle:expr) => {{
        let cfg: &WebApp = $cfg;
        let mut w = $b.devtools(cfg.devtools);
        // Custom CSS and JS are separate scripts, so an error in the JS cannot stop the CSS.
        if let Some(css) = &cfg.inject_css {
            let lit = serde_json::to_string(css).unwrap();
            w = w.initialization_script(format!(
                "document.addEventListener('DOMContentLoaded',()=>{{const s=document.createElement('style');s.textContent={lit};document.head.appendChild(s);}});"
            ));
        }
        if let Some(js) = &cfg.inject_js {
            w = w.initialization_script(js.clone());
        }
        let seen = $visited.clone();
        w = with_rules!(w, cfg, $visited, $handle).on_page_load(move |_, payload| remember(&seen, payload.url()));
        // Isolated profile per app. macOS WKWebView uses a data-store id instead of a directory.
        #[cfg(target_os = "macos")]
        {
            w = w.data_store_identifier(id_bytes(&cfg.id));
        }
        #[cfg(not(target_os = "macos"))]
        {
            w = w.data_directory(penates_core::store::profile_dir(&cfg.id));
        }
        w
    }};
}

fn open_popup(
    handle: &tauri::AppHandle,
    cfg: &WebApp,
    visited: &Visited,
    features: tauri::webview::NewWindowFeatures,
) -> Option<tauri::WebviewWindow> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let label = format!("popup-{}", N.fetch_add(1, Ordering::Relaxed));
    let b = WebviewWindowBuilder::new(handle, label, WebviewUrl::External("about:blank".parse().ok()?))
        .title(&cfg.name)
        .window_features(features)
        .on_document_title_changed(|w, title| {
            let _ = w.set_title(&title);
        });
    match with_rules!(b, cfg, visited, handle.clone()).build() {
        Ok(w) => Some(w),
        Err(e) => {
            eprintln!("popup: {e}");
            None
        }
    }
}

/// The app's main window and the web view showing the site: a plain window, or with `toolbar` on, a
/// window holding the toolbar page above the site.
fn build_main_window(app: &tauri::App, cfg: &WebApp, visited: &Visited) -> tauri::Result<(tauri::Window, tauri::Webview)> {
    let url = cfg.url.parse::<url::Url>().map_err(|e| anyhow::anyhow!("invalid URL in config: {e}"))?;
    if cfg.toolbar {
        return build_toolbar_window(app, cfg, visited, url);
    }
    let w = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title(&cfg.name)
        .theme(theme_of(&cfg.theme))
        .inner_size(cfg.width, cfg.height)
        .maximized(cfg.start_maximized)
        .fullscreen(cfg.start_fullscreen)
        .resizable(cfg.resizable)
        .always_on_top(cfg.always_on_top)
        .decorations(cfg.decorations)
        // "Only in the tray" starts hidden; `run` shows it (minimized) if the tray icon fails.
        .visible(cfg.start_as != "tray");
    let w = site_view!(w, cfg, visited, app.handle().clone()).build()?;
    let site: &tauri::Webview = w.as_ref();
    Ok((site.window(), site.clone()))
}

/// The window with the toolbar: the toolbar page (`ui/toolbar.html`) on top and the site below it,
/// both resized with the window. The address on the toolbar follows the site, also on pages that
/// change it without loading (single-page apps), so it is read from the site twice a second.
fn build_toolbar_window(
    app: &tauri::App,
    cfg: &WebApp,
    visited: &Visited,
    url: url::Url,
) -> tauri::Result<(tauri::Window, tauri::Webview)> {
    let window = tauri::window::WindowBuilder::new(app, "main")
        .title(&cfg.name)
        .theme(theme_of(&cfg.theme))
        .inner_size(cfg.width, cfg.height)
        .maximized(cfg.start_maximized)
        .fullscreen(cfg.start_fullscreen)
        .resizable(cfg.resizable)
        .always_on_top(cfg.always_on_top)
        .decorations(cfg.decorations)
        .visible(cfg.start_as != "tray")
        .build()?;
    let size = window.inner_size()?.to_logical::<f64>(window.scale_factor()?);
    let bar = WebviewBuilder::new("toolbar", WebviewUrl::App("toolbar.html".into()));
    let toolbar = window.add_child(bar, LogicalPosition::new(0.0, 0.0), LogicalSize::new(size.width, TOOLBAR_HEIGHT))?;
    // On Linux both views are packed into the window's vertical box, which shares the height equally
    // and ignores bounds: the toolbar keeps its own height and the site takes the rest. (Windows and
    // macOS place the views by their bounds, kept up to date below.)
    #[cfg(target_os = "linux")]
    let _ = toolbar.with_webview(|wv| {
        use gtk::prelude::*;
        let view = wv.inner();
        if let Some(column) = view.parent().and_then(|p| p.downcast::<gtk::Box>().ok()) {
            column.set_child_packing(&view, false, true, 0, gtk::PackType::Start);
        }
        view.set_size_request(-1, TOOLBAR_HEIGHT as i32);
    });
    let site = site_view!(WebviewBuilder::new("site", WebviewUrl::External(url)), cfg, visited, app.handle().clone());
    let site = window.add_child(
        site,
        LogicalPosition::new(0.0, TOOLBAR_HEIGHT),
        LogicalSize::new(size.width, (size.height - TOOLBAR_HEIGHT).max(0.0)),
    )?;
    let (w, t, s) = (window.clone(), toolbar.clone(), site.clone());
    window.on_window_event(move |ev| {
        if matches!(ev, tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. }) {
            let Ok(size) = w.inner_size().and_then(|p| Ok(p.to_logical::<f64>(w.scale_factor()?))) else { return };
            let _ = t.set_bounds(tauri::Rect {
                position: LogicalPosition::new(0.0, 0.0).into(),
                size: LogicalSize::new(size.width, TOOLBAR_HEIGHT).into(),
            });
            let _ = s.set_bounds(tauri::Rect {
                position: LogicalPosition::new(0.0, TOOLBAR_HEIGHT).into(),
                size: LogicalSize::new(size.width, (size.height - TOOLBAR_HEIGHT).max(0.0)).into(),
            });
        }
    });
    let (t, s) = (toolbar.clone(), site.clone());
    std::thread::spawn(move || {
        let mut shown = String::new();
        while let Ok(u) = s.url() {
            if u.as_str() != shown {
                shown = u.to_string();
                let _ = t.eval(format!("window.penatesAddress?.({})", serde_json::to_string(&shown).unwrap()));
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    Ok((window, site))
}

/// What the toolbar's buttons do. Only the toolbar page may call it: the site's pages cannot reach
/// Penates at all (no capability lets remote pages use IPC), and this check refuses any other caller.
#[tauri::command]
fn toolbar_action(webview: tauri::Webview, app: tauri::AppHandle, cfg: tauri::State<'_, WebApp>, action: String) -> Result<(), String> {
    if webview.label() != "toolbar" {
        return Err("only the toolbar can do this".into());
    }
    let site = app.get_webview("site").ok_or("the site is not open")?;
    let done = match action.as_str() {
        "back" => site.eval("history.back()"),
        "forward" => site.eval("history.forward()"),
        "reload" => site.reload(),
        "home" => match cfg.url.parse() {
            Ok(u) => site.navigate(u),
            Err(e) => return Err(format!("start page: {e}")),
        },
        "browser" => {
            if let Ok(u) = site.url() {
                open_external(&u);
            }
            Ok(())
        }
        _ => return Err(format!("unknown toolbar action '{action}'")),
    };
    done.map_err(|e| e.to_string())
}

/// Closing the window hides it; the tray menu shows it again or quits.
fn enable_close_to_tray(win: &tauri::Window) {
    let w2 = win.clone();
    win.on_window_event(move |ev| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
            api.prevent_close();
            let _ = w2.hide();
        }
    });
}

#[cfg(windows)]
fn set_aumid(id: &str) {
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    let w: Vec<u16> = format!("dev.penates.app.{id}").encode_utf16().chain(Some(0)).collect();
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(w.as_ptr());
    }
}

pub fn run(cfg: WebApp, context: tauri::Context) -> tauri::Result<()> {
    // Taskbar identity: own WM class / app_id (Linux) and own AppUserModelID (Windows), so each
    // app gets its own taskbar entry matching its launcher and icon.
    #[cfg(target_os = "linux")]
    gtk::glib::set_prgname(Some(&integration::wm_class(&cfg.id)));
    #[cfg(windows)]
    set_aumid(&cfg.id);
    // Started straight from the CLI or a dev build: make sure the launcher (and so the taskbar icon)
    // exists, unless the app was created without one.
    if cfg.launcher && !integration::is_installed(&cfg) {
        if let Err(e) = ops::reinstall(None, &cfg) {
            eprintln!("launcher: {e:#}");
        }
    }

    // Every running app owns a loopback port (so the manager can tell it is running). In
    // single-window mode a second launch focuses the first window and exits. The port answers
    // from the start, so a second launch during a slow start does not open a second window.
    static HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();
    match single::try_bind(&cfg.id) {
        Some(l) => single::serve(l, cfg.id.clone(), || {
            if let Some(w) = HANDLE.get().and_then(|h| h.get_window("main")) {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }),
        None if cfg.single_instance && single::request_focus(&cfg.id) => return Ok(()),
        None => {}
    }

    tauri::Builder::default()
        .manage(cfg.clone())
        .invoke_handler(tauri::generate_handler![toolbar_action])
        .setup(move |app| {
            let _ = HANDLE.set(app.handle().clone());
            let visited: Visited = Arc::default();
            let (win, site) = build_main_window(app, &cfg, &visited)?;
            #[cfg(target_os = "linux")]
            apply_site_permissions(&site, &cfg);
            if (cfg.zoom - 1.0).abs() > f64::EPSILON {
                let _ = site.set_zoom(cfg.zoom);
            }
            if let Some(img) = cfg.icon_path().and_then(|p| tauri::image::Image::from_path(p).ok()) {
                let _ = win.set_icon(img);
            }
            let tray_ok = cfg.tray
                && match build_tray(app, &cfg) {
                    Ok(()) => true,
                    Err(e) => {
                        eprintln!("tray: {e}"); // no tray, so closing must really close
                        false
                    }
                };
            if tray_ok && cfg.close_to_tray {
                enable_close_to_tray(&win);
            }
            match cfg.start_as.as_str() {
                "minimized" => {
                    let _ = win.minimize();
                }
                // Without a tray icon a hidden window could not be reached: start minimized instead.
                "tray" if !tray_ok => {
                    let _ = win.show();
                    let _ = win.minimize();
                }
                _ => {}
            }
            Ok(())
        })
        .run(context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_store_ids_are_stable_and_distinct() {
        // The hash must not overflow: debug builds check arithmetic and would panic.
        assert_eq!(id_bytes("github"), id_bytes("github"));
        assert_ne!(id_bytes("github"), id_bytes("notion"));
        assert_ne!(id_bytes(""), [0u8; 16]);
    }

    #[test]
    fn download_names_never_overwrite() {
        let p = unique_download_path(std::path::Path::new("/somewhere/../../etc/passwd"));
        assert_eq!(p.file_name().unwrap(), "passwd");
        assert!(!p.starts_with("/etc"));
    }

    /// The JavaScript link guard and `WebApp::is_internal` must agree. Runs the script's matcher in
    /// Node (skipped when Node is not installed).
    #[test]
    fn js_and_rust_link_rules_agree() {
        let cfg = WebApp {
            url: "https://www.example.com/app".into(),
            internal_hosts: vec!["*.login.example.net".into(), "accounts.google.com".into()],
            ..Default::default()
        };
        let hosts = [
            "example.com", "www.example.com", "WWW.Example.COM", "sub.example.com", "login.example.net",
            "a.login.example.net", "evil-login.example.net", "accounts.google.com", "google.com", "other.org",
        ];
        let expected: Vec<bool> =
            hosts.iter().map(|h| cfg.is_internal(&url::Url::parse(&format!("https://{h}/")).unwrap())).collect();
        // The page shown is another site, so "the page's own host" adds nothing here.
        let js = format!(
            "const location = {{ hostname: 'unrelated.test' }};\n{}\nconsole.log(JSON.stringify({}.map(inApp)));",
            matcher_script(&cfg),
            serde_json::to_string(&hosts).unwrap()
        );
        let Ok(out) = std::process::Command::new("node").args(["-e", &js]).output() else {
            eprintln!("node not installed; skipping");
            return;
        };
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let got: Vec<bool> = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(got, expected, "hosts: {hosts:?}");
    }
}
