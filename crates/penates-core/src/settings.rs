//! Penates's own settings (`~/.config/penates/settings.toml`): update checks, the preset catalogs,
//! the manager's look and where it saves exports.

use crate::store;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(from = "RawUpdateSettings")]
pub struct UpdateSettings {
    /// Whether the Penates app catalog is used (the built-in copy when it cannot be reached).
    pub penates_catalog: bool,
    /// Your own preset catalogs, used together with it: https:// or file:// URLs, or local paths. Their
    /// entries replace the Penates catalog's entries with the same id; among them, the first one listed wins.
    pub catalogs: Vec<String>,
    pub check_on_start: bool,
}
impl Default for UpdateSettings {
    fn default() -> Self {
        Self { penates_catalog: true, catalogs: Vec::new(), check_on_start: true }
    }
}

/// `[updates]` as written, including the single `catalog_url` (or `manifest_url`) address, which meant
/// "this catalog instead of the Penates one": read as the only entry of `catalogs`, with the Penates catalog off.
#[derive(Deserialize)]
#[serde(default)]
struct RawUpdateSettings {
    penates_catalog: Option<bool>,
    catalogs: Vec<String>,
    #[serde(alias = "manifest_url")]
    catalog_url: String,
    check_on_start: bool,
}
impl Default for RawUpdateSettings {
    fn default() -> Self {
        Self { penates_catalog: None, catalogs: Vec::new(), catalog_url: String::new(), check_on_start: true }
    }
}
impl From<RawUpdateSettings> for UpdateSettings {
    fn from(r: RawUpdateSettings) -> Self {
        let mut catalogs: Vec<String> = Vec::new();
        let single = r.catalog_url.trim().to_string();
        for c in r.catalogs.into_iter().chain((!single.is_empty()).then(|| single.clone())) {
            let c = c.trim().to_string();
            if !c.is_empty() && !catalogs.contains(&c) {
                catalogs.push(c);
            }
        }
        Self { penates_catalog: r.penates_catalog.unwrap_or(single.is_empty()), catalogs, check_on_start: r.check_on_start }
    }
}

pub const THEMES: [&str; 3] = ["system", "light", "dark"];
pub const SKINS: [&str; 4] = ["default", "nord", "solarized", "mono"];
pub const ACCENTS: [&str; 6] = ["violet", "blue", "green", "orange", "rose", "slate"];
/// The default toolbar color for apps: each site's own color, or Penates's.
pub const TOOLBAR_COLORS: [&str; 2] = ["site", "penates"];

/// Look of the manager window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Appearance {
    /// One of `THEMES`.
    pub theme: String,
    /// One of `SKINS` (ui/skins.css).
    pub skin: String,
    /// One of `ACCENTS` (ui/tokens.css).
    pub accent: String,
    /// One of `TOOLBAR_COLORS`: the toolbar color of apps that use the default.
    pub toolbar_color: String,
}
impl Default for Appearance {
    fn default() -> Self {
        Self { theme: "system".into(), skin: "default".into(), accent: "violet".into(), toolbar_color: "site".into() }
    }
}
impl Appearance {
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(THEMES.contains(&self.theme.as_str()), "unknown theme '{}'", self.theme);
        anyhow::ensure!(SKINS.contains(&self.skin.as_str()), "unknown look '{}'", self.skin);
        anyhow::ensure!(ACCENTS.contains(&self.accent.as_str()), "unknown accent '{}'", self.accent);
        anyhow::ensure!(TOOLBAR_COLORS.contains(&self.toolbar_color.as_str()), "unknown toolbar color '{}'", self.toolbar_color);
        Ok(())
    }
}

/// Where the manager saves exports and backups.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SharingSettings {
    /// Folder for exports and backups. Empty: the Downloads folder.
    pub export_dir: String,
}
impl SharingSettings {
    /// The folder exports go to: the chosen one while it exists, else Downloads (else the home folder).
    pub fn export_folder(&self) -> std::path::PathBuf {
        let chosen = std::path::Path::new(self.export_dir.trim());
        if !self.export_dir.trim().is_empty() && chosen.is_absolute() && chosen.is_dir() {
            return chosen.to_path_buf();
        }
        dirs::download_dir().or_else(dirs::home_dir).unwrap_or_else(std::env::temp_dir)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub updates: UpdateSettings,
    pub appearance: Appearance,
    pub sharing: SharingSettings,
    /// Version of Penates that last rewrote the launchers, and the program they point at
    /// (see `ops::refresh_launchers_if_needed`).
    pub launchers_version: String,
    pub launchers_bin: String,
}

fn path() -> std::path::PathBuf {
    store::root_config_dir().join("settings.toml")
}

/// The saved settings, or defaults. A file that does not parse is moved aside to
/// `settings.toml.broken` (so the next save cannot silently destroy it) and reported on stderr.
pub fn load() -> Settings {
    let p = path();
    let Ok(text) = std::fs::read_to_string(&p) else { return Settings::default() };
    match toml::from_str(&text) {
        Ok(s) => s,
        Err(e) => {
            let aside = p.with_extension("toml.broken");
            let _ = std::fs::rename(&p, &aside);
            crate::log::warn(&format!("{} could not be read ({e}); moved it to {} and used defaults", p.display(), aside.display()));
            Settings::default()
        }
    }
}

pub fn save(s: &Settings) -> Result<()> {
    store::write_atomic(&path(), toml::to_string_pretty(s)?.as_bytes())
}

/// Load, change and save in one step, so concurrent writers only touch the fields they mean to.
pub fn update(f: impl FnOnce(&mut Settings)) -> Result<Settings> {
    let mut s = load();
    f(&mut s);
    save(&s)?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_catalog_address_becomes_the_only_catalog() {
        for key in ["catalog_url", "manifest_url"] {
            let s: Settings = toml::from_str(&format!("[updates]\n{key} = \"file:///x.json\"\n")).unwrap();
            assert_eq!(s.updates.catalogs, ["file:///x.json"]);
            assert!(!s.updates.penates_catalog && s.updates.check_on_start);
        }
        let s: Settings = toml::from_str("[updates]\ncatalog_url = \"\"\n").unwrap();
        assert!(s.updates.penates_catalog && s.updates.catalogs.is_empty());
    }

    #[test]
    fn catalog_list_round_trips() {
        let mut s = Settings::default();
        s.updates.catalogs = vec!["/home/me/mine.json".into(), "https://example.com/team.json".into()];
        s.updates.penates_catalog = false;
        let back: Settings = toml::from_str(&toml::to_string_pretty(&s).unwrap()).unwrap();
        assert_eq!(back.updates, s.updates);
        assert!(!toml::to_string_pretty(&s).unwrap().contains("catalog_url"));
    }

    #[test]
    fn appearance_is_checked() {
        assert!(Appearance::default().validate().is_ok());
        assert!(Appearance { accent: "neon".into(), ..Default::default() }.validate().is_err());
    }

    #[test]
    fn export_folder_is_the_chosen_one_only_while_it_exists() {
        let downloads = SharingSettings::default().export_folder();
        let dir = std::env::temp_dir().join(format!("penates-export-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let chosen = SharingSettings { export_dir: dir.display().to_string() };
        assert_eq!(chosen.export_folder(), dir);
        std::fs::remove_dir(&dir).unwrap();
        assert_eq!(chosen.export_folder(), downloads, "a folder that is gone falls back to Downloads");
        let relative = SharingSettings { export_dir: "backups".into() };
        assert_eq!(relative.export_folder(), downloads, "only absolute paths count");
    }
}
