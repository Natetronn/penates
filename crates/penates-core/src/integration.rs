//! OS launcher integration: .desktop (Linux), .app bundle (macOS), Start Menu shortcut (Windows).

use crate::WebApp;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// WM class / Wayland app_id the runner window uses so docks group it correctly (Linux).
pub fn wm_class(id: &str) -> String {
    format!("penates-{id}")
}

pub fn desktop_exe_name() -> &'static str {
    if cfg!(windows) { "penates-desktop.exe" } else { "penates-desktop" }
}

/// Locate the desktop binary: $PENATES_DESKTOP, next to this exe, then $PATH.
pub fn find_desktop_bin() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("PENATES_DESKTOP") {
        return Some(p.into());
    }
    // Inside an AppImage the running exe lives in a temporary mount; launchers must use the file.
    if let Some(p) = std::env::var_os("APPIMAGE") {
        return Some(p.into());
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
        let p = dir.join(desktop_exe_name());
        if p.exists() {
            return Some(p);
        }
    }
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(desktop_exe_name())).find(|p| p.exists())
}

/// File name for the app's launcher on macOS and Windows. Two apps can share a display name (e.g.
/// both "Google"), so the id is added whenever the name alone would not identify the app.
#[cfg(any(target_os = "macos", windows, test))]
fn safe_name(app: &WebApp) -> String {
    let n: String = app.name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect();
    let n = n.trim().to_string();
    if n.is_empty() {
        app.id.clone()
    } else if crate::slugify(&n) == app.id {
        n
    } else {
        format!("{n} ({})", app.id)
    }
}

pub use imp::{install_quiet, is_installed, refresh_caches, uninstall_id};
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub use imp::install_manager;

/// Write (or rewrite) the app's launcher and let the desktop pick it up.
pub fn install(app: &WebApp, bin: &Path) -> Result<PathBuf> {
    let p = install_quiet(app, bin)?;
    refresh_caches();
    Ok(p)
}

/// Remove the app's launcher (best effort).
pub fn uninstall(app: &WebApp) {
    imp::remove(app);
    refresh_caches();
}

/// Remove the app's launcher without refreshing the desktop's caches (callers refresh once).
pub fn uninstall_quiet(app: &WebApp) {
    imp::remove(app);
}

// ---------------------------------------------------------------- Linux / other unix
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use super::*;

    fn dir() -> PathBuf {
        dirs::data_dir().expect("no data dir").join("applications")
    }
    fn file(id: &str) -> PathBuf {
        dir().join(format!("penates-{id}.desktop"))
    }
    fn quote(s: &str) -> String {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('$', "\\$").replace('`', "\\`"))
    }

    /// Exec= value for a program path: quoted, with `%` doubled (the spec reserves `%` for field codes).
    fn exec(bin: &Path) -> String {
        quote(&bin.to_string_lossy()).replace('%', "%%")
    }

    pub fn install_quiet(app: &WebApp, bin: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dir())?;
        let icon = app.icon_path().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "applications-internet".into());
        let content = format!(
            "[Desktop Entry]\nType=Application\nVersion=1.0\nName={name}\nComment=Web app: {url}\nExec={exec} --run {id}\nIcon={icon}\nTerminal=false\nCategories=Network;\nStartupWMClass={class}\nStartupNotify=true\nX-Penates-Id={id}\n",
            name = app.name.replace('\n', " "),
            url = app.url,
            exec = exec(bin),
            id = app.id,
            class = wm_class(&app.id),
        );
        let p = file(&app.id);
        crate::store::write_atomic(&p, content.as_bytes())?;
        Ok(p)
    }

    pub fn remove(app: &WebApp) {
        let _ = std::fs::remove_file(file(&app.id));
    }

    pub fn uninstall_id(id: &str) {
        let _ = std::fs::remove_file(file(id));
        refresh_caches();
    }

    pub fn is_installed(app: &WebApp) -> bool {
        file(&app.id).exists()
    }

    /// Make the desktop pick up new/changed launchers right away. KDE otherwise keeps a stale
    /// app database and shows a generic fallback icon for windows opened before it refreshes.
    pub fn refresh_caches() {
        use std::process::{Command, Stdio};
        let quiet = |mut c: Command| {
            let _ = c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
        };
        let mut u = Command::new("update-desktop-database");
        u.arg(dir());
        quiet(u);
        for bin in ["kbuildsycoca6", "kbuildsycoca5"] {
            let mut k = Command::new(bin);
            if k.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() {
                break;
            }
        }
    }

    /// Launcher for the manager window itself (WM class "penates") so it gets a proper taskbar icon.
    pub fn install_manager(bin: &Path, icon_png: &[u8]) -> Result<()> {
        std::fs::create_dir_all(dir())?;
        std::fs::create_dir_all(crate::store::icon_dir())?;
        let icon = crate::store::icon_dir().join("penates.png");
        std::fs::write(&icon, icon_png)?;
        let content = format!(
            "[Desktop Entry]\nType=Application\nVersion=1.0\nName=Penates\nComment=Turn websites into desktop apps\nExec={}\nIcon={}\nTerminal=false\nCategories=Network;Utility;\nStartupWMClass=penates\nStartupNotify=true\n",
            exec(bin),
            icon.display()
        );
        std::fs::write(dir().join("penates.desktop"), content)?;
        refresh_caches();
        Ok(())
    }
}

// ---------------------------------------------------------------- macOS
#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn root() -> PathBuf {
        dirs::home_dir().expect("no home").join("Applications").join("Penates")
    }
    fn bundle(app: &WebApp) -> PathBuf {
        root().join(format!("{}.app", safe_name(app)))
    }
    fn xml(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }

    pub fn install_quiet(app: &WebApp, bin: &Path) -> Result<PathBuf> {
        let b = bundle(app);
        let _ = std::fs::remove_dir_all(&b);
        std::fs::create_dir_all(b.join("Contents/MacOS"))?;
        std::fs::create_dir_all(b.join("Contents/Resources"))?;
        // The running process must live inside the bundle for the Dock to use its name and icon,
        // so link (or copy) the binary in and have the launcher exec that copy.
        let inner = b.join("Contents/MacOS/penates-desktop");
        if std::fs::hard_link(bin, &inner).is_err() {
            std::fs::copy(bin, &inner)?;
        }
        let launcher = b.join("Contents/MacOS/launcher");
        std::fs::write(&launcher, format!("#!/bin/sh\nexec \"$(dirname \"$0\")/penates-desktop\" --run {}\n", app.id))?;
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755))?;
        let mut has_icon = false;
        if let Some(png) = app.icon_path() {
            let img = image::open(&png)?.to_rgba8();
            let im = icns::Image::from_data(icns::PixelFormat::RGBA, img.width(), img.height(), img.into_raw())?;
            let mut fam = icns::IconFamily::new();
            fam.add_icon(&im)?;
            fam.write(std::io::BufWriter::new(std::fs::File::create(b.join("Contents/Resources/icon.icns"))?))?;
            has_icon = true;
        }
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>{n}</string>
<key>CFBundleDisplayName</key><string>{n}</string>
<key>CFBundleIdentifier</key><string>dev.penates.app.{id}</string>
<key>CFBundleExecutable</key><string>launcher</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>NSCameraUsageDescription</key><string>The website in this app wants to use your camera.</string>
<key>NSMicrophoneUsageDescription</key><string>The website in this app wants to use your microphone.</string>
{icon}</dict></plist>
"#,
            n = xml(&app.name),
            id = app.id,
            icon = if has_icon { "<key>CFBundleIconFile</key><string>icon</string>\n" } else { "" },
        );
        std::fs::write(b.join("Contents/Info.plist"), plist)?;
        Ok(b)
    }

    pub fn remove(app: &WebApp) {
        let _ = std::fs::remove_dir_all(bundle(app));
    }

    /// Find the bundle by the bundle id written into its Info.plist (the name may be unknown).
    pub fn uninstall_id(id: &str) {
        let needle = format!("<string>dev.penates.app.{id}</string>");
        for b in std::fs::read_dir(root()).into_iter().flatten().flatten().map(|e| e.path()) {
            if std::fs::read_to_string(b.join("Contents/Info.plist")).is_ok_and(|p| p.contains(&needle)) {
                let _ = std::fs::remove_dir_all(b);
            }
        }
    }

    pub fn is_installed(app: &WebApp) -> bool {
        bundle(app).exists()
    }

    pub fn refresh_caches() {}
}

// ---------------------------------------------------------------- Windows
#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;

    fn dir() -> PathBuf {
        dirs::data_dir().expect("no appdata").join("Microsoft/Windows/Start Menu/Programs/Penates")
    }
    fn lnk(app: &WebApp) -> PathBuf {
        dir().join(format!("{}.lnk", safe_name(app)))
    }

    pub fn install_quiet(app: &WebApp, bin: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dir())?;
        let mut icon = String::new();
        if let Some(png) = app.icon_path() {
            let ico = crate::store::icon_dir().join(format!("{}.ico", app.id));
            image::open(&png)?.save_with_format(&ico, image::ImageFormat::Ico)?;
            icon = ico.to_string_lossy().into_owned();
        }
        let l = lnk(app);
        // Values go through env vars so names/paths are never interpolated into the script.
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command",
                "$s=(New-Object -ComObject WScript.Shell).CreateShortcut($env:PENATES_LNK);$s.TargetPath=$env:PENATES_TARGET;$s.Arguments=$env:PENATES_ARGS;if($env:PENATES_ICON){$s.IconLocation=$env:PENATES_ICON};$s.Description=$env:PENATES_DESC;$s.Save()"])
            .env("PENATES_LNK", &l)
            .env("PENATES_TARGET", bin)
            .env("PENATES_ARGS", format!("--run {}", app.id))
            .env("PENATES_ICON", icon)
            .env("PENATES_DESC", format!("Web app: {}", app.url))
            .creation_flags(0x0800_0000)
            .status()?;
        anyhow::ensure!(status.success(), "powershell failed to create the shortcut");
        Ok(l)
    }

    pub fn remove(app: &WebApp) {
        let _ = std::fs::remove_file(lnk(app));
    }

    /// Find the shortcut by its arguments (the name may be unknown). A .lnk stores them as UTF-16
    /// preceded by their length, so matching the length too keeps `github` from matching `github-2`.
    pub fn uninstall_id(id: &str) {
        let args: Vec<u16> = format!("--run {id}").encode_utf16().collect();
        let mut needle = (args.len() as u16).to_le_bytes().to_vec();
        needle.extend(args.iter().flat_map(|u| u.to_le_bytes()));
        for l in std::fs::read_dir(dir()).into_iter().flatten().flatten().map(|e| e.path()) {
            if std::fs::read(&l).is_ok_and(|b| b.windows(needle.len()).any(|w| w == needle)) {
                let _ = std::fs::remove_file(l);
            }
        }
    }

    pub fn is_installed(app: &WebApp) -> bool {
        lnk(app).exists()
    }

    pub fn refresh_caches() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_names_do_not_collide() {
        let a = WebApp { id: "google".into(), name: "Google".into(), ..Default::default() };
        let b = WebApp { id: "google-2".into(), name: "Google".into(), ..Default::default() };
        assert_eq!(safe_name(&a), "Google");
        assert_eq!(safe_name(&b), "Google (google-2)");
    }
}
