//! Shared model + storage + desktop integration for penates.
//! Both the CLI and the Tauri desktop app depend on this crate.

pub mod export;
pub mod http;
pub mod icon;
pub mod integration;
pub mod ops;
pub mod settings;
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
    pub start_maximized: bool,
    pub start_fullscreen: bool,
    pub resizable: bool,
    pub always_on_top: bool,
    /// Show the native title bar and borders.
    pub decorations: bool,
    /// Open links to other sites in the system browser (otherwise they load in-app).
    pub external_links_in_browser: bool,
    pub devtools: bool,
    /// Window theme: "system", "light" or "dark" (honored on Windows and macOS; Linux sites follow the desktop).
    pub theme: String,
    /// Let the site show desktop notifications (enforced on Linux; see docs for other platforms).
    pub allow_notifications: bool,
    /// Let the site use the camera and microphone (enforced on Linux; see docs for other platforms).
    pub allow_camera_microphone: bool,
    /// Show a system tray icon.
    pub tray: bool,
    /// Draw the tray icon in grayscale.
    pub tray_grayscale: bool,
    /// Closing the window hides it to the tray instead of quitting.
    pub close_to_tray: bool,
    /// Keep a launcher in the application menu (off for apps created with `--no-install`).
    pub launcher: bool,
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
            start_maximized: true,
            start_fullscreen: false,
            resizable: true,
            always_on_top: false,
            decorations: true,
            external_links_in_browser: true,
            devtools: false,
            theme: "system".into(),
            allow_notifications: true,
            allow_camera_microphone: false,
            tray: false,
            tray_grayscale: false,
            close_to_tray: false,
            launcher: true,
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

    /// The app's own host, without a leading `www.`.
    pub fn own_host(&self) -> Option<String> {
        url::Url::parse(&self.url).ok()?.host_str().map(|h| bare_host(h).to_string())
    }

    /// Should navigation to `url` stay inside the app window? Mirrored in JavaScript by the runner's
    /// link guard; keep the two in step (see `host_matches`).
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
        self.own_host().is_some_and(|own| bare_host(host) == own) || self.internal_hosts.iter().any(|pat| host_matches(pat, host))
    }
}

/// `www.example.com` and `example.com` are the same site.
pub fn bare_host(host: &str) -> &str {
    host.strip_prefix("www.").unwrap_or(host)
}

/// Does `host` match an internal-host pattern (`example.com`, or `*.example.com` for it and every
/// subdomain)? Patterns are stored lowercase (see `normalize_host_pattern`).
pub fn host_matches(pattern: &str, host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    match pattern.strip_prefix("*.") {
        Some(suffix) => host == suffix || host.ends_with(&format!(".{suffix}")),
        None => pattern == host,
    }
}

/// Clean one internal-host entry as typed by a person: lowercase, no scheme, path, port or trailing
/// dot. Returns None for entries that are not a host, and for wildcards over a whole top-level
/// domain such as `*.com`.
pub fn normalize_host_pattern(input: &str) -> Option<String> {
    let s = input.trim().to_ascii_lowercase();
    let s = s.split_once("://").map_or(s.as_str(), |(_, rest)| rest);
    let s = s.split(['/', '?', '#']).next().unwrap_or("");
    let s = s.rsplit_once('@').map_or(s, |(_, h)| h);
    let s = s.split(':').next().unwrap_or("").trim_end_matches('.');
    let (wild, host) = match s.strip_prefix("*.") {
        Some(h) => (true, h),
        None => (false, s),
    };
    let labels: Vec<&str> = host.split('.').collect();
    let ok = !host.is_empty()
        && labels.iter().all(|l| !l.is_empty() && l.len() <= 63 && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && (!wild || labels.len() >= 2);
    ok.then(|| if wild { format!("*.{host}") } else { host.to_string() })
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
    // Ids are capped at 64 characters (see store::valid_id).
    let out: String = out.chars().take(64).collect();
    out.trim_end_matches('-').to_string()
}

/// Normalise user input like `example.com` into a full URL.
pub fn normalize_url(input: &str) -> anyhow::Result<String> {
    let input = input.trim();
    // Only a scheme at the very start counts ("example.com/?next=http://x" has none).
    let has_scheme = input.split_once("://").is_some_and(|(s, _)| {
        !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    });
    let with_scheme = if has_scheme { input.to_string() } else { format!("https://{input}") };
    let u = url::Url::parse(&with_scheme)?;
    anyhow::ensure!(matches!(u.scheme(), "http" | "https"), "only http(s) URLs are supported");
    anyhow::ensure!(u.host_str().is_some(), "URL has no host");
    Ok(u.to_string())
}

/// The label that names a site in its host: "mail.google.com" -> "google", "example.co.uk" ->
/// "example". IP addresses and single-label hosts come back whole.
pub fn site_label(host: &str) -> String {
    let host = bare_host(host.trim_end_matches('.'));
    if host.parse::<std::net::IpAddr>().is_ok() || host.starts_with('[') {
        return host.to_string();
    }
    let parts: Vec<&str> = host.split('.').collect();
    // Second-level labels used under country domains (co.uk, com.au, ...).
    const SLD: [&str; 8] = ["co", "com", "org", "net", "gov", "ac", "edu", "ne"];
    let n = parts.len();
    let i = match n {
        0 | 1 => 0,
        _ if n >= 3 && parts[n - 1].len() == 2 && SLD.contains(&parts[n - 2]) => n - 3,
        _ => n - 2,
    };
    parts.get(i).copied().unwrap_or("app").to_ascii_lowercase()
}

/// Guess a display name from a URL host ("mail.google.com" -> "Google").
pub fn name_from_url(url: &str) -> String {
    let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_owned)).unwrap_or_default();
    let label = site_label(&host);
    let mut c = label.chars();
    match c.next() {
        Some(f) if host.parse::<std::net::IpAddr>().is_err() => f.to_uppercase().collect::<String>() + c.as_str(),
        Some(_) => label,
        None => "App".into(),
    }
}

/// An id for a new app: from its name, else (names in other alphabets have no ASCII letters)
/// from its address, else "app".
pub fn id_for(name: &str, url: &str) -> String {
    [slugify(name), slugify(&name_from_url(url))].into_iter().find(|s| !s.is_empty()).unwrap_or_else(|| "app".into())
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
        let long = slugify(&"Very Long Name ".repeat(10));
        assert!(long.len() <= 64 && store::valid_id(&long), "{long}");
    }

    #[test]
    fn urls() {
        assert_eq!(normalize_url("example.com").unwrap(), "https://example.com/");
        assert!(normalize_url("ftp://x.com").is_err());
        assert_eq!(normalize_url("example.com/?next=http://x.y").unwrap(), "https://example.com/?next=http://x.y");
        assert_eq!(normalize_url("HTTP://Example.com").unwrap(), "http://example.com/");
    }

    #[test]
    fn names_and_ids_from_addresses() {
        assert_eq!(name_from_url("https://mail.google.com/"), "Google");
        assert_eq!(name_from_url("https://www.example.co.uk/"), "Example");
        assert_eq!(name_from_url("http://192.168.1.20:8123/"), "192.168.1.20");
        assert_eq!(id_for("日本語", "https://example.jp/"), "example");
        assert_eq!(id_for("Яндекс", "http://10.0.0.1/"), "10-0-0-1");
        assert_eq!(id_for("GitHub", "https://github.com/"), "github");
    }

    #[test]
    fn host_patterns_are_cleaned() {
        assert_eq!(normalize_host_pattern(" Accounts.Google.COM. ").as_deref(), Some("accounts.google.com"));
        assert_eq!(normalize_host_pattern("https://accounts.google.com/signin?x=1").as_deref(), Some("accounts.google.com"));
        assert_eq!(normalize_host_pattern("*.Example.com").as_deref(), Some("*.example.com"));
        assert_eq!(normalize_host_pattern("*.com"), None);
        assert_eq!(normalize_host_pattern("not a host"), None);
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
        assert!(app.is_internal(&u("https://www.app.example.com/x"))); // www is the same site
        assert!(app.is_internal(&u("https://SSO.Login.com/")));
        assert!(app.is_internal(&u("https://sso.login.com/")));
        assert!(!app.is_internal(&u("https://other.com/")));
        assert!(!app.is_internal(&u("mailto:a@b.c")));
    }
}
