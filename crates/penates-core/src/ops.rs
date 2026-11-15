//! High-level operations shared by the CLI and the desktop manager.

use crate::{icon, integration, normalize_url, store, updates, WebApp};
use anyhow::{ensure, Context, Result};

/// Remove any old launcher, then (re)write it for `app`. Best effort: a missing
/// desktop binary just means no launcher yet.
pub fn reinstall(old: Option<&WebApp>, app: &WebApp) {
    if let Some(o) = old {
        let _ = integration::uninstall(o);
    }
    if let Some(bin) = integration::find_desktop_bin() {
        let _ = integration::install(app, &bin);
    }
}

fn blank_to_none(s: &mut Option<String>) {
    if s.as_deref().map(str::trim).is_some_and(str::is_empty) {
        *s = None;
    }
}

/// Clean up user-edited values.
pub fn validate(app: &mut WebApp) -> Result<()> {
    app.url = normalize_url(&app.url)?;
    app.name = app.name.trim().to_string();
    ensure!(!app.name.is_empty(), "name cannot be empty");
    app.width = app.width.clamp(200.0, 10000.0);
    app.height = app.height.clamp(200.0, 10000.0);
    app.zoom = app.zoom.clamp(0.25, 5.0);
    app.internal_hosts = app.internal_hosts.iter().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect();
    if !matches!(app.theme.as_str(), "system" | "light" | "dark") {
        app.theme = "system".into();
    }
    blank_to_none(&mut app.user_agent);
    blank_to_none(&mut app.inject_css);
    blank_to_none(&mut app.inject_js);
    Ok(())
}

/// Save an edited config. The id and icon are never changed here (icons have their own calls).
pub fn save_edit(mut incoming: WebApp) -> Result<WebApp> {
    let old = store::load(&incoming.id)?;
    incoming.icon = old.icon.clone();
    incoming.custom_icon = old.custom_icon;
    validate(&mut incoming)?;
    store::save(&incoming)?;
    reinstall(Some(&old), &incoming);
    Ok(incoming)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Outcome {
    pub app: WebApp,
    pub icon_refreshed: bool,
    /// Catalog preset version applied, if any.
    pub preset: Option<String>,
}

/// Refresh one app: apply a newer catalog preset, re-download the site icon (unless the
/// user picked one), and rewrite the launcher. `manifest` is None when offline/unavailable.
pub fn update_with(id: &str, manifest: Option<&updates::Manifest>) -> Result<Outcome> {
    let mut app = store::load(id)?;
    let mut icon_refreshed = false;
    let mut preset = None;
    let mut preset_icon = None;
    if let Some(m) = manifest {
        if let Some((v, icon_url)) = updates::apply_preset(&mut app, m) {
            preset = Some(v);
            preset_icon = icon_url;
        }
    }
    if !app.custom_icon {
        let got = match preset_icon {
            Some(u) => icon::set_from_src(&u, id).or_else(|_| icon::fetch_for(&app.url, id)),
            None => icon::fetch_for(&app.url, id),
        };
        if let Ok(p) = got {
            app.icon = Some(store::icon_ref(&p));
            icon_refreshed = true;
        }
    }
    store::save(&app)?;
    reinstall(Some(&app.clone()), &app);
    Ok(Outcome { app, icon_refreshed, preset })
}

pub fn update(id: &str) -> Result<Outcome> {
    let m = updates::load_manifest().ok();
    update_with(id, m.as_ref())
}

/// Sync every app (fetches the manifest once).
pub fn update_all() -> Result<Vec<Outcome>> {
    let m = updates::load_manifest().ok();
    let mut out = vec![];
    for a in store::list()? {
        if let Ok(o) = update_with(&a.id, m.as_ref()) {
            out.push(o);
        }
    }
    Ok(out)
}

pub fn set_icon_file(id: &str, path: std::path::PathBuf, custom: bool) -> Result<WebApp> {
    let mut app = store::load(id)?;
    app.icon = Some(store::icon_ref(&path));
    app.custom_icon = custom;
    store::save(&app)?;
    reinstall(Some(&app.clone()), &app);
    Ok(app)
}

/// Go back to the site's default icon.
pub fn reset_icon(id: &str) -> Result<WebApp> {
    let app = store::load(id)?;
    let p = icon::fetch_for(&app.url, id).context("no icon found on the site")?;
    set_icon_file(id, p, false)
}
