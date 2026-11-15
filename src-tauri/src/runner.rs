//! The runner: one wrapped web app in its own window (`penates-desktop --run <id>`). It registers
//! no IPC commands or plugins; the site gets nothing from Penates but a window.

use penates_core::{icon, integration, ops, single, store, WebApp};
use std::sync::{Arc, Mutex};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

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
fn apply_site_permissions(win: &tauri::WebviewWindow, cfg: &WebApp) {
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
    if let Some(w) = app.get_webview_window("main") {
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

/// The rules every window of a wrapped app gets: its main window and any popup it opens.
fn with_rules<'a, M: Manager<tauri::Wry>>(
    mut b: WebviewWindowBuilder<'a, tauri::Wry, M>,
    cfg: &WebApp,
    visited: &Visited,
    handle: tauri::AppHandle,
) -> WebviewWindowBuilder<'a, tauri::Wry, M> {
    if cfg.external_links_in_browser {
        b = b.initialization_script(link_guard_script(cfg));
    }
    if let Some(ua) = &cfg.user_agent {
        b = b.user_agent(ua);
    }
    let (popup_cfg, visited) = (cfg.clone(), visited.clone());
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
    match with_rules(b, cfg, visited, handle.clone()).build() {
        Ok(w) => Some(w),
        Err(e) => {
            eprintln!("popup: {e}");
            None
        }
    }
}

/// The app's main window, with its profile, injected CSS/JS and link rules.
fn build_main_window(app: &tauri::App, cfg: &WebApp, visited: &Visited) -> tauri::Result<tauri::WebviewWindow> {
    let url = cfg.url.parse::<url::Url>().map_err(|e| anyhow::anyhow!("invalid URL in config: {e}"))?;
    let mut w = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title(&cfg.name)
        .theme(theme_of(&cfg.theme))
        .inner_size(cfg.width, cfg.height)
        .maximized(cfg.start_maximized)
        .fullscreen(cfg.start_fullscreen)
        .resizable(cfg.resizable)
        .always_on_top(cfg.always_on_top)
        .decorations(cfg.decorations)
        .devtools(cfg.devtools);
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
    let seen = visited.clone();
    w = with_rules(w, cfg, visited, app.handle().clone()).on_page_load(move |_, payload| remember(&seen, payload.url()));
    // Isolated profile per app. macOS WKWebView uses a data-store id instead of a directory.
    #[cfg(target_os = "macos")]
    {
        w = w.data_store_identifier(id_bytes(&cfg.id));
    }
    #[cfg(not(target_os = "macos"))]
    {
        w = w.data_directory(store::profile_dir(&cfg.id));
    }
    w.build()
}

/// Closing the window hides it; the tray menu shows it again or quits.
fn enable_close_to_tray(win: &tauri::WebviewWindow) {
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
            if let Some(w) = HANDLE.get().and_then(|h| h.get_webview_window("main")) {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }),
        None if cfg.single_instance && single::request_focus(&cfg.id) => return Ok(()),
        None => {}
    }

    tauri::Builder::default()
        .setup(move |app| {
            let _ = HANDLE.set(app.handle().clone());
            let visited: Visited = Arc::default();
            let win = build_main_window(app, &cfg, &visited)?;
            #[cfg(target_os = "linux")]
            apply_site_permissions(&win, &cfg);
            if (cfg.zoom - 1.0).abs() > f64::EPSILON {
                let _ = win.set_zoom(cfg.zoom);
            }
            if let Some(img) = cfg.icon_path().and_then(|p| tauri::image::Image::from_path(p).ok()) {
                let _ = win.set_icon(img);
            }
            if cfg.tray {
                match build_tray(app, &cfg) {
                    Ok(()) if cfg.close_to_tray => enable_close_to_tray(&win),
                    Ok(()) => {}
                    Err(e) => eprintln!("tray: {e}"), // no tray, so closing must really close
                }
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
        // Regression: this overflowed (panicked) in debug builds on macOS.
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
