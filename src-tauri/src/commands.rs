//! IPC commands for the manager window (the only window with IPC; see capabilities/default.json).
//! Anything that touches the network or the disk runs on a blocking thread, so the window never
//! waits on it.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use penates_core::{export, icon, integration, normalize_url, ops, settings, single, site::Page, store, updates, WebApp};
use std::process::{Command, Stdio};
use tauri::Manager;

type CmdResult<T> = Result<T, String>;

/// A command's failure, as the manager shows it. Also logged, so a problem report has it.
fn err<E: std::fmt::Display>(e: E) -> String {
    let msg = format!("{e:#}");
    penates_core::log::write(penates_core::log::Level::Error, &msg);
    msg
}

/// Run blocking work off the async runtime's worker threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?.map_err(err)
}

fn png_data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", B64.encode(bytes))
}

// ---- apps ----

#[derive(serde::Serialize)]
pub struct AppList {
    apps: Vec<WebApp>,
    /// Configs that exist but do not load; shown so they can be removed.
    broken: Vec<store::Broken>,
}

#[tauri::command]
pub async fn list_apps() -> CmdResult<AppList> {
    blocking(|| {
        let (apps, broken) = store::scan();
        Ok(AppList { apps, broken })
    })
    .await
}

#[tauri::command]
pub fn app_version() -> String {
    updates::BUILD_VERSION.to_string()
}

/// Name suggested for a URL, used to prefill the Create form.
#[tauri::command]
pub async fn suggest_name(url: String) -> CmdResult<String> {
    blocking(move || {
        let url = normalize_url(&url)?;
        let name = Page::fetch(&url).ok().and_then(|p| p.name()).unwrap_or_else(|| penates_core::name_from_url(&url));
        Ok(ops::unique_name(&name)) // "GitHub 2" when you already have a GitHub app
    })
    .await
}

#[tauri::command]
pub async fn create_app(
    url: String,
    name: Option<String>,
    open_after: bool,
    icon_url: Option<String>,
    icon_bytes: Option<Vec<u8>>,
    toolbar: Option<bool>,
) -> CmdResult<ops::Created> {
    let icon = match (icon_bytes.filter(|b| !b.is_empty()), icon_url.map(|u| u.trim().to_string()).filter(|u| !u.is_empty())) {
        (Some(b), _) => ops::IconChoice::Bytes(b),
        (None, Some(u)) if u.starts_with("https://") || u.starts_with("http://") => ops::IconChoice::Source(u),
        (None, Some(_)) => return Err("The icon address must start with https:// or http://".into()),
        (None, None) => ops::IconChoice::Site,
    };
    let created = blocking(move || ops::create(ops::NewApp { url, name, icon, toolbar: toolbar.unwrap_or(true), ..Default::default() })).await?;
    if open_after {
        run_app(created.app.id.clone())?;
    }
    Ok(created)
}

#[tauri::command]
pub async fn save_app(app: WebApp) -> CmdResult<ops::Saved> {
    blocking(move || ops::save_edit(app)).await
}

#[tauri::command]
pub async fn remove_app(id: String, purge: bool) -> CmdResult<()> {
    blocking(move || ops::remove(&id, purge)).await
}

#[tauri::command]
pub fn run_app(id: String) -> CmdResult<()> {
    store::load(&id).map_err(err)?;
    // Inside an AppImage, current_exe() lives in a mount that disappears when the manager quits.
    let exe = integration::find_desktop_bin().map(Ok).unwrap_or_else(std::env::current_exe).map_err(err)?;
    let mut child = Command::new(exe).args(["--run", &id]).stdin(Stdio::null()).spawn().map_err(err)?;
    // Collect the app's exit status when it closes, so it does not linger as a defunct process.
    std::thread::spawn(move || child.wait());
    Ok(())
}

// ---- data ----

#[derive(serde::Serialize)]
pub struct CacheInfo {
    bytes: u64,
    running: bool,
}

#[tauri::command]
pub async fn cache_info(id: String) -> CmdResult<CacheInfo> {
    blocking(move || Ok(CacheInfo { bytes: store::cache_size(&id), running: single::is_running(&id) })).await
}

/// Delete cached files, keep logins. Returns bytes freed.
#[tauri::command]
pub async fn clear_cache(id: String) -> CmdResult<u64> {
    blocking(move || {
        ops::ensure_closed(&id)?;
        Ok(store::clear_cache(&id))
    })
    .await
}

/// Delete cookies, storage and cache (signs the app out).
#[tauri::command]
pub async fn clear_data(id: String) -> CmdResult<()> {
    blocking(move || {
        ops::ensure_closed(&id)?;
        store::clear_profile(&id)
    })
    .await
}

// ---- icons ----

#[tauri::command]
pub async fn icon_data(path: String) -> CmdResult<String> {
    // `path` is an icon file name (a full path also works): only the name is used, and it is always
    // resolved inside our icon folder.
    blocking(move || {
        let name = std::path::Path::new(&path).file_name().ok_or_else(|| anyhow::anyhow!("bad icon name"))?.to_owned();
        Ok(png_data_url(&std::fs::read(store::icon_dir().join(name))?))
    })
    .await
}

#[derive(serde::Serialize)]
pub struct Candidate {
    src: String,
    data: String,
}

#[tauri::command]
pub async fn icon_candidates(url: String) -> CmdResult<Vec<Candidate>> {
    blocking(move || {
        Ok(icon::preview_candidates(&normalize_url(&url)?)
            .into_iter()
            .map(|c| Candidate { data: png_data_url(&c.png), src: c.src })
            .collect())
    })
    .await
}

/// Use an image address (a found candidate or one the person pasted) as the app's icon.
#[tauri::command]
pub async fn set_icon_from_url(id: String, url: String) -> CmdResult<ops::Saved> {
    blocking(move || {
        let p = icon::set_from_src(url.trim(), &id)?;
        ops::set_icon_file(&id, p, true)
    })
    .await
}

#[tauri::command]
pub async fn set_icon_from_bytes(id: String, bytes: Vec<u8>) -> CmdResult<ops::Saved> {
    blocking(move || {
        let p = icon::set_from_bytes(&bytes, &id)?;
        ops::set_icon_file(&id, p, true)
    })
    .await
}

#[tauri::command]
pub async fn reset_icon(id: String) -> CmdResult<ops::Saved> {
    blocking(move || ops::reset_icon(&id)).await
}

// ---- updates ----

#[tauri::command]
pub async fn update_app(id: String) -> CmdResult<ops::Outcome> {
    blocking(move || ops::update(&id)).await
}

#[tauri::command]
pub async fn update_all() -> CmdResult<ops::UpdateAll> {
    blocking(|| Ok(ops::update_all())).await
}

/// The app catalog's apps, for browsing (the Catalog tab).
#[tauri::command]
pub async fn catalog_apps() -> CmdResult<updates::CatalogListing> {
    blocking(updates::catalog_listing).await
}

/// Read one catalog before it is added to Settings: how many apps it has, or why it cannot be used.
#[tauri::command]
pub async fn check_catalog(source: String) -> CmdResult<usize> {
    blocking(move || updates::read_source(&source).map(|c| c.catalog.len())).await
}

/// Prepare a catalog submission for one of your apps (shown for review before anything opens).
#[tauri::command]
pub async fn catalog_submission(id: String, notes: String) -> CmdResult<updates::Submission> {
    blocking(move || updates::submission(&store::load(&id)?, &notes)).await
}

/// The icon of a catalog app, for the Catalog tab (cached; see `icon::catalog_icon`).
#[tauri::command]
pub async fn catalog_icon(id: String, url: String, icon_url: Option<String>) -> CmdResult<String> {
    blocking(move || Ok(png_data_url(&icon::catalog_icon(&id, &url, icon_url.as_deref())?))).await
}

/// Apps with a newer catalog preset. (Penates's own version: `check_self_update`.)
#[tauri::command]
pub async fn check_apps() -> CmdResult<updates::AppsReport> {
    blocking(updates::check_apps).await
}

#[derive(serde::Serialize)]
pub struct SelfUpdate {
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
pub async fn check_self_update(app: tauri::AppHandle) -> CmdResult<Option<SelfUpdate>> {
    use tauri_plugin_updater::UpdaterExt;
    let update = match app.updater().map_err(err)?.check().await {
        Ok(u) => u,
        // Before the first release there is no latest.json at all: nothing newer, not an error.
        Err(e) => match blocking(updates::check_penates).await {
            Ok(None) => None,
            _ => return Err(err(e)),
        },
    };
    let bundled = tauri::utils::platform::bundle_type().is_some();
    Ok(update.map(|u| SelfUpdate {
        current: u.current_version.clone(),
        version: u.version.clone(),
        notes: u.body.clone().unwrap_or_default(),
        can_install: bundled,
        package_manager: !bundled
            && std::env::current_exe().map(|p| p.starts_with("/usr/") && !p.starts_with("/usr/local/")).unwrap_or(false),
        releases_page: updates::RELEASES_PAGE.into(),
    }))
}

/// Download, verify the signature, install and restart.
#[tauri::command]
pub async fn install_self_update(app: tauri::AppHandle) -> CmdResult<()> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app.updater().map_err(err)?.check().await.map_err(err)?.ok_or("Penates is already up to date")?;
    update.download_and_install(|_, _| {}, || {}).await.map_err(err)?;
    app.restart();
}

// ---- settings ----

#[tauri::command]
pub fn get_update_settings() -> settings::UpdateSettings {
    settings::load().updates
}

#[tauri::command]
pub fn set_update_settings(updates: settings::UpdateSettings) -> CmdResult<()> {
    settings::update(|s| s.updates = updates).map(|_| ()).map_err(err)
}

#[tauri::command]
pub fn get_appearance() -> settings::Appearance {
    settings::load().appearance
}

/// The desktop's light/dark choice, from the XDG settings portal (KDE, GNOME and others implement it).
/// None when there is no preference or no portal.
#[cfg(target_os = "linux")]
fn desktop_theme() -> Option<tauri::Theme> {
    use gtk::gio;
    use gtk::glib::prelude::*;
    let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()?;
    let read = |method: &str| {
        bus.call_sync(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            method,
            Some(&("org.freedesktop.appearance", "color-scheme").to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            1000,
            gio::Cancellable::NONE,
        )
        .ok()
    };
    // ReadOne answers (v: u); the older Read answers (v: v: u).
    let value = read("ReadOne").or_else(|| read("Read"))?.child_value(0);
    let mut inner = value.as_variant()?;
    if let Some(nested) = inner.as_variant() {
        inner = nested;
    }
    match inner.get::<u32>()? {
        1 => Some(tauri::Theme::Dark),
        2 => Some(tauri::Theme::Light),
        _ => None,
    }
}

/// The window theme to set for a look. "system" has to be spelled out on Linux: there, "no theme"
/// switches GTK's dark preference off (forcing light) instead of following the desktop.
fn window_theme(theme: &str) -> Option<tauri::Theme> {
    match crate::runner::theme_of(theme) {
        Some(t) => Some(t),
        #[cfg(target_os = "linux")]
        None => Some(desktop_theme().unwrap_or(tauri::Theme::Light)),
        #[cfg(not(target_os = "linux"))]
        None => None,
    }
}

#[tauri::command]
pub fn set_appearance(app: tauri::AppHandle, appearance: settings::Appearance) -> CmdResult<()> {
    appearance.validate().map_err(err)?;
    if let Some(w) = app.get_webview_window("manager") {
        let _ = w.set_theme(window_theme(&appearance.theme));
    }
    settings::update(|s| s.appearance = appearance).map(|_| ()).map_err(err)
}

// ---- sharing ----

/// Save an export into the export folder (Settings; Downloads unless chosen), never overwriting a file.
/// Empty `ids` = all apps.
#[tauri::command]
pub async fn export_apps(ids: Vec<String>, with_settings: bool) -> CmdResult<String> {
    blocking(move || {
        let f = export::build(&ids, with_settings)?;
        let name = match f.apps.as_slice() {
            [one] => format!("{}.penates", one.config.id),
            _ => "penates-backup.penates".to_string(),
        };
        let dir = settings::load().sharing.export_folder();
        let path = crate::runner::unique_path_in(&dir, std::path::Path::new(&name));
        std::fs::write(&path, serde_json::to_string_pretty(&f)?)?;
        Ok(path.display().to_string())
    })
    .await
}

#[derive(serde::Serialize)]
pub struct ExportFolder {
    /// The folder chosen in Settings; empty for Downloads.
    chosen: String,
    /// The folder exports go to now (the chosen one, or Downloads when none is chosen or it is gone).
    folder: String,
}

fn export_folder_info() -> ExportFolder {
    let s = settings::load().sharing;
    ExportFolder { folder: s.export_folder().display().to_string(), chosen: s.export_dir }
}

#[tauri::command]
pub fn get_export_folder() -> ExportFolder {
    export_folder_info()
}

/// Set the export folder: an existing absolute folder, or "" for Downloads.
#[tauri::command]
pub fn set_export_folder(dir: String) -> CmdResult<ExportFolder> {
    let dir = dir.trim().to_string();
    if !dir.is_empty() {
        let p = std::path::Path::new(&dir);
        if !p.is_absolute() || !p.is_dir() {
            return Err(format!("{dir} is not a folder"));
        }
    }
    settings::update(|s| s.sharing.export_dir = dir).map_err(err)?;
    Ok(export_folder_info())
}

/// Ask for a folder with the system's own folder picker; None when cancelled. Only this command opens
/// dialogs: the manager page has no access to the dialog plugin itself.
#[tauri::command]
pub async fn pick_export_folder(app: tauri::AppHandle) -> CmdResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let start = settings::load().sharing.export_folder();
    blocking(move || {
        Ok(app.dialog().file().set_title("Folder for Exports and Backups").set_directory(start).blocking_pick_folder()
            .and_then(|f| f.into_path().ok()).map(|p| p.display().to_string()))
    })
    .await
}

/// A file chosen in the system's file dialog: its name and contents.
#[derive(serde::Serialize)]
pub struct PickedFile {
    name: String,
    bytes: Vec<u8>,
}

/// Choose a file with the system's file dialog and read it: `kind` "image" (an icon, starting in your home
/// folder) or "penates" (an export to import, starting in your export folder). Every file picker in Penates is
/// the system dialog, so each starts in a useful folder (a web page's file input cannot choose one, and GTK
/// opens it in Recent, often empty) and filters by file name (a web page's input on Linux filters by MIME
/// type only, which hides `.penates` files). None when cancelled.
#[tauri::command]
pub async fn pick_file(app: tauri::AppHandle, kind: String) -> CmdResult<Option<PickedFile>> {
    use tauri_plugin_dialog::DialogExt;
    const MAX: u64 = 20 * 1024 * 1024;
    blocking(move || {
        let dialog = app.dialog().file();
        let dialog = match kind.as_str() {
            "image" => dialog.set_title("Choose an Icon Image").add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif", "ico"]),
            "penates" => dialog
                .set_title("Choose a Penates File")
                .set_directory(settings::load().sharing.export_folder())
                .add_filter("Penates files", &["penates"])
                .add_filter("All files", &["*"]),
            _ => anyhow::bail!("unknown kind of file '{kind}'"),
        };
        let dialog = if kind == "image" { home_start(dialog) } else { dialog };
        let Some(path) = dialog.blocking_pick_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
        anyhow::ensure!(std::fs::metadata(&path)?.len() <= MAX, "that file is over 20 MB");
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(Some(PickedFile { name, bytes: std::fs::read(&path)? }))
    })
    .await
}

/// Start a file dialog in the home folder.
fn home_start<R: tauri::Runtime>(d: tauri_plugin_dialog::FileDialogBuilder<R>) -> tauri_plugin_dialog::FileDialogBuilder<R> {
    match dirs::home_dir() {
        Some(h) => d.set_directory(h),
        None => d,
    }
}

/// Choose a catalog file for Settings → Updates → Your catalogs (None when cancelled; starts in your home folder).
#[tauri::command]
pub async fn pick_catalog_file(app: tauri::AppHandle) -> CmdResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    blocking(move || {
        Ok(home_start(app.dialog().file()).set_title("Choose an App Catalog").add_filter("App catalogs", &["json"]).blocking_pick_file()
            .and_then(|f| f.into_path().ok()).map(|p| p.display().to_string()))
    })
    .await
}

#[derive(serde::Serialize)]
pub struct Inspected {
    /// The raw text, handed back to `import_apps` after the person confirms.
    json: String,
    summary: Vec<export::Summary>,
    has_settings: bool,
}

/// Read an export (file contents, or a URL) and describe it, without changing anything.
#[tauri::command]
pub async fn inspect_import(source: String, is_url: bool) -> CmdResult<Inspected> {
    blocking(move || {
        let text = if is_url { export::read_source(source.trim())? } else { source };
        let f = export::parse(&text)?;
        Ok(Inspected { summary: export::inspect(&f), has_settings: f.settings.is_some(), json: text })
    })
    .await
}

#[tauri::command]
pub async fn import_apps(json: String, on_conflict: String, allow_js: bool, settings: bool) -> CmdResult<export::ImportReport> {
    blocking(move || {
        let f = export::parse(&json)?;
        Ok(export::apply(&f, &export::ImportOptions { on_conflict: on_conflict.parse()?, allow_js, settings }))
    })
    .await
}

// ---- problem reports ----

/// The web view's name and version, e.g. "WebKitGTK 2.48.3".
fn webview() -> Option<String> {
    let name = if cfg!(windows) { "WebView2" } else if cfg!(target_os = "macos") { "WKWebView" } else { "WebKitGTK" };
    tauri::webview_version().ok().map(|v| format!("{name} {v}"))
}

/// A problem report to review: the version, system and recent log lines, personal details removed.
#[tauri::command]
pub async fn problem_report(what: String) -> CmdResult<penates_core::report::Report> {
    blocking(move || Ok(penates_core::report::build(&what, webview().as_deref()))).await
}

/// An error in the manager's own page that nothing caught.
#[tauri::command]
pub fn log_ui_error(message: String) {
    penates_core::log::write(penates_core::log::Level::Error, &format!("page: {}", message.chars().take(1000).collect::<String>()));
}

#[tauri::command]
pub fn clear_log() -> CmdResult<()> {
    penates_core::log::clear().map_err(|e| format!("{e:#}"))
}

/// Show the folder with the log in the file manager.
#[tauri::command]
pub fn open_log_folder() -> CmdResult<()> {
    let dir = penates_core::log::dir();
    std::fs::create_dir_all(&dir).map_err(err)?;
    open::that_detached(&dir).map_err(err)
}

#[tauri::command]
pub fn open_url(url: String) -> CmdResult<()> {
    let u = url::Url::parse(&url).map_err(err)?;
    if !matches!(u.scheme(), "http" | "https") {
        return Err("only http(s) links".into());
    }
    open::that_detached(u.as_str()).map_err(err)
}
