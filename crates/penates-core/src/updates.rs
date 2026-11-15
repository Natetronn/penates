//! Update checks.
//!  - Penates itself: the public `latest.json` published with each release (the same file Tauri's
//!    updater reads; the desktop app installs from it, the CLI only reports).
//!  - App presets: a catalog of per-site fixes synced into installed apps. By default the Penates
//!    app catalog (github.com/Natetronn/penates-apps); the built-in copy (`data/presets.json`) when it
//!    cannot be reached; or the address in settings (`[updates] catalog_url`).

use crate::{http, settings, store, WebApp};
use anyhow::{bail, Context, Result};
use semver::Version;
use serde::{Deserialize, Serialize};

/// This build's version, for comparing with releases.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// The version to show people: `0.1.0` for a release build, `0.1.0+8b4141a` for any other commit
/// (set by build.rs).
pub const BUILD_VERSION: &str = env!("PENATES_BUILD_VERSION");
const BUILTIN: &str = include_str!("../data/presets.json");
/// The shared preset catalog, built from github.com/Natetronn/penates-apps.
pub const DEFAULT_CATALOG: &str = "https://raw.githubusercontent.com/Natetronn/penates-apps/main/catalog.json";
/// GitHub releases (installers and `latest.json`).
pub const RELEASES_PAGE: &str = "https://github.com/Natetronn/penates/releases/latest";
pub const LATEST_JSON: &str = "https://github.com/Natetronn/penates/releases/latest/download/latest.json";

// ---- the preset catalog ----

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Catalog {
    pub schema: u32,
    pub catalog: Vec<CatalogEntry>,
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
    pub icon_url: Option<String>,
    // `inject_js` is deliberately not read: "Update all" would otherwise let whoever controls the
    // catalog address run script inside every matching app.
}

/// The catalog and a label for where it came from.
pub fn load_catalog() -> Result<(Catalog, String)> {
    let url = settings::load().updates.catalog_url;
    let url = url.trim();
    if url.is_empty() {
        return Ok(match http::get_text(DEFAULT_CATALOG).and_then(|t| parse_catalog(&t)) {
            Ok(c) => (c, "Penates app catalog".into()),
            Err(_) => (parse_catalog(BUILTIN)?, "built-in catalog (app catalog unreachable)".into()),
        });
    }
    let text = if let Some(p) = url.strip_prefix("file://") {
        std::fs::read_to_string(p).with_context(|| format!("reading {p}"))?
    } else if url.starts_with("https://") {
        http::get_text(url)?
    } else if !url.contains("://") {
        std::fs::read_to_string(url).with_context(|| format!("reading {url}"))?
    } else {
        bail!("the catalog address must be https://, file:// or a path");
    };
    Ok((parse_catalog(&text)?, url.to_string()))
}

fn parse_catalog(text: &str) -> Result<Catalog> {
    let c: Catalog = serde_json::from_str(text).context("invalid preset catalog")?;
    anyhow::ensure!(c.schema == 1, "unsupported catalog schema {}", c.schema);
    Ok(c)
}

fn ver(s: &str) -> Version {
    Version::parse(s.trim_start_matches('v')).unwrap_or_else(|_| Version::new(0, 0, 0))
}

pub fn find_entry<'a>(c: &'a Catalog, app: &WebApp) -> Option<&'a CatalogEntry> {
    if let Some(id) = &app.catalog_id {
        return c.catalog.iter().find(|e| &e.id == id);
    }
    let h = app.own_host()?;
    c.catalog.iter().find(|e| url::Url::parse(&e.url).ok().and_then(|u| u.host_str().map(|x| crate::bare_host(x) == h)) == Some(true))
}

/// The catalog entry for `app` when it is newer than the preset the app already has.
pub fn newer_entry<'a>(c: &'a Catalog, app: &WebApp) -> Option<&'a CatalogEntry> {
    let e = find_entry(c, app)?;
    let have = app.catalog_version.as_deref().map(ver).unwrap_or_else(|| Version::new(0, 0, 0));
    (ver(&e.version) > have).then_some(e)
}

/// Apply a newer catalog preset to `app` in place. Returns (new version, icon_url) when applied.
pub fn apply_preset(app: &mut WebApp, c: &Catalog) -> Option<(String, Option<String>)> {
    let e = newer_entry(c, app)?;
    for h in e.internal_hosts.iter().flatten().filter_map(|h| crate::normalize_host_pattern(h)) {
        if !app.internal_hosts.contains(&h) {
            app.internal_hosts.push(h);
        }
    }
    if let Some(v) = &e.user_agent {
        app.user_agent = Some(v.clone());
    }
    if let Some(v) = &e.inject_css {
        app.inject_css = Some(v.clone());
    }
    app.catalog_id = Some(e.id.clone());
    app.catalog_version = Some(e.version.clone());
    Some((e.version.clone(), e.icon_url.clone()))
}

// ---- reports ----

#[derive(Debug, Clone, Serialize)]
pub struct PenatesUpdate {
    pub current: String,
    pub latest: String,
    pub notes: String,
    pub pub_date: String,
}

/// The parts of the published `latest.json` Penates reads.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LatestRelease {
    pub version: String,
    pub notes: String,
    pub pub_date: String,
}

/// A newer published Penates, if there is one. Before the first release there is no `latest.json`
/// (404), which simply means nothing newer.
pub fn check_penates() -> Result<Option<PenatesUpdate>> {
    let Some(text) = http::get_text_if_exists(LATEST_JSON).context("could not reach the releases page")? else {
        return Ok(None);
    };
    let r: LatestRelease = serde_json::from_str(&text).context("invalid latest.json")?;
    Ok((ver(&r.version) > ver(CURRENT_VERSION)).then(|| PenatesUpdate {
        current: CURRENT_VERSION.into(),
        latest: r.version,
        notes: r.notes,
        pub_date: r.pub_date,
    }))
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
pub struct AppsReport {
    pub current: String,
    /// "Penates app catalog", "built-in catalog (…)" or the catalog address used.
    pub source: String,
    /// Apps in the catalog, and apps installed here.
    pub catalog_size: usize,
    pub app_count: usize,
    pub apps: Vec<AppUpdate>,
}

/// Apps whose catalog preset has a newer version.
pub fn check_apps() -> Result<AppsReport> {
    let (c, source) = load_catalog()?;
    let mine = store::list();
    let app_count = mine.len();
    let apps = mine
        .into_iter()
        .filter_map(|a| {
            let e = newer_entry(&c, &a)?;
            Some(AppUpdate { to: e.version.clone(), notes: e.notes.clone(), from: a.catalog_version, id: a.id, name: a.name })
        })
        .collect();
    Ok(AppsReport { current: CURRENT_VERSION.into(), source, catalog_size: c.catalog.len(), app_count, apps })
}

/// One app in the catalog, for browsing ("Popular apps").
#[derive(Debug, Clone, Serialize)]
pub struct CatalogApp {
    pub id: String,
    pub name: String,
    pub url: String,
    pub notes: String,
    /// An icon address the catalog gives for this site, if any.
    pub icon_url: Option<String>,
    /// The id of an app already made for this site, if any.
    pub added: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogListing {
    pub source: String,
    pub apps: Vec<CatalogApp>,
}

/// Every app in the catalog, sorted by name, marking the ones already made here.
pub fn catalog_listing() -> Result<CatalogListing> {
    let (c, source) = load_catalog()?;
    Ok(listing(&c, source, &store::list()))
}

fn listing(c: &Catalog, source: String, mine: &[WebApp]) -> CatalogListing {
    let mut apps: Vec<CatalogApp> = c
        .catalog
        .iter()
        .map(|e| CatalogApp {
            id: e.id.clone(),
            name: e.name.clone().unwrap_or_else(|| e.id.clone()),
            url: e.url.clone(),
            notes: e.notes.clone(),
            icon_url: e.icon_url.clone(),
            added: mine.iter().find(|a| find_entry(c, a).is_some_and(|x| x.id == e.id)).map(|a| a.id.clone()),
        })
        .collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    CatalogListing { source, apps }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_parses_and_applies() {
        let c = parse_catalog(BUILTIN).unwrap();
        assert!(c.catalog.len() >= 8);
        let mut app = WebApp { id: "gh".into(), url: "https://www.github.com/".into(), ..Default::default() };
        let (v, _) = apply_preset(&mut app, &c).unwrap();
        assert_eq!(app.catalog_id.as_deref(), Some("github"));
        assert_eq!(app.catalog_version.as_deref(), Some(v.as_str()));
        assert!(!app.internal_hosts.is_empty());
        assert!(apply_preset(&mut app, &c).is_none()); // already current
    }

    #[test]
    fn presets_never_inject_javascript() {
        let c = parse_catalog(r#"{"schema":1,"catalog":[{"id":"x","version":"9.0.0","url":"https://x.example/","inject_js":"steal()","inject_css":"a{}","internal_hosts":["*.COM","Login.X.example"]}]}"#).unwrap();
        let mut app = WebApp { url: "https://x.example/".into(), ..Default::default() };
        assert!(apply_preset(&mut app, &c).is_some());
        assert!(app.inject_js.is_none());
        assert_eq!(app.inject_css.as_deref(), Some("a{}"));
        assert_eq!(app.internal_hosts, ["login.x.example"]); // cleaned; the *.com wildcard dropped
    }

    #[test]
    fn listing_marks_apps_already_made() {
        let c = parse_catalog(BUILTIN).unwrap();
        let mine = [WebApp { id: "my-gh".into(), url: "https://www.github.com/".into(), ..Default::default() }];
        let l = listing(&c, "test".into(), &mine);
        assert_eq!(l.apps.len(), c.catalog.len());
        assert!(l.apps.windows(2).all(|w| w[0].name.to_lowercase() <= w[1].name.to_lowercase()));
        let gh = l.apps.iter().find(|a| a.id == "github").unwrap();
        assert_eq!(gh.added.as_deref(), Some("my-gh"));
        assert!(l.apps.iter().filter(|a| a.id != "github").all(|a| a.added.is_none()));
    }

    #[test]
    fn semver_order() {
        assert!(ver("0.10.0") > ver("0.9.9"));
        assert!(ver("v1.0.0") > ver("1.0.0-rc.1"));
    }
}
