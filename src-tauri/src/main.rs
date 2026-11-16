#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! `penates-desktop`: the manager window, or with `--run <id>` one wrapped web app.

mod commands;
mod runner;

use penates_core::{ops, settings, single, store};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

/// The macro embeds the app's Info.plist as a global symbol, so it may only be expanded once
/// (twice is a hard link error on macOS).
fn app_context() -> tauri::Context {
    tauri::generate_context!()
}

/// Script run before the manager's page loads: the start view (`--tab`) and the saved look, so the
/// window never flashes the default theme first.
fn manager_init_script(start: Option<&str>) -> String {
    let look = settings::load().appearance;
    format!(
        "window.__PENATES_START = {};\nwindow.__PENATES_APPEARANCE = {};",
        serde_json::to_string(&start.unwrap_or("")).unwrap(),
        serde_json::to_string(&look).unwrap()
    )
}

fn run_manager(start: Option<String>) -> tauri::Result<()> {
    // One manager at a time: a second start brings the open one to the front.
    let Some(listener) = single::try_bind(single::MANAGER) else {
        if single::request_focus(single::MANAGER) {
            return Ok(());
        }
        return run_manager_window(start, None);
    };
    run_manager_window(start, Some(listener))
}

fn run_manager_window(start: Option<String>, listener: Option<std::net::TcpListener>) -> tauri::Result<()> {
    // The window's app_id is "penates" and must equal a launcher file name (penates.desktop) for the
    // desktop to show the right icon. Packages ship that file; dev runs write it.
    #[cfg(target_os = "linux")]
    gtk::glib::set_prgname(Some("penates"));
    #[cfg(all(target_os = "linux", debug_assertions))]
    if let Ok(exe) = std::env::current_exe() {
        let _ = penates_core::integration::install_manager(&exe, include_bytes!("../icons/icon.png"));
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_apps, commands::app_version, commands::suggest_name, commands::create_app,
            commands::save_app, commands::remove_app, commands::run_app,
            commands::cache_info, commands::clear_cache, commands::clear_data,
            commands::icon_data, commands::icon_candidates, commands::set_icon_from_url,
            commands::set_icon_from_bytes, commands::reset_icon,
            commands::update_app, commands::update_all, commands::check_apps, commands::catalog_apps, commands::check_catalog, commands::pick_catalog_file, commands::pick_file, commands::catalog_icon, commands::catalog_submission,
            commands::check_self_update, commands::install_self_update,
            commands::get_update_settings, commands::set_update_settings, commands::get_appearance, commands::set_appearance,
            commands::export_apps, commands::inspect_import, commands::import_apps, commands::open_url,
            commands::get_export_folder, commands::set_export_folder, commands::pick_export_folder,
        ])
        .setup(move |app| {
            if let Some(l) = listener {
                let handle = app.handle().clone();
                single::serve(l, single::MANAGER.into(), move || {
                    if let Some(w) = handle.get_webview_window("manager") {
                        let _ = w.unminimize();
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                });
            }
            // After a Penates update (or a move), refresh every app's launcher, off the UI thread.
            std::thread::spawn(ops::refresh_launchers_if_needed);
            WebviewWindowBuilder::new(app, "manager", WebviewUrl::App("index.html".into()))
                .initialization_script(manager_init_script(start.as_deref()))
                .title("Penates")
                .theme(runner::theme_of(&settings::load().appearance.theme))
                .inner_size(860.0, 720.0)
                .build()?;
            Ok(())
        })
        .run(app_context())
}

fn arg_after<'a>(args: &'a [String], flag: &str) -> Option<&'a String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = if args.iter().any(|a| a == "--run") {
        let Some(id) = arg_after(&args, "--run") else {
            eprintln!("usage: penates-desktop --run <id>");
            std::process::exit(2);
        };
        match store::load(id).and_then(|mut cfg| ops::validate(&mut cfg).map(|_| cfg)) {
            Ok(cfg) => runner::run(cfg, app_context()),
            Err(e) => {
                eprintln!("error: {e:#}");
                std::process::exit(1);
            }
        }
    } else {
        run_manager(arg_after(&args, "--tab").cloned())
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
