//! Penates's own settings (`~/.config/penates/settings.toml`): update checks, the preset catalog
//! address and the manager's look.

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
}
impl Default for Appearance {
    fn default() -> Self {
        Self { theme: "system".into(), skin: "default".into(), accent: "violet".into() }
    }
}
impl Appearance {
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(THEMES.contains(&self.theme.as_str()), "unknown theme '{}'", self.theme);
        anyhow::ensure!(SKINS.contains(&self.skin.as_str()), "unknown look '{}'", self.skin);
        anyhow::ensure!(ACCENTS.contains(&self.accent.as_str()), "unknown accent '{}'", self.accent);
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub updates: UpdateSettings,
    pub appearance: Appearance,
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
}
