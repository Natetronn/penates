//! Report a problem: the Penates version, how it was installed, the system and the recent log lines, with
//! personal details taken out, for the user to read and then send themselves as a GitHub issue. Nothing
//! here sends anything.

use crate::{log, store, updates::BUILD_VERSION};
use serde::Serialize;

/// Where reports go: the bug form of Penates's issue tracker, whose fields the link fills in.
pub const ISSUES: &str = "https://github.com/Natetronn/penates/issues/new";
/// How many log lines a report shows.
pub const LOG_LINES: usize = 40;
/// Longest link the report makes; browsers and GitHub refuse much longer ones. Older log lines are left
/// out of the link to fit (the copied report keeps them).
const MAX_LINK: usize = 7000;

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// The whole report, as shown and copied.
    pub text: String,
    /// A new GitHub issue with the bug form filled in from the report.
    pub link: String,
    /// Log lines left out of the link to keep it short enough (the text has all of them).
    pub left_out: usize,
    /// The log file, for "Open log folder".
    pub log: String,
}

/// Build a report. `what` is the user's own description (may be empty); `webview` the web view's version
/// when the caller knows it (the manager does, the command line does not).
pub fn build(what: &str, webview: Option<&str>) -> Report {
    let install = install_kind();
    let system = system();
    let apps = store::list().len();
    let mut facts = vec![format!("Penates {BUILD_VERSION} ({install})"), format!("System: {system}")];
    if let Some(w) = webview {
        facts.push(format!("Web view: {w}"));
    }
    facts.push(format!("Apps: {apps}"));
    let lines: Vec<String> = log::recent(LOG_LINES).iter().map(|l| redact(l)).collect();
    let compose = |lines: &[String], left_out: usize| {
        let mut t = facts.join("\n") + "\n\n";
        if lines.is_empty() {
            t += "Recent log: empty (no warnings or errors recorded)\n";
        } else {
            t += &format!("Recent log (oldest first, {} lines):\n", lines.len());
            if left_out > 0 {
                t += &format!("({left_out} older lines left out to fit the link)\n");
            }
            t += &lines.join("\n");
            t += "\n";
        }
        t
    };
    let text = compose(&lines, 0);
    let what = redact(what.trim());
    let link_for = |body: &str| {
        url::Url::parse_with_params(
            ISSUES,
            &[
                ("template", "bug.yml"),
                ("what", what.as_str()),
                ("version", &format!("{BUILD_VERSION}, {install}")),
                ("os", &system),
                ("report", body),
            ],
        )
        .map(String::from)
        .unwrap_or_else(|_| ISSUES.to_string())
    };
    let mut left_out = 0;
    let mut link = link_for(&text);
    while link.len() > MAX_LINK && left_out < lines.len() {
        left_out += 1;
        link = link_for(&compose(&lines[left_out..], left_out));
    }
    Report { text, link, left_out, log: log::path().display().to_string() }
}

/// How this copy of Penates was installed, as far as can be told from where it runs.
pub fn install_kind() -> String {
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    if exe.contains("/target/debug/") || exe.contains("\\target\\debug\\") || exe.contains("/target/release/") || exe.contains("\\target\\release\\") {
        return "own build".into();
    }
    if cfg!(target_os = "linux") {
        if std::env::var_os("APPIMAGE").is_some() {
            return "AppImage".into();
        }
        let pacman: Vec<String> = std::fs::read_dir("/var/lib/pacman/local")
            .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        let has = |name: &str| pacman.iter().any(|p| p.strip_prefix(name).and_then(|v| v.strip_prefix('-')).is_some_and(|v| v.starts_with(|c: char| c.is_ascii_digit())));
        if has("penates-bin") && exe.starts_with("/usr/") {
            return "Arch package penates-bin".into();
        }
        if has("penates") && exe.starts_with("/usr/") {
            return "Arch package penates".into();
        }
        if std::path::Path::new("/var/lib/dpkg/info/penates.list").exists() && exe.starts_with("/usr/") {
            return ".deb package".into();
        }
        if exe.starts_with("/usr/") {
            return "system package".into();
        }
    }
    if cfg!(target_os = "macos") && exe.contains(".app/Contents/MacOS/") {
        return "macOS app".into();
    }
    if cfg!(windows) && (exe.contains("\\Program Files") || exe.contains("\\AppData\\Local\\")) {
        return "Windows installer".into();
    }
    "tarball or own build".into()
}

/// The operating system, version and desktop, e.g. "CachyOS, KDE, wayland, x86_64".
pub fn system() -> String {
    let mut parts = vec![];
    if cfg!(target_os = "linux") {
        let pretty = std::fs::read_to_string("/etc/os-release").ok().and_then(|t| {
            t.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string()))
        });
        parts.push(pretty.unwrap_or_else(|| "Linux".into()));
        for var in ["XDG_CURRENT_DESKTOP", "XDG_SESSION_TYPE"] {
            if let Some(v) = std::env::var(var).ok().filter(|v| !v.is_empty()) {
                parts.push(v);
            }
        }
    } else if cfg!(target_os = "macos") {
        let ver = std::process::Command::new("sw_vers").arg("-productVersion").output().ok();
        let ver = ver.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|v| !v.is_empty());
        parts.push(ver.map_or("macOS".into(), |v| format!("macOS {v}")));
    } else {
        parts.push(std::env::consts::OS.into());
    }
    parts.push(std::env::consts::ARCH.into());
    redact(&parts.join(", "))
}

/// Take personal details out of a line: web addresses keep only their site (no path, query or login),
/// the home folder becomes `~`, and the user name, email addresses and IP addresses become placeholders.
pub fn redact(text: &str) -> String {
    let mut out = strip_addresses(text);
    if let Some(home) = dirs::home_dir().map(|h| h.display().to_string()).filter(|h| h.len() > 1) {
        out = out.replace(&home, "~").replace(&home.replace('\\', "/"), "~");
    }
    let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
    if user.chars().count() >= 3 {
        out = replace_word(&out, &user, "<user>");
    }
    out.split_inclusive(char::is_whitespace).map(redact_token).collect()
}

/// Web addresses cut down to `scheme://host/…`.
fn strip_addresses(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = ["https://", "http://", "file://"].iter().filter_map(|s| rest.find(s)).min() {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let end = tail.find(|c: char| c.is_whitespace() || "\"'<>()[]{},`".contains(c)).unwrap_or(tail.len());
        let address = &tail[..end];
        out.push_str(&match url::Url::parse(address) {
            Ok(u) if u.scheme() != "file" => {
                let site = format!("{}://{}", u.scheme(), u.host_str().unwrap_or(""));
                let more = u.path() != "/" || u.query().is_some() || u.fragment().is_some();
                if more { site + "/…" } else { site + "/" }
            }
            _ => format!("{}…", &address[..address.find("//").map_or(0, |i| i + 2)]),
        });
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

fn replace_word(text: &str, word: &str, with: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find(word) {
        let before_ok = rest[..i].chars().next_back().map_or(true, |c| !c.is_alphanumeric());
        let after_ok = rest[i + word.len()..].chars().next().map_or(true, |c| !c.is_alphanumeric());
        out.push_str(&rest[..i]);
        out.push_str(if before_ok && after_ok { with } else { word });
        rest = &rest[i + word.len()..];
    }
    out.push_str(rest);
    out
}

/// One whitespace-separated piece: an email address or IPv4 address becomes a placeholder.
fn redact_token(token: &str) -> String {
    let core_start = token.find(|c: char| c.is_alphanumeric()).unwrap_or(token.len());
    let core_end = token.rfind(|c: char| c.is_alphanumeric()).map_or(core_start, |i| i + 1);
    if core_start >= core_end {
        return token.to_string();
    }
    let core = &token[core_start..core_end];
    let replacement = if let Some((local, domain)) = core.split_once('@') {
        let ok_local = !local.is_empty() && local.chars().all(|c| c.is_alphanumeric() || "._%+-".contains(c));
        let ok_domain = domain.contains('.') && domain.chars().all(|c| c.is_alphanumeric() || ".-".contains(c));
        (ok_local && ok_domain).then_some("<email>")
    } else {
        let parts: Vec<&str> = core.split('.').collect();
        (parts.len() == 4 && parts.iter().all(|p| !p.is_empty() && p.len() <= 3 && p.parse::<u8>().is_ok())).then_some("<ip>")
    };
    match replacement {
        Some(r) => format!("{}{r}{}", &token[..core_start], &token[core_end..]),
        None => token.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_keep_only_their_site() {
        assert_eq!(
            redact("popup https://mail.google.com/mail/u/0/#inbox?x=1 failed"),
            "popup https://mail.google.com/… failed"
        );
        assert_eq!(redact("at https://user:secret@example.org/ ok"), "at https://example.org/ ok");
        assert_eq!(redact("(see http://example.org)"), "(see http://example.org/)");
        assert_eq!(redact("file:///home/someone/notes.txt"), "file://…");
        assert_eq!(redact("no address here"), "no address here");
    }

    #[test]
    fn emails_and_ips_become_placeholders() {
        assert_eq!(redact("signed in as jo.doe+x@example.co.uk, from 192.168.1.20."), "signed in as <email>, from <ip>.");
        assert_eq!(redact("WebKitGTK 2.48.3 and Penates 0.1.3"), "WebKitGTK 2.48.3 and Penates 0.1.3");
        assert_eq!(redact("@mention and a@b"), "@mention and a@b");
        assert_eq!(redact("999.1.1.1"), "999.1.1.1");
    }

    #[test]
    fn the_home_folder_and_user_name_are_hidden() {
        let home = dirs::home_dir().unwrap().display().to_string();
        assert_eq!(redact(&format!("reading {home}/.config/penates/apps/x.toml")), "reading ~/.config/penates/apps/x.toml");
        assert_eq!(replace_word("bob, bobby and bob.", "bob", "<user>"), "<user>, bobby and <user>.");
    }

    #[test]
    fn the_link_fills_the_bug_form_and_stays_short() {
        let r = build("It froze when I clicked https://example.org/private/page", Some("WebKitGTK 2.48.3"));
        assert!(r.text.starts_with(&format!("Penates {BUILD_VERSION} (")));
        assert!(r.text.contains("Web view: WebKitGTK 2.48.3"));
        assert!(r.link.starts_with("https://github.com/Natetronn/penates/issues/new?template=bug.yml&what="));
        assert!(r.link.contains("example.org%2F%E2%80%A6"), "the description is redacted too: {}", r.link);
        assert!(!r.link.contains("private"));
        assert!(r.link.len() <= MAX_LINK);
    }
}
