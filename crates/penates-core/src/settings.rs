//! Penates's own settings (`~/.config/penates/settings.toml`): update checks, the preset catalog
//! address, the manager's look and where it saves exports.

use crate::store;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UpdateSettings {
    /// Preset catalog: an https:// or file:// URL, or a local path. Empty = the Penates app catalog
    /// (the built-in copy when it cannot be reached).
    #[serde(alias = "manifest_url")]
    pub catalog_url: String,
    pub check_on_start: bool,
}
impl Default for UpdateSettings {
    fn default() -> Self {
        Self { catalog_url: String::new(), check_on_start: true }
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
            eprintln!("warning: {} could not be read ({e}); moved it to {} and used defaults", p.display(), aside.display());
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
    fn manifest_url_is_another_name_for_catalog_url() {
        let s: Settings = toml::from_str("[updates]\nmanifest_url = \"file:///x.json\"\n").unwrap();
        assert_eq!(s.updates.catalog_url, "file:///x.json");
        assert!(s.updates.check_on_start);
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
