#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use penates_core::{export, icon, single, integration, resolve_name, normalize_url, ops, slugify, store, updates, WebApp};
use std::process::{Command, Stdio};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

// ---------- manager commands (IPC only available to the "manager" window) ----------
// Anything touching the network is `async` so it never blocks the UI thread.

type CmdResult<T> = Result<T, String>;
fn err<E: std::fmt::Display>(e: E) -> String {
    format!("{e:#}")
}

#[derive(serde::Serialize)]
struct Candidate {
    src: String,
    data: String,
}

#[tauri::command]
fn list_apps() -> CmdResult<Vec<WebApp>> {
    store::list().map_err(err)
}

/// Name suggested for a URL, used to prefill the Create form.
#[tauri::command]
async fn suggest_name(url: String) -> CmdResult<String> {
    let url = normalize_url(&url).map_err(err)?;
    Ok(resolve_name(&url))
}

#[tauri::command]
fn app_version() -> String {
    updates::CURRENT_VERSION.to_string()
}

#[tauri::command]
async fn create_app(
    url: String,
    name: Option<String>,
    open_after: bool,
    icon_url: Option<String>,
    icon_bytes: Option<Vec<u8>>,
) -> CmdResult<WebApp> {
    let url = normalize_url(&url).map_err(err)?;
    let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| resolve_name(&url));
    let base = slugify(&name);
    if base.is_empty() {
        return Err("Name must contain letters or digits".into());
    }
    let mut id = base.clone();
    let mut n = 2;
    while store::exists(&id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    // Icon: a chosen file or URL wins (and is never replaced by Update); otherwise the site's own.
    let (icon, custom_icon) = if let Some(b) = icon_bytes.filter(|b| !b.is_empty()) {
        if b.len() > 8 * 1024 * 1024 {
            return Err("image is larger than 8 MB".into());
        }
        (Some(icon::set_from_bytes(&b, &id).map_err(err)?), true)
    } else if let Some(u) = icon_url.map(|u| u.trim().to_string()).filter(|u| !u.is_empty()) {
        (Some(icon::set_from_src(&u, &id).map_err(|e| format!("icon URL: {e:#}"))?), true)
    } else {
        (icon::fetch_for(&url, &id).ok(), false)
    };
    let icon = icon.map(|p| store::icon_ref(&p));
    let mut app = WebApp { id, name, url, icon, custom_icon, ..Default::default() };
    ops::validate(&mut app).map_err(err)?;
    // Link to a catalog preset if one matches this site.
    if let Ok(m) = updates::load_manifest() {
        updates::apply_preset(&mut app, &m);
    }
    store::save(&app).map_err(err)?;
    ops::reinstall(None, &app);
    if open_after {
        run_app(app.id.clone())?;
    }
    Ok(app)
}

#[tauri::command]
fn save_app(app: WebApp) -> CmdResult<WebApp> {
    ops::save_edit(app).map_err(err)
}

#[tauri::command]
async fn update_app(id: String) -> CmdResult<ops::Outcome> {
    ops::update(&id).map_err(err)
}

#[tauri::command]
async fn update_all() -> CmdResult<Vec<ops::Outcome>> {
    ops::update_all().map_err(err)
}

#[tauri::command]
async fn check_updates() -> CmdResult<updates::UpdateReport> {
    updates::check().map_err(err)
}

#[tauri::command]
fn get_update_settings() -> updates::UpdateSettings {
    updates::load_settings().updates
}

#[tauri::command]
fn set_update_settings(settings: updates::UpdateSettings) -> CmdResult<()> {
    let mut s = updates::load_settings();
    s.updates = settings;
    updates::save_settings(&s).map_err(err)
}

#[tauri::command]
fn get_appearance() -> updates::Appearance {
    updates::load_settings().appearance
}

fn theme_of(name: &str) -> Option<tauri::Theme> {
    match name {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => None,
    }
}

#[tauri::command]
fn set_appearance(app: tauri::AppHandle, appearance: updates::Appearance) -> CmdResult<()> {
    if !matches!(appearance.theme.as_str(), "system" | "light" | "dark") {
        return Err("unknown theme".into());
    }
    if !["default", "nord", "solarized", "mono"].contains(&appearance.skin.as_str()) {
        return Err("unknown skin".into());
    }
    if !["blue", "violet", "green", "orange", "rose", "slate"].contains(&appearance.accent.as_str()) {
        return Err("unknown accent".into());
    }
    if let Some(w) = app.get_webview_window("manager") {
        let _ = w.set_theme(theme_of(&appearance.theme));
    }
    let mut s = updates::load_settings();
    s.appearance = appearance;
    updates::save_settings(&s).map_err(err)
}

/// Save an export into the Downloads folder (no file dialog needed). Empty `ids` = all apps.
#[tauri::command]
fn export_apps(ids: Vec<String>, with_settings: bool) -> CmdResult<String> {
    let f = export::build(&ids, with_settings).map_err(err)?;
    let name = match f.apps.as_slice() {
        [one] => format!("{}.penates", one.config.id),
        _ => "penates-backup.penates".to_string(),
    };
    let path = unique_download_path(std::path::Path::new(&name));
    std::fs::write(&path, serde_json::to_string_pretty(&f).map_err(err)?).map_err(err)?;
    Ok(path.display().to_string())
}

#[derive(serde::Serialize)]
struct Inspected {
    /// The raw text, handed back to `import_apps` after the user confirms.
    json: String,
    summary: Vec<export::Summary>,
    has_settings: bool,
}

/// Read an export (pasted file contents, or a URL) and describe it, without changing anything.
#[tauri::command]
async fn inspect_import(source: String, is_url: bool) -> CmdResult<Inspected> {
    let text = if is_url { export::read_source(source.trim()).map_err(err)? } else { source };
    let f = export::parse(&text).map_err(err)?;
    Ok(Inspected { summary: export::inspect(&f), has_settings: f.settings.is_some(), json: text })
}

#[tauri::command]
async fn import_apps(json: String, on_conflict: String, allow_js: bool, settings: bool) -> CmdResult<export::ImportReport> {
    let f = export::parse(&json).map_err(err)?;
    let opts = export::ImportOptions { on_conflict: on_conflict.parse().map_err(err)?, allow_js, settings };
    Ok(export::apply(&f, &opts))
}

#[derive(serde::Serialize)]
struct SelfUpdate {
    current: String,
    version: String,
    notes: String,
    /// The installed package type can update itself (AppImage, .deb, .rpm, NSIS/MSI, .app).
    /// Arch packages, tarballs and dev builds get a download link instead.
    can_install: bool,
    /// Installed by a package manager (e.g. the AUR package): update it there, not in-app.
    package_manager: bool,
    releases_page: String,
}

/// Ask the public `latest.json` whether a newer Penates exists (signature-checked on install).
#[tauri::command]
async fn check_self_update(app: tauri::AppHandle) -> CmdResult<Option<SelfUpdate>> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app.updater().map_err(err)?.check().await.map_err(err)?;
    Ok(update.map(|u| SelfUpdate {
        current: u.current_version.clone(),
        version: u.version.clone(),
        notes: u.body.clone().unwrap_or_default(),
        can_install: tauri::utils::platform::bundle_type().is_some(),
        package_manager: tauri::utils::platform::bundle_type().is_none()
            && std::env::current_exe().map(|p| p.starts_with("/usr/")).unwrap_or(false),
        releases_page: updates::RELEASES_PAGE.into(),
    }))
}

/// Download, verify the signature, install and restart.
#[tauri::command]
async fn install_self_update(app: tauri::AppHandle) -> CmdResult<()> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app.updater().map_err(err)?.check().await.map_err(err)?.ok_or("Penates is already up to date")?;
    update.download_and_install(|_, _| {}, || {}).await.map_err(err)?;
    app.restart();
}

#[tauri::command]
fn open_url(url: String) -> CmdResult<()> {
    let u = url::Url::parse(&url).map_err(err)?;
    if !matches!(u.scheme(), "http" | "https") {
        return Err("only http(s) links".into());
    }
    open::that_detached(u.as_str()).map_err(err)
}

#[tauri::command]
fn remove_app(id: String, purge: bool) -> CmdResult<()> {
    store::remove(&id, purge).map_err(err)
}

fn ensure_closed(id: &str) -> CmdResult<String> {
    let app = store::load(id).map_err(err)?;
    if single::is_running(id) {
        return Err(format!("{} is running. Close it first.", app.name));
    }
    Ok(app.name)
}

#[derive(serde::Serialize)]
struct CacheInfo {
    bytes: u64,
    running: bool,
}

#[tauri::command]
fn cache_info(id: String) -> CacheInfo {
    CacheInfo { bytes: store::cache_size(&id), running: single::is_running(&id) }
}

/// Delete cached files, keep logins. Returns bytes freed.
#[tauri::command]
fn clear_cache(id: String) -> CmdResult<u64> {
    ensure_closed(&id)?;
    Ok(store::clear_cache(&id))
}

/// Delete cookies, storage and cache (signs the app out).
#[tauri::command]
fn clear_data(id: String) -> CmdResult<()> {
    ensure_closed(&id)?;
    store::clear_profile(&id);
    Ok(())
}

#[tauri::command]
fn run_app(id: String) -> CmdResult<()> {
    store::load(&id).map_err(err)?;
    let exe = std::env::current_exe().map_err(err)?;
    Command::new(exe).args(["--run", &id]).stdin(Stdio::null()).spawn().map_err(err)?;
    Ok(())
}

#[tauri::command]
fn icon_data(path: String) -> CmdResult<String> {
    // `path` is an icon file name (older configs: an absolute path); only the name is used and
    // it is always resolved inside our icon folder.
    let name = std::path::Path::new(&path).file_name().ok_or("bad icon name")?;
    Ok(png_data_url(&std::fs::read(store::icon_dir().join(name)).map_err(err)?))
}

#[tauri::command]
async fn icon_candidates(url: String) -> CmdResult<Vec<Candidate>> {
    Ok(icon::preview_candidates(&url)
        .into_iter()
        .map(|c| Candidate { data: png_data_url(&c.png), src: c.src })
        .collect())
}

#[tauri::command]
async fn set_icon_from_candidate(id: String, src: String) -> CmdResult<WebApp> {
    let p = icon::set_from_src(&src, &id).map_err(err)?;
    ops::set_icon_file(&id, p, true).map_err(err)
}

#[tauri::command]
async fn set_icon_from_url(id: String, url: String) -> CmdResult<WebApp> {
    let p = icon::set_from_src(url.trim(), &id).map_err(err)?;
    ops::set_icon_file(&id, p, true).map_err(err)
}

#[tauri::command]
fn set_icon_from_bytes(id: String, bytes: Vec<u8>) -> CmdResult<WebApp> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("image is larger than 8 MB".into());
    }
    let p = icon::set_from_bytes(&bytes, &id).map_err(err)?;
    ops::set_icon_file(&id, p, true).map_err(err)
}

#[tauri::command]
async fn reset_icon(id: String) -> CmdResult<WebApp> {
    ops::reset_icon(&id).map_err(err)
}

fn png_data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", b64(bytes))
}

fn b64(d: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::new();
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        o.push(T[(n >> 18) as usize & 63] as char);
        o.push(T[(n >> 12) as usize & 63] as char);
        o.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        o.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    o
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

// ---------- runner: one wrapped web app ----------

/// Save downloads to the user's Downloads folder, never overwriting an existing file.
fn unique_download_path(suggested: &std::path::Path) -> std::path::PathBuf {
    let name = suggested.file_name().map(|n| n.to_os_string()).unwrap_or_else(|| "download".into());
    let dir = dirs::download_dir().or_else(dirs::home_dir).unwrap_or_else(std::env::temp_dir);
    let first = dir.join(&name);
    if !first.exists() {
        return first;
    }
    let p = std::path::Path::new(&name);
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|c| !c.exists())
        .expect("unbounded range")
}

/// The macro embeds the app's Info.plist as a global symbol, so it may only be expanded once
/// (twice is a hard link error on macOS).
fn app_context() -> tauri::Context {
    tauri::generate_context!()
}

/// Hand a link the app won't load itself to the OS, but only for schemes a browser would open
/// without asking. `file:`, `smb:` and custom app schemes are dropped: a site must not be able to
/// make the desktop open local files or launch other programs.
fn open_external(u: &url::Url) {
    if matches!(u.scheme(), "http" | "https" | "mailto" | "tel") {
        let _ = open::that_detached(u.as_str());
    } else {
        eprintln!("blocked opening {}: link", u.scheme());
    }
}

/// WebKitGTK denies every permission request the app does not answer, and ships with camera and
/// WebRTC switched off. Answer notification and camera/microphone requests from the app's settings;
/// deny everything else (location, pointer lock, etc.).
#[cfg(target_os = "linux")]
fn apply_site_permissions(win: &tauri::WebviewWindow, cfg: &WebApp) {
    let (notify, media) = (cfg.allow_notifications, cfg.allow_camera_microphone);
    let _ = win.with_webview(move |wv| {
        use gtk::glib::prelude::*;
        use webkit2gtk::{NotificationPermissionRequest, PermissionRequestExt, SettingsExt, UserMediaPermissionRequest, WebViewExt};
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
            } else if req.is::<UserMediaPermissionRequest>() {
                media
            } else {
                false
            };
            if allow { req.allow() } else { req.deny() }
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

fn run_webapp(cfg: WebApp) -> tauri::Result<()> {
    // Taskbar identity: own WM class / app_id (Linux) and own AppUserModelID (Windows), so each
    // app gets its own taskbar entry matching its launcher and icon.
    #[cfg(target_os = "linux")]
    gtk::glib::set_prgname(Some(&integration::wm_class(&cfg.id)));
    #[cfg(windows)]
    set_aumid(&cfg.id);
    // Running straight from the CLI/dev: make sure a launcher (and icon) exists.
    if !integration::is_installed(&cfg) {
        ops::reinstall(None, &cfg);
    }

    // Every running app owns a loopback port (so the manager can tell it is running). In
    // single-window mode a second launch focuses the first window and exits.
    let listener = single::try_bind(&cfg.id);
    if listener.is_none() && cfg.single_instance && single::request_focus(&cfg.id) {
        return Ok(());
    }
    let mut listener = listener;

    tauri::Builder::default()
        .setup(move |app| {
            let url = cfg.url.parse::<url::Url>().expect("validated url");
            let nav_cfg = cfg.clone();
            let mut js = String::new();
            if let Some(css) = &cfg.inject_css {
                let lit = serde_json::to_string(css).unwrap();
                js.push_str(&format!(
                    "document.addEventListener('DOMContentLoaded',()=>{{const s=document.createElement('style');s.textContent={lit};document.head.appendChild(s);}});\n"
                ));
            }
            if let Some(extra) = &cfg.inject_js {
                js.push_str(extra);
            }

            let mut w = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title(&cfg.name)
                .theme(theme_of(&cfg.theme))
                .inner_size(cfg.width, cfg.height)
                .maximized(cfg.start_maximized)
                .fullscreen(cfg.start_fullscreen)
                .resizable(cfg.resizable)
                .always_on_top(cfg.always_on_top)
                .decorations(cfg.decorations)
                .devtools(cfg.devtools)
                .initialization_script(&js)
                .on_new_window({
                    let c = cfg.clone();
                    move |u, _features| {
                        // Popups to the app's own sites (OAuth, "open in new window") keep their opener;
                        // anything else goes to the system browser.
                        if c.is_internal(&u) {
                            tauri::webview::NewWindowResponse::Allow
                        } else {
                            open_external(&u);
                            tauri::webview::NewWindowResponse::Deny
                        }
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
                .on_navigation(move |u| {
                    if nav_cfg.is_internal(u) {
                        true
                    } else {
                        open_external(u);
                        false
                    }
                });
            // Isolated profile per app. macOS WKWebView uses a data-store id instead of a directory.
            #[cfg(target_os = "macos")]
            {
                w = w.data_store_identifier(id_bytes(&cfg.id));
            }
            #[cfg(not(target_os = "macos"))]
            {
                w = w.data_directory(store::profile_dir(&cfg.id));
            }
            if let Some(ua) = &cfg.user_agent {
                w = w.user_agent(ua);
            }
            let win = w.build()?;
            #[cfg(target_os = "linux")]
            apply_site_permissions(&win, &cfg);
            if (cfg.zoom - 1.0).abs() > f64::EPSILON {
                let _ = win.set_zoom(cfg.zoom);
            }
            if cfg.hide_menu_bar {
                let _ = win.remove_menu();
            }
            if let Some(icon) = cfg.icon_path() {
                if let Ok(img) = tauri::image::Image::from_path(icon) {
                    let _ = win.set_icon(img);
                }
            }
            if cfg.tray {
                if let Err(e) = build_tray(app, &cfg) {
                    eprintln!("tray: {e}");
                }
                if cfg.close_to_tray {
                    let w2 = win.clone();
                    win.on_window_event(move |ev| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
                            api.prevent_close();
                            let _ = w2.hide();
                        }
                    });
                }
            }
            if let Some(l) = listener.take() {
                let handle = app.handle().clone();
                single::serve(l, cfg.id.clone(), move || {
                    if let Some(w) = handle.get_webview_window("main") {
                        let _ = w.unminimize();
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                });
            }
            Ok(())
        })
        .run(app_context())
}

// ---------- manager ----------

#[cfg(windows)]
fn set_aumid(id: &str) {
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    let w: Vec<u16> = format!("dev.penates.app.{id}").encode_utf16().chain(Some(0)).collect();
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(w.as_ptr());
    }
}

fn run_manager(start: Option<String>) -> tauri::Result<()> {
    // The window's app_id is "penates" and must equal a launcher file name (penates.desktop) for the
    // desktop to show the right icon. Packages ship that file; dev runs write it.
    #[cfg(target_os = "linux")]
    gtk::glib::set_prgname(Some("penates"));
    #[cfg(all(target_os = "linux", debug_assertions))]
    {
        if let Ok(exe) = std::env::current_exe() {
            let _ = integration::install_manager(&exe, include_bytes!("../icons/icon.png"));
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            list_apps, app_version, suggest_name, create_app, save_app, update_app, update_all, check_updates,
            get_update_settings, set_update_settings, get_appearance, set_appearance, open_url, remove_app, cache_info, clear_cache, clear_data, run_app, export_apps, inspect_import, import_apps,
            check_self_update, install_self_update, icon_data, icon_candidates, set_icon_from_candidate, set_icon_from_url, set_icon_from_bytes, reset_icon
        ])
        .setup(move |app| {
            // `--tab settings` / `--tab edit:<id>` opens the manager on that view.
            let init = start.as_deref().map(|t| format!("window.__PENATES_START = {};", serde_json::to_string(t).unwrap())).unwrap_or_default();
            WebviewWindowBuilder::new(app, "manager", WebviewUrl::App("index.html".into()))
                .initialization_script(&init)
                .title("Penates")
                .theme(theme_of(&updates::load_settings().appearance.theme))
                .inner_size(860.0, 720.0)
                .build()?;
            Ok(())
        })
        .run(app_context())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.iter().position(|a| a == "--run") {
        Some(i) => {
            let Some(id) = args.get(i + 1) else {
                eprintln!("usage: penates-desktop --run <id>");
                std::process::exit(2);
            };
            match store::load(id) {
                Ok(cfg) => run_webapp(cfg),
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }
        None => run_manager(args.iter().position(|a| a == "--tab").and_then(|i| args.get(i + 1)).cloned()),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
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
}
