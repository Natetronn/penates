//! IPC commands for the manager window (the only window with IPC; see capabilities/default.json).
//! Anything that touches the network or the disk runs on a blocking thread, so the window never
//! waits on it.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use penates_core::{export, icon, integration, normalize_url, ops, settings, single, site::Page, store, updates, WebApp};
use std::process::{Command, Stdio};
use tauri::Manager;

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    format!("{e:#}")
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
    updates::CURRENT_VERSION.to_string()
}

/// Name suggested for a URL, used to prefill the Create form.
#[tauri::command]
pub async fn suggest_name(url: String) -> CmdResult<String> {
    blocking(move || {
        let url = normalize_url(&url)?;
        Ok(Page::fetch(&url).ok().and_then(|p| p.name()).unwrap_or_else(|| penates_core::name_from_url(&url)))
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
) -> CmdResult<ops::Created> {
    let icon = match (icon_bytes.filter(|b| !b.is_empty()), icon_url.map(|u| u.trim().to_string()).filter(|u| !u.is_empty())) {
        (Some(b), _) => ops::IconChoice::Bytes(b),
        (None, Some(u)) if u.starts_with("https://") || u.starts_with("http://") => ops::IconChoice::Source(u),
        (None, Some(_)) => return Err("The icon address must start with https:// or http://".into()),
        (None, None) => ops::IconChoice::Site,
    };
    let created = blocking(move || ops::create(ops::NewApp { url, name, icon, ..Default::default() })).await?;
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
    Command::new(exe).args(["--run", &id]).stdin(Stdio::null()).spawn().map_err(err)?;
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
    // `path` is an icon file name (older configs: an absolute path); only the name is used and it is
    // always resolved inside our icon folder.
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

/// The app catalog's apps, for browsing (Create → Popular apps).
#[tauri::command]
pub async fn catalog_apps() -> CmdResult<updates::CatalogListing> {
    blocking(updates::catalog_listing).await
}

/// The icon of a catalog app, for the Popular apps list (cached; see `icon::catalog_icon`).
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

#[tauri::command]
pub fn set_appearance(app: tauri::AppHandle, appearance: settings::Appearance) -> CmdResult<()> {
    appearance.validate().map_err(err)?;
    if let Some(w) = app.get_webview_window("manager") {
        let _ = w.set_theme(crate::runner::theme_of(&appearance.theme));
    }
    settings::update(|s| s.appearance = appearance).map(|_| ()).map_err(err)
}

// ---- sharing ----

/// Save an export into the Downloads folder (no file dialog needed). Empty `ids` = all apps.
#[tauri::command]
pub async fn export_apps(ids: Vec<String>, with_settings: bool) -> CmdResult<String> {
    blocking(move || {
        let f = export::build(&ids, with_settings)?;
        let name = match f.apps.as_slice() {
            [one] => format!("{}.penates", one.config.id),
            _ => "penates-backup.penates".to_string(),
        };
        let path = crate::runner::unique_download_path(std::path::Path::new(&name));
        std::fs::write(&path, serde_json::to_string_pretty(&f)?)?;
        Ok(path.display().to_string())
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

#[tauri::command]
pub fn open_url(url: String) -> CmdResult<()> {
    let u = url::Url::parse(&url).map_err(err)?;
    if !matches!(u.scheme(), "http" | "https") {
        return Err("only http(s) links".into());
    }
    open::that_detached(u.as_str()).map_err(err)
}
