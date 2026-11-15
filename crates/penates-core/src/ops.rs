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

/// Names end up in launcher files, so drop control characters (newlines could add lines to a
/// .desktop file), collapse whitespace and cap the length.
pub fn clean_name(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(100).collect()
}

/// Clean up user-edited values.
pub fn validate(app: &mut WebApp) -> Result<()> {
    app.url = normalize_url(&app.url)?;
    app.name = clean_name(&app.name);
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

/// Link a new app to the catalog preset for its site, if there is one. A user agent or CSS the user
/// chose already wins over the preset's. Best effort: no catalog means no preset.
pub fn link_preset(app: &mut WebApp) -> Option<String> {
    let m = updates::load_manifest().ok()?;
    let (ua, css) = (app.user_agent.clone(), app.inject_css.clone());
    let (v, _) = updates::apply_preset(app, &m)?;
    if ua.is_some() { app.user_agent = ua; }
    if css.is_some() { app.inject_css = css; }
    Some(v)
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

/// After Penates itself was updated, rewrite every launcher so each points at (and, on macOS,
/// contains) the new binary. Cheap no-op otherwise.
pub fn refresh_launchers_if_upgraded() {
    let mut s = updates::load_settings();
    if s.launchers_version == updates::CURRENT_VERSION {
        return;
    }
    if let Ok(apps) = store::list() {
        for a in &apps {
            reinstall(Some(a), a);
        }
    }
    s.launchers_version = updates::CURRENT_VERSION.into();
    let _ = updates::save_settings(&s);
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
        validate(&mut a).unwrap();
        assert_eq!((a.name.as_str(), a.url.as_str()), ("A", "https://example.com/"));
        assert_eq!((a.zoom, a.width, a.theme.as_str()), (0.25, 200.0, "system"));
        a.url = "javascript:alert(1)".into();
        assert!(validate(&mut a).is_err());
    }
}
