//! Where apps live on disk: configs, icons and the per-app browsing profiles.

use crate::WebApp;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn root_config_dir() -> PathBuf {
    dirs::config_dir().expect("no config dir").join("penates")
}
pub fn config_dir() -> PathBuf {
    root_config_dir().join("apps")
}
pub fn data_dir() -> PathBuf {
    // Local, not roaming, on Windows: profiles hold caches that must not sync across machines.
    dirs::data_local_dir().expect("no data dir").join("penates")
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
pub fn icon_ref(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Ids become file and folder names, so only `a-z 0-9 - _` (max 64) are accepted. Lowercase only:
/// `Demo` and `demo` would be the same file on Windows and macOS. This also keeps an id such as `..`
/// from ever reaching a path (where `--purge` would delete the wrong folder).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-' | '_'))
}

pub fn check_id(id: &str) -> Result<()> {
    anyhow::ensure!(valid_id(id), "invalid app id '{id}' (use lowercase letters, digits, - and _; at most 64)");
    Ok(())
}

pub fn path_for(id: &str) -> PathBuf {
    config_dir().join(format!("{id}.toml"))
}

/// A config file exists for this id (it may or may not parse).
pub fn exists(id: &str) -> bool {
    valid_id(id) && path_for(id).exists()
}

/// Write via a temporary file and a rename, so a crash never leaves a half-written file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

pub fn save(app: &WebApp) -> Result<()> {
    check_id(&app.id)?;
    write_atomic(&path_for(&app.id), toml::to_string_pretty(app)?.as_bytes())
}

pub fn load(id: &str) -> Result<WebApp> {
    check_id(id)?;
    let p = path_for(id);
    let s = std::fs::read_to_string(&p).with_context(|| format!("no app with id '{id}'"))?;
    let mut app: WebApp = toml::from_str(&s).with_context(|| format!("parsing {}", p.display()))?;
    app.id = id.to_string();
    Ok(app)
}

/// A config that exists but does not load, with the reason.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Broken {
    pub id: String,
    pub error: String,
}

/// Every config file: the ones that load (sorted by name) and the ones that don't.
pub fn scan() -> (Vec<WebApp>, Vec<Broken>) {
    let (mut ok, mut broken) = (vec![], vec![]);
    let Ok(rd) = std::fs::read_dir(config_dir()) else { return (ok, broken) };
    for p in rd.flatten().map(|e| e.path()) {
        if p.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        let Some(id) = p.file_stem().and_then(|s| s.to_str()) else { continue };
        match load(id) {
            Ok(a) => ok.push(a),
            Err(e) => broken.push(Broken { id: id.to_string(), error: format!("{e:#}") }),
        }
    }
    ok.sort_by_key(|a| a.name.to_lowercase());
    broken.sort_by(|a, b| a.id.cmp(&b.id));
    (ok, broken)
}

pub fn list() -> Vec<WebApp> {
    scan().0
}

/// Remove config, icon and launcher; with `purge` the browsing profile too. Works for configs that
/// no longer parse (the launcher is then found by id).
pub fn remove(id: &str, purge: bool) -> Result<()> {
    check_id(id)?;
    anyhow::ensure!(path_for(id).exists() || profile_dir(id).exists(), "no app with id '{id}'");
    match load(id) {
        Ok(app) => crate::integration::uninstall(&app),
        Err(_) => crate::integration::uninstall_id(id),
    }
    for p in [path_for(id), icon_dir().join(format!("{id}.png")), icon_dir().join(format!("{id}.ico"))] {
        if let Err(e) = std::fs::remove_file(&p) {
            anyhow::ensure!(e.kind() == std::io::ErrorKind::NotFound, "removing {}: {e}", p.display());
        }
    }
    if purge {
        clear_profile(id)?;
    }
    Ok(())
}

/// Delete cookies/storage/cache for an app.
/// macOS keeps each app's data in a WKWebView data store, not a folder, so clearing is not
/// supported there yet.
pub const CAN_CLEAR_DATA: bool = !cfg!(target_os = "macos");

pub fn clear_profile(id: &str) -> Result<()> {
    check_id(id)?;
    match std::fs::remove_dir_all(profile_dir(id)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e).context("clearing the app's data"),
        _ => Ok(()),
    }
}

fn dir_size(p: &Path) -> u64 {
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
        p.join("WebKitCache"),             // Linux (WebKitGTK)
        p.join("CacheStorage"),            // Linux: Cache API
        p.join("EBWebView/Default/Cache"), // Windows (WebView2)
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
        for bad in ["", "..", "../x", "a/b", "a\\b", ".hidden", "x y", "é", "Demo", &"a".repeat(65)] {
            assert!(!valid_id(bad), "{bad:?} should be invalid");
            assert!(load(bad).is_err());
        }
        assert!(cache_dirs("..").is_empty());
        assert!(valid_id("github") && valid_id("my-app_2"));
    }

    #[test]
    fn atomic_write_replaces_whole_file() {
        let p = std::env::temp_dir().join(format!("penates-atomic-{}.txt", std::process::id()));
        write_atomic(&p, b"first version, long").unwrap();
        write_atomic(&p, b"second").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"second");
        assert!(!p.with_extension("tmp").exists());
        let _ = std::fs::remove_file(p);
    }
}
