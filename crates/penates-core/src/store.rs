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

/// Ids become file and folder names, so only `a-z 0-9 - _` (max 64) are accepted. This keeps a bad id
/// such as `..` from ever reaching a path (where `--purge` would delete the wrong folder).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_'))
}

fn check_id(id: &str) -> Result<()> {
    anyhow::ensure!(valid_id(id), "invalid app id '{id}' (use letters, digits, - and _)");
    Ok(())
}

pub fn path_for(id: &str) -> PathBuf {
    config_dir().join(format!("{id}.toml"))
}

pub fn exists(id: &str) -> bool {
    valid_id(id) && path_for(id).exists()
}

pub fn save(app: &WebApp) -> Result<()> {
    check_id(&app.id)?;
    std::fs::create_dir_all(config_dir())?;
    std::fs::write(path_for(&app.id), toml::to_string_pretty(app)?)?;
    Ok(())
}

pub fn load(id: &str) -> Result<WebApp> {
    check_id(id)?;
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
    out.sort_by_key(|a| a.name.to_lowercase());
    Ok(out)
}

/// Remove config, icon, launcher; optionally the browsing profile too.
pub fn remove(id: &str, purge: bool) -> Result<()> {
    check_id(id)?;
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
    if !valid_id(id) {
        return;
    }
    let _ = std::fs::remove_dir_all(profile_dir(id));
}

fn dir_size(p: &std::path::Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    rd.flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// Folders inside an app's profile that hold only re-downloadable cache (no logins).
pub fn cache_dirs(id: &str) -> Vec<PathBuf> {
    if !valid_id(id) {
        return vec![];
    }
    let p = profile_dir(id);
    vec![
        p.join("WebKitCache"),               // Linux (WebKitGTK)
        p.join("CacheStorage"),              // Linux: Cache API
        p.join("EBWebView/Default/Cache"),   // Windows (WebView2)
        p.join("EBWebView/Default/Code Cache"),
        p.join("EBWebView/Default/GPUCache"),
    ]
}

pub fn cache_size(id: &str) -> u64 {
    cache_dirs(id).iter().map(|d| dir_size(d)).sum()
}

/// Delete the cache and keep cookies/logins. Returns bytes freed.
pub fn clear_cache(id: &str) -> u64 {
    let before = cache_size(id);
    for d in cache_dirs(id) {
        let _ = std::fs::remove_dir_all(d);
    }
    before.saturating_sub(cache_size(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_that_could_escape_are_rejected() {
        for bad in ["", "..", "../x", "a/b", "a\\b", ".hidden", "x y", "é"] {
            assert!(!valid_id(bad), "{bad:?} should be invalid");
            assert!(load(bad).is_err());
        }
        assert!(cache_dirs("..").is_empty());
        assert!(valid_id("github") && valid_id("my-app_2"));
    }
}
