//! Shared model + storage + desktop integration for penates.
//! Both the CLI and the Tauri desktop app depend on this crate.

pub mod icon;
pub mod integration;
pub mod ops;
pub mod single;
pub mod site;
pub mod store;
pub mod updates;

use serde::{Deserialize, Serialize};

/// Configuration for one wrapped web app. Stored as `<config>/penates/apps/<id>.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WebApp {
    pub id: String,
    pub name: String,
    pub url: String,
    /// File name of the app's icon inside `~/.config/penates/icons` (our own normalized PNG copy).
    pub icon: Option<String>,
    /// True when the user chose the icon; `update` then never replaces it.
    pub custom_icon: bool,
    /// Catalog preset this app follows, and the preset version last applied.
    pub catalog_id: Option<String>,
    pub catalog_version: Option<String>,
    pub user_agent: Option<String>,
    pub width: f64,
    pub height: f64,
    pub zoom: f64,
    /// CSS injected into every page.
    pub inject_css: Option<String>,
    /// JS injected into every page (runs before page scripts).
    pub inject_js: Option<String>,
    /// Extra hosts (e.g. "accounts.google.com") that stay inside the app window.
    /// The host of `url` is always internal; `*.example.com` wildcards allowed.
    pub internal_hosts: Vec<String>,
    /// Keep a single window; relaunching focuses the existing one.
    pub single_instance: bool,
    pub hide_menu_bar: bool,
    pub start_maximized: bool,
    pub start_fullscreen: bool,
    pub resizable: bool,
    pub always_on_top: bool,
    /// Show the native title bar and borders.
    pub decorations: bool,
    /// Open links to other sites in the system browser (otherwise they load in-app).
    pub external_links_in_browser: bool,
    pub devtools: bool,
    /// Show a system tray icon.
    /// Window theme: "system", "light" or "dark" (also drives the site's prefers-color-scheme).
    pub theme: String,
    pub tray: bool,
    /// Draw the tray icon in grayscale.
    pub tray_grayscale: bool,
    /// Closing the window hides it to the tray instead of quitting.
    pub close_to_tray: bool,
}

impl Default for WebApp {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            url: String::new(),
            icon: None,
            custom_icon: false,
            catalog_id: None,
            catalog_version: None,
            user_agent: None,
            width: 1200.0,
            height: 800.0,
            zoom: 1.0,
            inject_css: None,
            inject_js: None,
            internal_hosts: vec![],
            single_instance: true,
            hide_menu_bar: true,
            start_maximized: true,
            start_fullscreen: false,
            resizable: true,
            always_on_top: false,
            decorations: true,
            external_links_in_browser: true,
            devtools: false,
            theme: "system".into(),
            tray: false,
            tray_grayscale: false,
            close_to_tray: false,
        }
    }
}

impl WebApp {
    /// Full path of the stored icon, resolved inside the icon folder. Older configs that hold an
    /// absolute path still work (only the file name is used). None if there is no icon file.
    pub fn icon_path(&self) -> Option<std::path::PathBuf> {
        let name = std::path::Path::new(self.icon.as_deref()?).file_name()?.to_owned();
        let p = store::icon_dir().join(name);
        p.exists().then_some(p)
    }

    /// Should navigation to `url` stay inside the app window?
    pub fn is_internal(&self, target: &url::Url) -> bool {
        if !matches!(target.scheme(), "http" | "https" | "about" | "blob" | "data") {
            return false;
        }
        if !self.external_links_in_browser && matches!(target.scheme(), "http" | "https") {
            return true;
        }
        let Some(host) = target.host_str() else {
            return true; // about:blank etc.
        };
        let own = url::Url::parse(&self.url).ok().and_then(|u| u.host_str().map(str::to_owned));
        if own.as_deref() == Some(host) {
            return true;
        }
        self.internal_hosts.iter().any(|pat| host_matches(pat, host))
    }
}

fn host_matches(pattern: &str, host: &str) -> bool {
    match pattern.strip_prefix("*.") {
        Some(suffix) => host == suffix || host.ends_with(&format!(".{suffix}")),
        None => pattern == host,
    }
}

/// Turn an arbitrary name into a filesystem-safe id.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// Normalise user input like `example.com` into a full URL.
pub fn normalize_url(input: &str) -> anyhow::Result<String> {
    let input = input.trim();
    let with_scheme = if input.contains("://") { input.to_string() } else { format!("https://{input}") };
    let u = url::Url::parse(&with_scheme)?;
    anyhow::ensure!(matches!(u.scheme(), "http" | "https"), "only http(s) URLs are supported");
    anyhow::ensure!(u.host_str().is_some(), "URL has no host");
    Ok(u.to_string())
}

/// Best name for a URL: the site's own meta tags, else a guess from the host.
pub fn resolve_name(url: &str) -> String {
    site::name(url).unwrap_or_else(|| name_from_url(url))
}

/// Guess a display name from a URL host ("mail.google.com" -> "Google").
pub fn name_from_url(url: &str) -> String {
    let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_owned)).unwrap_or_default();
    let parts: Vec<&str> = host.trim_start_matches("www.").split('.').collect();
    let base = if parts.len() >= 2 { parts[parts.len() - 2] } else { parts.first().copied().unwrap_or("app") };
    let mut c = base.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "App".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_ref_is_just_a_file_name() {
        let p = std::path::Path::new("/home/x/.config/penates/icons/gh.png");
        assert_eq!(store::icon_ref(p), "gh.png");
        // old absolute paths resolve by file name inside the icon folder
        let app = WebApp { icon: Some("/somewhere/else/missing-xyz.png".into()), ..Default::default() };
        assert!(app.icon_path().is_none()); // file doesn't exist; no panic
    }

    #[test]
    fn slug() {
        assert_eq!(slugify("My Cool App!"), "my-cool-app");
    }

    #[test]
    fn urls() {
        assert_eq!(normalize_url("example.com").unwrap(), "https://example.com/");
        assert!(normalize_url("ftp://x.com").is_err());
        assert_eq!(name_from_url("https://mail.google.com/"), "Google");
    }

    #[test]
    fn internal() {
        let app = WebApp {
            url: "https://app.example.com/".into(),
            internal_hosts: vec!["*.login.com".into()],
            ..Default::default()
        };
        let u = |s: &str| url::Url::parse(s).unwrap();
        assert!(app.is_internal(&u("https://app.example.com/x")));
        assert!(app.is_internal(&u("https://sso.login.com/")));
        assert!(!app.is_internal(&u("https://other.com/")));
        assert!(!app.is_internal(&u("mailto:a@b.c")));
    }
}
