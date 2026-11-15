//! High-level operations shared by the CLI and the desktop manager. Both front ends call these, so
//! an app created, edited, updated or removed from either one ends up the same.

use crate::{icon, integration, normalize_url, settings, single, site::Page, store, updates, WebApp};
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use std::path::PathBuf;

// ---------------------------------------------------------------- launchers

/// Write one launcher without refreshing the desktop's caches (callers refresh once).
pub fn reinstall_quiet(old: Option<&WebApp>, app: &WebApp) -> Result<Option<PathBuf>> {
    if let Some(o) = old {
        integration::uninstall_quiet(o); // its file name may change with the name
    }
    if !app.launcher {
        integration::uninstall_quiet(app);
        return Ok(None);
    }
    let bin = integration::find_desktop_bin()
        .context("penates-desktop not found (put it next to `penates`, on PATH, or set PENATES_DESKTOP)")?;
    integration::install_quiet(app, &bin).map(Some)
}

/// Rewrite the app's launcher after a change, removing `old`'s first. Apps with `launcher = false`
/// get theirs removed. Returns the launcher's path, if one was written.
pub fn reinstall(old: Option<&WebApp>, app: &WebApp) -> Result<Option<PathBuf>> {
    let r = reinstall_quiet(old, app);
    integration::refresh_caches();
    r
}

/// Rewrite many launchers with one cache refresh at the end. Returns the ones that failed.
pub fn reinstall_all(apps: &[WebApp]) -> Vec<Failure> {
    let failed = apps
        .iter()
        .filter_map(|a| reinstall_quiet(Some(a), a).err().map(|e| Failure { id: a.id.clone(), error: format!("{e:#}") }))
        .collect();
    integration::refresh_caches();
    failed
}

fn launcher_warning(r: Result<Option<PathBuf>>) -> Option<String> {
    r.err().map(|e| format!("launcher not updated: {e:#}"))
}

/// After Penates itself was updated or moved, rewrite every launcher so each points at (and, on
/// macOS, contains) the current program. Cheap no-op otherwise.
pub fn refresh_launchers_if_needed() {
    let Some(bin) = integration::find_desktop_bin().map(|b| b.display().to_string()) else { return };
    let s = settings::load();
    if s.launchers_version == updates::BUILD_VERSION && s.launchers_bin == bin {
        return;
    }
    for f in reinstall_all(&store::list()) {
        eprintln!("warning: launcher for '{}' not updated: {}", f.id, f.error);
    }
    // Re-read before saving, so a setting changed meanwhile is not reverted.
    let _ = settings::update(|s| {
        s.launchers_version = updates::BUILD_VERSION.into();
        s.launchers_bin = bin;
    });
}

// ---------------------------------------------------------------- validation

fn blank_to_none(s: &mut Option<String>) {
    if s.as_deref().map(str::trim).is_some_and(str::is_empty) {
        *s = None;
    }
}

/// Names end up in launcher files, so drop control characters (newlines could add lines to a
/// .desktop file), collapse whitespace and cap the length.
pub fn clean_name(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(100).collect()
}

/// The ways an app can start (see `WebApp::start_as`).
pub const START_AS: [&str; 3] = ["window", "minimized", "tray"];

/// Clean up user-edited values. Fails on an unusable URL, an empty name or an entry in
/// `internal_hosts` that is not a host name.
pub fn validate(app: &mut WebApp) -> Result<()> {
    app.url = normalize_url(&app.url)?;
    app.name = clean_name(&app.name);
    ensure!(!app.name.is_empty(), "name cannot be empty");
    app.width = app.width.clamp(200.0, 10000.0);
    app.height = app.height.clamp(200.0, 10000.0);
    app.zoom = app.zoom.clamp(0.25, 5.0);
    let mut hosts = vec![];
    for h in app.internal_hosts.iter().map(|h| h.trim()).filter(|h| !h.is_empty()) {
        let p = crate::normalize_host_pattern(h)
            .with_context(|| format!("'{h}' is not a host name (use example.com or *.example.com)"))?;
        if !hosts.contains(&p) {
            hosts.push(p);
        }
    }
    app.internal_hosts = hosts;
    if !crate::settings::THEMES.contains(&app.theme.as_str()) {
        app.theme = "system".into();
    }
    if !START_AS.contains(&app.start_as.as_str()) {
        app.start_as = "window".into();
    }
    blank_to_none(&mut app.user_agent);
    blank_to_none(&mut app.inject_css);
    blank_to_none(&mut app.inject_js);
    Ok(())
}

// ---------------------------------------------------------------- create

/// Where a new app's icon comes from.
#[derive(Debug, Clone, Default)]
pub enum IconChoice {
    /// The site's own (found automatically).
    #[default]
    Site,
    /// A local image file or an http(s) image URL.
    Source(String),
    /// Image bytes (an upload).
    Bytes(Vec<u8>),
}

/// Everything needed to create an app. Only `url` is required.
#[derive(Debug, Clone)]
pub struct NewApp {
    pub url: String,
    /// Display name; read from the site when None.
    pub name: Option<String>,
    /// Id; derived from the name (made unique) when None.
    pub id: Option<String>,
    pub icon: IconChoice,
    pub user_agent: Option<String>,
    pub width: f64,
    pub height: f64,
    pub zoom: f64,
    pub internal_hosts: Vec<String>,
    pub inject_css: Option<String>,
    pub inject_js: Option<String>,
    pub devtools: bool,
    /// Add a launcher to the application menu.
    pub launcher: bool,
    /// Replace an existing app with the same id.
    pub replace: bool,
}

impl Default for NewApp {
    fn default() -> Self {
        let d = WebApp::default();
        Self {
            url: String::new(),
            name: None,
            id: None,
            icon: IconChoice::Site,
            user_agent: None,
            width: d.width,
            height: d.height,
            zoom: d.zoom,
            internal_hosts: vec![],
            inject_css: None,
            inject_js: None,
            devtools: false,
            launcher: true,
            replace: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Created {
    pub app: WebApp,
    /// Catalog preset version linked, if any.
    pub preset: Option<String>,
    /// Launcher path, if one was written.
    pub launcher: Option<PathBuf>,
    /// Things that did not work but did not stop the app being created (no icon, no launcher).
    pub warnings: Vec<String>,
}

/// `base`, or `base-2`, `base-3`, ... whichever is free.
pub fn unique_id(base: &str) -> String {
    if !store::exists(base) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base}-{n}")).find(|c| !store::exists(c)).expect("unbounded range")
}

/// `name`, or `name 2`, `name 3`, ... whichever no existing app uses yet (ignoring case), so two apps for
/// the same site are told apart in menus and the taskbar.
pub fn unique_name(name: &str) -> String {
    let taken: Vec<String> = store::list().into_iter().map(|a| a.name.to_lowercase()).collect();
    if !taken.contains(&name.to_lowercase()) {
        return name.to_string();
    }
    (2..).map(|n| format!("{name} {n}")).find(|c| !taken.contains(&c.to_lowercase())).expect("unbounded range")
}

/// The host a site redirected to, when it is another site (e.g. a bare domain that sends you to
/// `mail.example.com`). Pages there should stay in the app.
fn redirect_host(app: &WebApp, page: &Page) -> Option<String> {
    let host = page.final_url.host_str()?;
    let target = url::Url::parse(&format!("https://{host}/")).ok()?;
    (!app.is_internal(&target)).then(|| crate::normalize_host_pattern(host)).flatten()
}

pub fn create(n: NewApp) -> Result<Created> {
    let url = normalize_url(&n.url)?;
    if let Some(id) = &n.id {
        store::check_id(id)?;
        ensure!(n.replace || !store::exists(id), "'{id}' already exists (use --force or another --id)");
    }
    let page = Page::fetch(&url).ok(); // None when offline: create the app anyway
    // A name you typed is kept as typed; one read from the site gets a number if another app has it.
    let name = match n.name.map(|s| clean_name(&s)).filter(|s| !s.is_empty()) {
        Some(typed) => typed,
        None => unique_name(&page.as_ref().and_then(Page::name).unwrap_or_else(|| crate::name_from_url(&url))),
    };
    let id = n.id.unwrap_or_else(|| unique_id(&crate::id_for(&name, &url)));
    let old = if n.replace { store::load(&id).ok() } else { None };
    let mut app = WebApp {
        id,
        name,
        url,
        user_agent: n.user_agent,
        width: n.width,
        height: n.height,
        zoom: n.zoom,
        internal_hosts: n.internal_hosts,
        inject_css: n.inject_css,
        inject_js: n.inject_js,
        devtools: n.devtools,
        launcher: n.launcher,
        ..Default::default()
    };
    if let Some(h) = page.as_ref().and_then(|p| redirect_host(&app, p)) {
        app.internal_hosts.push(h);
    }
    validate(&mut app)?;

    // The icon is written only after validation, so a rejected app leaves nothing behind.
    let mut warnings = vec![];
    let (icon, custom) = match n.icon {
        IconChoice::Bytes(b) => (Some(icon::set_from_bytes(&b, &app.id).context("icon")?), true),
        IconChoice::Source(s) => (Some(icon::import_source(&s, &app.id).context("icon")?), true),
        IconChoice::Site => match page.as_ref().map(icon::best_from) {
            Some(Ok(png)) => (Some(icon::save_png(&app.id, &png)?.0), false),
            Some(Err(e)) => {
                warnings.push(format!("{e:#}"));
                (None, false)
            }
            None => {
                warnings.push("could not reach the site, so there is no icon yet (Update fetches it later)".into());
                (None, false)
            }
        },
    };
    app.icon = icon.map(|p| store::icon_ref(&p));
    app.custom_icon = custom;
    let preset = link_preset(&mut app);
    store::save(&app)?;
    let launcher = match reinstall(old.as_ref(), &app) {
        Ok(p) => p,
        Err(e) => {
            warnings.push(format!("launcher not installed: {e:#}"));
            None
        }
    };
    Ok(Created { app, preset, launcher, warnings })
}

/// Link a new app to the catalog preset for its site, if there is one. A user agent or CSS the user
/// chose already wins over the preset's. Best effort: no catalog means no preset.
pub fn link_preset(app: &mut WebApp) -> Option<String> {
    let (catalog, _) = updates::load_catalog().ok()?;
    let (ua, css) = (app.user_agent.clone(), app.inject_css.clone());
    let (v, _) = updates::apply_preset(app, &catalog)?;
    if ua.is_some() {
        app.user_agent = ua;
    }
    if css.is_some() {
        app.inject_css = css;
    }
    Some(v)
}

// ---------------------------------------------------------------- edit

#[derive(Debug, Clone, Serialize)]
pub struct Saved {
    pub app: WebApp,
    pub warning: Option<String>,
}

/// Save an edited config. The id and icon are never changed here (icons have their own calls).
pub fn save_edit(mut incoming: WebApp) -> Result<Saved> {
    let old = store::load(&incoming.id)?;
    incoming.icon = old.icon.clone();
    incoming.custom_icon = old.custom_icon;
    validate(&mut incoming)?;
    store::save(&incoming)?;
    let warning = launcher_warning(reinstall(Some(&old), &incoming));
    Ok(Saved { app: incoming, warning })
}

pub fn set_icon_file(id: &str, path: PathBuf, custom: bool) -> Result<Saved> {
    let mut app = store::load(id)?;
    app.icon = Some(store::icon_ref(&path));
    app.custom_icon = custom;
    store::save(&app)?;
    let warning = launcher_warning(reinstall(None, &app));
    Ok(Saved { app, warning })
}

/// Go back to the site's default icon.
pub fn reset_icon(id: &str) -> Result<Saved> {
    let app = store::load(id)?;
    let p = icon::fetch_for(&app.url, id).context("no icon found on the site")?;
    set_icon_file(id, p, false)
}

// ---------------------------------------------------------------- update

#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    pub app: WebApp,
    /// The site's icon changed and was replaced.
    pub icon_refreshed: bool,
    /// Catalog preset version applied, if any.
    pub preset: Option<String>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Failure {
    pub id: String,
    pub error: String,
}

/// The site's icon for `app` (the preset's icon address first, if it has one), normalized.
fn fresh_icon(app: &WebApp, preset_icon: Option<&str>) -> Result<Vec<u8>> {
    if let Some(png) = preset_icon.and_then(|u| icon::fetch_png(u).ok()) {
        return Ok(png);
    }
    icon::best_from(&Page::fetch(&app.url)?)
}

/// Refresh one app: apply a newer catalog preset and re-download the site icon (unless the user
/// picked one). Files and the launcher are rewritten only when something changed; the desktop's
/// caches are not refreshed here (see `update`, `update_all`).
fn update_quiet(id: &str, catalog: Option<&updates::Catalog>) -> Result<Outcome> {
    let mut app = store::load(id)?;
    let before = app.clone();
    let (preset, preset_icon) = catalog.and_then(|c| updates::apply_preset(&mut app, c)).unzip();
    let mut icon_refreshed = false;
    if !app.custom_icon {
        if let Ok(png) = fresh_icon(&app, preset_icon.flatten().as_deref()) {
            let (p, changed) = icon::save_png(id, &png)?;
            app.icon = Some(store::icon_ref(&p));
            icon_refreshed = changed;
        }
    }
    let mut warning = None;
    if icon_refreshed || app != before {
        store::save(&app)?;
        warning = launcher_warning(reinstall_quiet(Some(&before), &app));
    }
    Ok(Outcome { app, icon_refreshed, preset, warning })
}

pub fn update(id: &str) -> Result<Outcome> {
    let catalog = updates::load_catalog().ok().map(|(c, _)| c);
    let o = update_quiet(id, catalog.as_ref());
    integration::refresh_caches();
    o
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateAll {
    pub updated: Vec<Outcome>,
    pub failed: Vec<Failure>,
}

/// Update every app, four at a time (most of the time is spent waiting on websites).
pub fn update_all() -> UpdateAll {
    let catalog = updates::load_catalog().ok().map(|(c, _)| c);
    let apps = store::list();
    let results: Vec<(String, Result<Outcome>)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..4)
            .map(|w| {
                let (apps, catalog) = (&apps, catalog.as_ref());
                s.spawn(move || {
                    apps.iter().skip(w).step_by(4).map(|a| (a.id.clone(), update_quiet(&a.id, catalog))).collect::<Vec<_>>()
                })
            })
            .collect();
        workers.into_iter().flat_map(|j| j.join().unwrap_or_default()).collect()
    });
    integration::refresh_caches();
    let mut out = UpdateAll { updated: vec![], failed: vec![] };
    for (id, r) in results {
        match r {
            Ok(o) => out.updated.push(o),
            Err(e) => out.failed.push(Failure { id, error: format!("{e:#}") }),
        }
    }
    out.updated.sort_by_key(|o| o.app.name.to_lowercase());
    out
}

// ---------------------------------------------------------------- data

/// Clearing an app's data needs it closed, and is not supported on macOS yet.
pub fn ensure_closed(id: &str) -> Result<WebApp> {
    ensure!(store::CAN_CLEAR_DATA, "Clearing data is not supported on macOS yet.");
    let app = store::load(id)?;
    ensure!(!single::is_running(id), "{} is running. Close it first.", app.name);
    Ok(app)
}

/// Remove an app; with `purge` also its cookies and data (refused while it is running).
pub fn remove(id: &str, purge: bool) -> Result<()> {
    store::check_id(id)?;
    ensure!(!(purge && single::is_running(id)), "'{id}' is running. Close it first.");
    store::remove(id, purge)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lose_control_characters() {
        assert_eq!(clean_name("  Git\r\nExec=evil  Hub "), "Git Exec=evil Hub");
        assert_eq!(clean_name(&"x".repeat(300)).len(), 100);
    }

    #[test]
    fn validate_clamps_and_normalizes() {
        let mut a = WebApp { id: "a".into(), name: " A ".into(), url: "example.com".into(), zoom: 0.0, width: 1.0, theme: "neon".into(), ..Default::default() };
        a.internal_hosts = vec!["Accounts.Google.com".into(), "https://accounts.google.com/x".into(), " ".into()];
        validate(&mut a).unwrap();
        assert_eq!((a.name.as_str(), a.url.as_str()), ("A", "https://example.com/"));
        assert_eq!((a.zoom, a.width, a.theme.as_str()), (0.25, 200.0, "system"));
        a.start_as = "sideways".into();
        validate(&mut a).unwrap();
        assert_eq!(a.start_as, "window");
        assert_eq!(a.internal_hosts, ["accounts.google.com"]);
        a.internal_hosts = vec!["*.com".into()];
        assert!(validate(&mut a).is_err());
        a.internal_hosts.clear();
        a.url = "javascript:alert(1)".into();
        assert!(validate(&mut a).is_err());
    }

    #[test]
    fn redirects_to_another_site_stay_in_the_app() {
        let app = WebApp { url: "https://example.com/".into(), ..Default::default() };
        let page = |u: &str| Page::from_html(url::Url::parse(u).unwrap(), "", false);
        assert_eq!(redirect_host(&app, &page("https://mail.example.net/inbox")).as_deref(), Some("mail.example.net"));
        assert_eq!(redirect_host(&app, &page("https://www.example.com/")), None);
    }
}
