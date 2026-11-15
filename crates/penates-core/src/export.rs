//! Export and import apps as a single `.penates` JSON file (sharing one app, or backing up all).
//!
//! The file holds app configs and their icons. It never holds cookies, logins or any browsing
//! data. Importing treats the file as untrusted: values are validated, and custom JavaScript is
//! dropped unless the caller explicitly allows it.

use crate::{icon, ops, slugify, store, updates, WebApp};
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use std::io::Read;

pub const FORMAT: &str = "penates-export";
pub const VERSION: u32 = 1;
const MAX_BYTES: u64 = 20 * 1024 * 1024;
const MAX_APPS: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFile {
    pub format: String,
    pub version: u32,
    pub penates_version: String,
    pub exported_unix: u64,
    /// Manager and update settings, only present when requested (backups).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<updates::Settings>,
    pub apps: Vec<ExportedApp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedApp {
    pub config: WebApp,
    /// Our normalized PNG, base64. Absent when the app had no icon.
    #[serde(default)]
    pub icon_png: Option<String>,
}

/// Build an export of `ids` (all apps when empty).
pub fn build(ids: &[String], with_settings: bool) -> Result<ExportFile> {
    let chosen: Vec<WebApp> = if ids.is_empty() {
        store::list()?
    } else {
        ids.iter().map(|id| store::load(id)).collect::<Result<_>>()?
    };
    ensure!(!chosen.is_empty(), "there are no apps to export");
    let apps = chosen
        .into_iter()
        .map(|mut c| {
            let icon_png = c.icon_path().and_then(|p| std::fs::read(p).ok()).map(|b| B64.encode(b));
            c.icon = None; // a file name on this machine means nothing elsewhere
            ExportedApp { config: c, icon_png }
        })
        .collect();
    let exported_unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    Ok(ExportFile {
        format: FORMAT.into(),
        version: VERSION,
        penates_version: updates::CURRENT_VERSION.into(),
        exported_unix,
        settings: with_settings.then(updates::load_settings),
        apps,
    })
}

/// Read export text from an http(s) URL or a local path.
pub fn read_source(src: &str) -> Result<String> {
    let mut buf = String::new();
    if src.starts_with("https://") || src.starts_with("http://") {
        let resp = icon::agent().get(src).call().with_context(|| format!("fetching {src}"))?;
        resp.into_reader().take(MAX_BYTES + 1).read_to_string(&mut buf)?;
    } else {
        let f = std::fs::File::open(src).with_context(|| format!("opening {src}"))?;
        f.take(MAX_BYTES + 1).read_to_string(&mut buf)?;
    }
    ensure!(buf.len() as u64 <= MAX_BYTES, "file is larger than 20 MB");
    Ok(buf)
}

/// Parse and sanity-check export text.
pub fn parse(text: &str) -> Result<ExportFile> {
    ensure!(text.len() as u64 <= MAX_BYTES, "file is larger than 20 MB");
    let f: ExportFile = serde_json::from_str(text).context("this is not an Penates export file")?;
    ensure!(f.format == FORMAT, "this is not an Penates export file");
    ensure!(f.version <= VERSION, "made by a newer Penates (file format v{}); update Penates first", f.version);
    ensure!(!f.apps.is_empty(), "the file contains no apps");
    ensure!(f.apps.len() <= MAX_APPS, "the file contains too many apps");
    Ok(f)
}

/// What the user should review before importing.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub id: String,
    pub name: String,
    pub url: String,
    pub custom_js: bool,
    pub custom_css: bool,
    pub user_agent: bool,
    pub has_icon: bool,
    /// Extra hosts kept inside the app window (no address bar there).
    pub extra_hosts: usize,
    /// All links stay in the app window, other sites included.
    pub all_links_in_app: bool,
    /// An app with this id already exists here.
    pub exists: bool,
}

pub fn inspect(f: &ExportFile) -> Vec<Summary> {
    f.apps
        .iter()
        .map(|a| {
            let c = &a.config;
            let id = clean_id(&c.id, &c.name);
            Summary {
                exists: store::exists(&id),
                id,
                name: c.name.clone(),
                url: c.url.clone(),
                custom_js: c.inject_js.is_some(),
                custom_css: c.inject_css.is_some(),
                user_agent: c.user_agent.is_some(),
                has_icon: a.icon_png.is_some(),
                extra_hosts: c.internal_hosts.len(),
                all_links_in_app: !c.external_links_in_browser,
            }
        })
        .collect()
}

/// Ids are used in file names and launcher names, so never trust the file's.
fn clean_id(id: &str, name: &str) -> String {
    let ok = !id.is_empty() && id.len() <= 64 && id.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-' | '_'));
    if ok {
        return id.to_string();
    }
    let s = slugify(if name.is_empty() { id } else { name });
    let s: String = s.chars().take(64).collect();
    if s.is_empty() { "app".into() } else { s }
}

fn unique_id(base: &str) -> String {
    if !store::exists(base) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base}-{n}")).find(|c| !store::exists(c)).expect("unbounded range")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    Skip,
    Rename,
    Overwrite,
}

impl std::str::FromStr for Conflict {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "skip" => Ok(Self::Skip),
            "rename" | "keep-both" => Ok(Self::Rename),
            "overwrite" => Ok(Self::Overwrite),
            other => anyhow::bail!("unknown conflict choice '{other}' (use skip, rename or overwrite)"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ImportOptions {
    pub on_conflict: Conflict,
    /// Keep custom JavaScript. Off by default: injected JS runs inside the site and can read what you type.
    pub allow_js: bool,
    /// Also apply the file's manager and update settings (including the update source).
    pub settings: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportReport {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
    /// (id in the file, id used here)
    pub renamed: Vec<(String, String)>,
    pub js_removed: Vec<String>,
    pub settings_applied: bool,
    pub errors: Vec<String>,
}

pub fn apply(f: &ExportFile, opts: &ImportOptions) -> ImportReport {
    let mut report = ImportReport::default();
    for a in &f.apps {
        let file_id = clean_id(&a.config.id, &a.config.name);
        let exists = store::exists(&file_id);
        let (id, old) = match (exists, opts.on_conflict) {
            (false, _) => (file_id.clone(), None),
            (true, Conflict::Skip) => {
                report.skipped.push(file_id);
                continue;
            }
            (true, Conflict::Rename) => (unique_id(&file_id), None),
            (true, Conflict::Overwrite) => (file_id.clone(), store::load(&file_id).ok()),
        };
        match import_one(a, &id, old.as_ref(), opts, &mut report) {
            Ok(()) => {
                if id != file_id {
                    report.renamed.push((file_id, id.clone()));
                }
                report.imported.push(id);
            }
            Err(e) => report.errors.push(format!("{}: {e:#}", a.config.name)),
        }
    }
    if opts.settings {
        if let Some(s) = &f.settings {
            match updates::save_settings(s) {
                Ok(()) => report.settings_applied = true,
                Err(e) => report.errors.push(format!("settings: {e:#}")),
            }
        }
    }
    report
}

fn import_one(a: &ExportedApp, id: &str, old: Option<&WebApp>, opts: &ImportOptions, report: &mut ImportReport) -> Result<()> {
    let mut cfg = a.config.clone();
    cfg.id = id.to_string();
    cfg.icon = None;
    if !opts.allow_js && cfg.inject_js.take().is_some() {
        report.js_removed.push(id.to_string());
    }
    ops::validate(&mut cfg)?; // url, sizes, theme, blanks

    // Icon: the file's own picture if it decodes, else the site's.
    let from_file = a
        .icon_png
        .as_deref()
        .and_then(|b| B64.decode(b).ok())
        .filter(|b| b.len() <= 8 * 1024 * 1024)
        .and_then(|b| icon::set_from_bytes(&b, id).ok());
    cfg.custom_icon = from_file.is_some() && a.config.custom_icon;
    let path = from_file.or_else(|| icon::fetch_for(&cfg.url, id).ok());
    cfg.icon = path.map(|p| store::icon_ref(&p));

    store::save(&cfg)?;
    ops::reinstall(old, &cfg);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> String {
        let mut c = WebApp { id: "gh".into(), name: "GitHub".into(), url: "https://github.com/".into(), ..Default::default() };
        c.inject_js = Some("alert(1)".into());
        serde_json::to_string(&ExportFile {
            format: FORMAT.into(),
            version: VERSION,
            penates_version: "0.1.0".into(),
            exported_unix: 0,
            settings: None,
            apps: vec![ExportedApp { config: c, icon_png: None }],
        })
        .unwrap()
    }

    #[test]
    fn parse_accepts_ours_and_rejects_others() {
        assert!(parse(&sample()).is_ok());
        assert!(parse("{}").is_err());
        assert!(parse("not json").is_err());
        let newer = sample().replace("\"version\":1", "\"version\":99");
        assert!(parse(&newer).unwrap_err().to_string().contains("newer Penates"));
    }

    #[test]
    fn inspect_flags_custom_code() {
        let f = parse(&sample()).unwrap();
        let s = inspect(&f);
        assert_eq!(s.len(), 1);
        assert!(s[0].custom_js && !s[0].custom_css);
    }

    #[test]
    fn ids_from_files_are_not_trusted() {
        assert_eq!(clean_id("good-id_1", "x"), "good-id_1");
        assert_eq!(clean_id("../../etc/passwd", "My App"), "my-app");
        assert_eq!(clean_id("", ""), "app");
        assert_eq!("rename".parse::<Conflict>().unwrap(), Conflict::Rename);
        assert!("nope".parse::<Conflict>().is_err());
    }
}
