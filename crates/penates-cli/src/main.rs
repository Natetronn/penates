use anyhow::{bail, Context, Result};
use penates_core::{icon, integration, resolve_name, normalize_url, ops, slugify, store, updates, WebApp};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "penates", version, about = "Turn websites into native desktop apps")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a new app from a URL
    Create {
        url: String,
        #[arg(short, long)]
        name: Option<String>,
        /// Custom id (defaults to slug of name)
        #[arg(long)]
        id: Option<String>,
        /// Icon: a local image file or an http(s) URL (otherwise fetched from the site)
        #[arg(short, long)]
        icon: Option<String>,
        #[arg(long)]
        user_agent: Option<String>,
        #[arg(long, default_value_t = 1200.0)]
        width: f64,
        #[arg(long, default_value_t = 800.0)]
        height: f64,
        #[arg(long, default_value_t = 1.0)]
        zoom: f64,
        /// Extra host that stays in-app (repeatable, supports *.wildcard)
        #[arg(long = "internal-host")]
        internal_hosts: Vec<String>,
        /// File of CSS to inject
        #[arg(long)]
        inject_css: Option<PathBuf>,
        /// File of JS to inject
        #[arg(long)]
        inject_js: Option<PathBuf>,
        #[arg(long)]
        devtools: bool,
        /// Don't create a launcher in the application menu
        #[arg(long)]
        no_install: bool,
        /// Overwrite if the id already exists
        #[arg(long)]
        force: bool,
    },
    /// List apps
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show an app's config
    Show { id: String },
    /// Print the path of an app's config file (for editing)
    Path { id: String },
    /// Run an app
    Run { id: String },
    /// (Re)create the application-menu launcher
    Install { id: String },
    /// Set an app's icon from a file or URL, or re-fetch the site's own
    Icon {
        id: String,
        /// Image file or http(s) URL; omit to re-fetch the site's own icon
        source: Option<String>,
    },
    /// List the icons found for a site (PWA manifest, apple-touch, favicon), best first
    Icons { url: String },
    /// Clear an app's cache (keeps logins), or with --data its cookies and storage too
    Clear {
        id: String,
        #[arg(long)]
        data: bool,
    },
    /// Remove an app
    Remove {
        id: String,
        /// Also delete cookies/storage
        #[arg(long)]
        purge: bool,
    },
    /// Check for Penates and app-preset updates
    Check,
    /// Update one app (preset sync + icon refresh), or all with --all
    Update {
        id: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Open an app's config file in $VISUAL/$EDITOR (or the default app)
    Edit { id: String },
    /// Open the desktop manager
    Gui,
}

fn fmt_bytes(b: u64) -> String {
    match b {
        0..=1023 => format!("{b} B"),
        1024..=1048575 => format!("{:.0} KB", b as f64 / 1024.0),
        _ => format!("{:.1} MB", b as f64 / 1048576.0),
    }
}

fn desktop_bin() -> Result<PathBuf> {
    integration::find_desktop_bin().context("penates-desktop not found (put it next to `penates`, on PATH, or set PENATES_DESKTOP)")
}

fn read_opt(p: Option<PathBuf>) -> Result<Option<String>> {
    p.map(|p| std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))).transpose()
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Create {
            url, name, id, icon: icon_file, user_agent, width, height, zoom, internal_hosts,
            inject_css, inject_js, devtools, no_install, force,
        } => {
            let url = normalize_url(&url)?;
            let name = name.unwrap_or_else(|| resolve_name(&url));
            let id = id.unwrap_or_else(|| slugify(&name));
            if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                bail!("invalid id '{id}' (use letters, digits, - and _)");
            }
            if store::exists(&id) && !force {
                bail!("'{id}' already exists (use --force or --id)");
            }
            let icon_file_given = icon_file.is_some();
            let icon_path = match icon_file {
                Some(f) => Some(icon::import_source(&f, &id)?),
                None => match icon::fetch_for(&url, &id) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        eprintln!("warning: {e}");
                        None
                    }
                },
            };
            let app = WebApp {
                id: id.clone(), name, url, user_agent, width, height, zoom, internal_hosts, devtools,
                custom_icon: icon_file_given,
                icon: icon_path.map(|p| store::icon_ref(&p)),
                inject_css: read_opt(inject_css)?,
                inject_js: read_opt(inject_js)?,
                ..Default::default()
            };
            store::save(&app)?;
            println!("created '{}' ({})", app.name, app.id);
            if !no_install {
                match desktop_bin().and_then(|b| integration::install(&app, &b)) {
                    Ok(p) => println!("launcher: {}", p.display()),
                    Err(e) => eprintln!("warning: launcher not installed: {e}"),
                }
            }
        }
        Cmd::List { json } => {
            let apps = store::list()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&apps)?);
            } else if apps.is_empty() {
                println!("no apps yet — try `penates create https://example.com`");
            } else {
                for a in apps {
                    println!("{:<20} {:<24} {}", a.id, a.name, a.url);
                }
            }
        }
        Cmd::Show { id } => print!("{}", std::fs::read_to_string(store::path_for(&id)).with_context(|| format!("no app '{id}'"))?),
        Cmd::Path { id } => println!("{}", store::path_for(&id).display()),
        Cmd::Run { id } => {
            store::load(&id)?;
            Command::new(desktop_bin()?)
                .args(["--run", &id])
                .stdin(Stdio::null())
                .spawn()
                .context("failed to launch penates-desktop")?;
        }
        Cmd::Install { id } => {
            let p = integration::install(&store::load(&id)?, &desktop_bin()?)?;
            println!("launcher: {}", p.display());
        }
        Cmd::Icon { id, source } => {
            let app = store::load(&id)?;
            let (p, custom) = match source {
                Some(f) => (icon::import_source(&f, &id)?, true),
                None => (icon::fetch_for(&app.url, &id)?, false),
            };
            ops::set_icon_file(&id, p.clone(), custom)?;
            println!("icon: {}", p.display());
        }
        Cmd::Icons { url } => {
            for (i, c) in icon::candidates(&normalize_url(&url)?).into_iter().enumerate() {
                println!("{:>2}. {c}", i + 1);
            }
        }
        Cmd::Clear { id, data } => {
            store::load(&id)?;
            if penates_core::single::is_running(&id) {
                bail!("'{id}' is running; close it first");
            }
            if data {
                store::clear_profile(&id);
                println!("cleared cookies and data for '{id}'");
            } else {
                println!("cleared {} of cache for '{id}'", fmt_bytes(store::clear_cache(&id)));
            }
        }
        Cmd::Remove { id, purge } => {
            store::load(&id)?;
            store::remove(&id, purge)?;
            println!("removed '{id}'{}", if purge { " and its data" } else { "" });
        }
        Cmd::Check => {
            let r = updates::check()?;
            println!("penates {} (source: {})", r.current, r.source);
            match r.penates {
                Some(u) => {
                    println!("  new version {} available — {}", u.latest, u.notes);
                    if let Some(d) = u.download_url {
                        println!("  download: {d}");
                    }
                }
                None => println!("  penates is up to date"),
            }
            for a in &r.apps {
                println!("  {:<18} preset {} -> {}  {}", a.id, a.from.as_deref().unwrap_or("none"), a.to, a.notes);
            }
            if r.apps.is_empty() {
                println!("  all apps are up to date");
            } else {
                println!("run `penates update --all` to apply");
            }
        }
        Cmd::Update { id, all } => {
            let outs = match (id, all) {
                (Some(id), _) => vec![ops::update(&id)?],
                (None, true) => ops::update_all()?,
                (None, false) => bail!("give an app id or --all"),
            };
            for o in outs {
                println!(
                    "{:<18} icon {}{}",
                    o.app.id,
                    if o.icon_refreshed { "refreshed" } else { "unchanged" },
                    o.preset.map(|v| format!(", preset {v} applied")).unwrap_or_default()
                );
            }
        }
        Cmd::Edit { id } => {
            store::load(&id)?;
            let path = store::path_for(&id);
            match std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")) {
                Ok(ed) => {
                    Command::new(ed).arg(&path).status()?;
                    // validate + refresh launcher
                    let app = store::load(&id).context("config no longer parses")?;
                    store::save(&app)?;
                    ops::reinstall(Some(&app), &app);
                }
                Err(_) => open::that(&path)?,
            }
        }
        Cmd::Gui => {
            Command::new(desktop_bin()?).stdin(Stdio::null()).spawn().context("failed to launch penates-desktop")?;
        }
    }
    Ok(())
}
