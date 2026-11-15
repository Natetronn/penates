//! Update checks.
//!  - Penates itself: the public `latest.json` published with each release (the same file Tauri's
//!    updater reads; the desktop app installs from it, the CLI only reports).
//!  - App presets: a catalog in a JSON manifest (URL in settings.toml) synced into installed apps.
//!    With an empty URL the bundled sample manifest is used.

use crate::{store, WebApp};
use anyhow::{bail, Context, Result};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const SAMPLE: &str = include_str!("../data/sample-manifest.json");
/// Public releases (installers and `latest.json`). The source repo is private.
pub const RELEASES_PAGE: &str = "https://github.com/Natetronn/penates-releases/releases/latest";
pub const LATEST_JSON: &str = "https://github.com/Natetronn/penates-releases/releases/latest/download/latest.json";

// ---- settings (~/.config/penates/settings.toml) ----

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UpdateSettings {
    /// https:// or file:// URL (or a local path). Empty = bundled sample manifest.
    pub manifest_url: String,
    pub check_on_start: bool,
}
impl Default for UpdateSettings {
    fn default() -> Self {
        Self { manifest_url: String::new(), check_on_start: true }
    }
}
/// Look of the manager window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Appearance {
    /// "system" | "light" | "dark"
    pub theme: String,
    /// skin name from ui/skins.css: default, nord, solarized, mono
    pub skin: String,
    /// accent name from ui/tokens.css: blue, violet, green, orange, rose, slate
    pub accent: String,
}
impl Default for Appearance {
    fn default() -> Self {
        Self { theme: "system".into(), skin: "default".into(), accent: "blue".into() }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub updates: UpdateSettings,
    pub appearance: Appearance,
}

fn settings_path() -> std::path::PathBuf {
    store::root_config_dir().join("settings.toml")
}
pub fn load_settings() -> Settings {
    std::fs::read_to_string(settings_path()).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default()
}
pub fn save_settings(s: &Settings) -> Result<()> {
    std::fs::create_dir_all(store::root_config_dir())?;
    std::fs::write(settings_path(), toml::to_string_pretty(s)?)?;
    Ok(())
}

// ---- manifest ----

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Manifest {
    pub schema: u32,
    /// Ignored since 0.1.0 (Penates's own version comes from `LATEST_JSON`); kept so old files parse.
    pub penates: Option<PenatesRelease>,
    pub catalog: Vec<CatalogEntry>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PenatesRelease {
    pub version: String,
    pub notes: String,
    pub pub_date: String,
    /// keys like "linux-x86_64", "darwin-aarch64", "windows-x86_64"
    pub platforms: BTreeMap<String, Platform>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Platform {
    pub url: String,
    pub signature: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CatalogEntry {
    pub id: String,
    pub version: String,
    pub name: Option<String>,
    /// Used to link existing apps by host when they have no catalog id yet.
    pub url: String,
    pub notes: String,
    pub internal_hosts: Option<Vec<String>>,
    pub user_agent: Option<String>,
    pub inject_css: Option<String>,
    /// Ignored when applying presets (see `apply_preset`); kept so older manifests still parse.
    pub inject_js: Option<String>,
    pub icon_url: Option<String>,
}

pub fn platform_key() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        o => o,
    };
    format!("{os}-{}", std::env::consts::ARCH)
}

pub fn load_manifest() -> Result<Manifest> {
    let url = load_settings().updates.manifest_url;
    let url = url.trim();
    let text = if url.is_empty() {
        SAMPLE.to_string()
    } else if let Some(p) = url.strip_prefix("file://") {
        std::fs::read_to_string(p).with_context(|| format!("reading {p}"))?
    } else if url.starts_with("https://") {
        ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .get(url)
            .call()?
            .into_string()?
    } else if !url.contains("://") {
        std::fs::read_to_string(url).with_context(|| format!("reading {url}"))?
    } else {
        bail!("manifest_url must be https://, file:// or a path");
    };
    let m: Manifest = serde_json::from_str(&text).context("invalid update manifest")?;
    anyhow::ensure!(m.schema == 1, "unsupported manifest schema {}", m.schema);
    Ok(m)
}

fn ver(s: &str) -> Version {
    Version::parse(s.trim_start_matches('v')).unwrap_or_else(|_| Version::new(0, 0, 0))
}

fn host_of(u: &str) -> Option<String> {
    url::Url::parse(u).ok()?.host_str().map(|h| h.trim_start_matches("www.").to_string())
}

pub fn find_entry<'a>(m: &'a Manifest, app: &WebApp) -> Option<&'a CatalogEntry> {
    if let Some(id) = &app.catalog_id {
        return m.catalog.iter().find(|e| &e.id == id);
    }
    let h = host_of(&app.url)?;
    m.catalog.iter().find(|e| host_of(&e.url).as_deref() == Some(h.as_str()))
}

/// Apply a newer catalog preset to `app` in place. Returns (new version, icon_url) when applied.
pub fn apply_preset(app: &mut WebApp, m: &Manifest) -> Option<(String, Option<String>)> {
    let e = find_entry(m, app)?;
    let have = app.catalog_version.as_deref().map(ver).unwrap_or_else(|| Version::new(0, 0, 0));
    if ver(&e.version) <= have {
        return None;
    }
    if let Some(v) = &e.internal_hosts {
        for h in v {
            if !app.internal_hosts.contains(h) {
                app.internal_hosts.push(h.clone());
            }
        }
    }
    if let Some(v) = &e.user_agent { app.user_agent = Some(v.clone()); }
    if let Some(v) = &e.inject_css { app.inject_css = Some(v.clone()); }
    // `inject_js` from a preset is deliberately ignored: "Update all" would otherwise let whoever
    // controls the manifest URL run script inside every matching app.
    app.catalog_id = Some(e.id.clone());
    app.catalog_version = Some(e.version.clone());
    Some((e.version.clone(), e.icon_url.clone()))
}

// ---- report ----

#[derive(Debug, Clone, Serialize)]
pub struct PenatesUpdate {
    pub current: String,
    pub latest: String,
    pub notes: String,
    pub pub_date: String,
    /// Download for this OS/arch, if the manifest lists one.
    pub download_url: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct AppUpdate {
    pub id: String,
    pub name: String,
    pub from: Option<String>,
    pub to: String,
    pub notes: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct UpdateReport {
    pub current: String,
    /// "sample" or the manifest URL actually used.
    pub source: String,
    pub penates: Option<PenatesUpdate>,
    /// Why Penates's own version could not be checked (offline, no release yet).
    pub penates_error: Option<String>,
    pub apps: Vec<AppUpdate>,
}

/// The newest published Penates release (`latest.json`, Tauri updater format).
pub fn latest_release() -> Result<PenatesRelease> {
    let text = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .get(LATEST_JSON)
        .call()
        .context("could not reach the releases page")?
        .into_string()?;
    serde_json::from_str(&text).context("invalid latest.json")
}

pub fn check() -> Result<UpdateReport> {
    let m = load_manifest()?;
    let s = load_settings();
    let (penates, penates_error) = match latest_release() {
        Ok(r) => (
            (ver(&r.version) > ver(CURRENT_VERSION)).then(|| PenatesUpdate {
                current: CURRENT_VERSION.into(),
                latest: r.version.clone(),
                notes: r.notes.clone(),
                pub_date: r.pub_date.clone(),
                download_url: Some(RELEASES_PAGE.into()),
            }),
            None,
        ),
        Err(e) => (None, Some(format!("{e:#}"))),
    };
    let mut apps = vec![];
    for a in store::list()? {
        if let Some(e) = find_entry(&m, &a) {
            let have = a.catalog_version.as_deref().map(ver).unwrap_or_else(|| Version::new(0, 0, 0));
            if ver(&e.version) > have {
                apps.push(AppUpdate { id: a.id, name: a.name, from: a.catalog_version, to: e.version.clone(), notes: e.notes.clone() });
            }
        }
    }
    let source = if s.updates.manifest_url.trim().is_empty() { "bundled sample".into() } else { s.updates.manifest_url };
    Ok(UpdateReport { current: CURRENT_VERSION.into(), source, penates, penates_error, apps })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_parses_and_applies() {
        let m: Manifest = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(m.schema, 1);
        let mut app = WebApp { id: "gh".into(), url: "https://github.com/".into(), ..Default::default() };
        let (v, _) = apply_preset(&mut app, &m).unwrap();
        assert_eq!(v, "1.1.0");
        assert!(app.internal_hosts.contains(&"githubassets.com".to_string()));
        assert!(apply_preset(&mut app, &m).is_none()); // already current
    }

    #[test]
    fn presets_never_inject_javascript() {
        let m: Manifest = serde_json::from_str(r#"{"schema":1,"catalog":[{"id":"x","version":"9.0.0","url":"https://x.example/","inject_js":"steal()","inject_css":"a{}"}]}"#).unwrap();
        let mut app = WebApp { url: "https://x.example/".into(), ..Default::default() };
        assert!(apply_preset(&mut app, &m).is_some());
        assert!(app.inject_js.is_none());
        assert_eq!(app.inject_css.as_deref(), Some("a{}"));
    }

    #[test]
    fn semver_order() {
        assert!(ver("0.10.0") > ver("0.9.9"));
        assert!(ver("v1.0.0") > ver("1.0.0-rc.1"));
    }
}
