use crate::WebApp;
use anyhow::{Context, Result};
use std::path::PathBuf;

pub fn root_config_dir() -> PathBuf {
    dirs::config_dir().expect("no config dir").join("penates")
}
pub fn config_dir() -> PathBuf {
    root_config_dir().join("apps")
}
pub fn data_dir() -> PathBuf {
    dirs::data_dir().expect("no data dir").join("penates")
}
/// Isolated webview profile (cookies, storage, cache) for one app.
pub fn profile_dir(id: &str) -> PathBuf {
    data_dir().join("profiles").join(id)
}
/// Icons are stored beside the config (`~/.config/penates/icons`) so the two travel together.
pub fn icon_dir() -> PathBuf {
    root_config_dir().join("icons")
}

/// What goes in `WebApp.icon`: just the file name, so the config never holds a machine-specific path.
pub fn icon_ref(path: &std::path::Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn path_for(id: &str) -> PathBuf {
    config_dir().join(format!("{id}.toml"))
}

pub fn exists(id: &str) -> bool {
    path_for(id).exists()
}

pub fn save(app: &WebApp) -> Result<()> {
    std::fs::create_dir_all(config_dir())?;
    std::fs::write(path_for(&app.id), toml::to_string_pretty(app)?)?;
    Ok(())
}

pub fn load(id: &str) -> Result<WebApp> {
    let p = path_for(id);
    let s = std::fs::read_to_string(&p).with_context(|| format!("no app with id '{id}'"))?;
    let mut app: WebApp = toml::from_str(&s).with_context(|| format!("parsing {}", p.display()))?;
    app.id = id.to_string();
    Ok(app)
}

pub fn list() -> Result<Vec<WebApp>> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(config_dir()) else { return Ok(out) };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        if let Some(id) = p.file_stem().and_then(|s| s.to_str()) {
            if let Ok(a) = load(id) {
                out.push(a);
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Remove config, icon, launcher; optionally the browsing profile too.
pub fn remove(id: &str, purge: bool) -> Result<()> {
    if let Ok(app) = load(id) {
        let _ = crate::integration::uninstall(&app);
    }
    let _ = std::fs::remove_file(path_for(id));
    let _ = std::fs::remove_file(icon_dir().join(format!("{id}.png")));
    let _ = std::fs::remove_file(icon_dir().join(format!("{id}.ico")));
    if purge {
        clear_profile(id);
    }
    Ok(())
}

/// Delete cookies/storage/cache for an app.
pub fn clear_profile(id: &str) {
    let _ = std::fs::remove_dir_all(profile_dir(id));
}
