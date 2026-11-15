use anyhow::{bail, Context, Result};
use penates_core::{export, icon, integration, resolve_name, normalize_url, ops, slugify, store, updates, WebApp};
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
    /// Export apps to a .penates file (share one app, or back up all). Never includes logins.
    Export {
        /// App ids; or use --all
        ids: Vec<String>,
        #[arg(long)]
        all: bool,
        /// Also include theme, accent and update-source settings (for backups)
        #[arg(long)]
        settings: bool,
        /// Output file ("-" for stdout). Default: <id>.penates, or penates-backup.penates
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Import apps from a .penates file or URL
    Import {
        source: String,
        /// When an id already exists: skip, rename (keep both) or overwrite
        #[arg(long, default_value = "rename")]
        on_conflict: String,
        /// Keep custom JavaScript (dropped by default; it runs inside the site)
        #[arg(long)]
        allow_js: bool,
        /// Also apply the file's settings, including the update source
        #[arg(long)]
        settings: bool,
        /// Don't ask for confirmation
        #[arg(short, long)]
        yes: bool,
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
            let mut app = WebApp {
                id: id.clone(), name, url, user_agent, width, height, zoom, internal_hosts, devtools,
                custom_icon: icon_file_given,
                icon: icon_path.map(|p| store::icon_ref(&p)),
                inject_css: read_opt(inject_css)?,
                inject_js: read_opt(inject_js)?,
                ..Default::default()
            };
            ops::validate(&mut app)?;
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
            for (id, why) in store::broken() {
                eprintln!("warning: '{id}' has a broken config and is skipped: {why}");
            }
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
        Cmd::Show { id } => {
            store::load(&id)?;
            print!("{}", std::fs::read_to_string(store::path_for(&id))?);
        }
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
            if !store::CAN_CLEAR_DATA {
                bail!("clearing data is not supported on macOS yet");
            }
            if data {
                store::clear_profile(&id);
                println!("cleared cookies and data for '{id}'");
            } else {
                println!("cleared {} of cache for '{id}'", fmt_bytes(store::clear_cache(&id)));
            }
        }
        Cmd::Export { ids, all, settings, output } => {
            if ids.is_empty() && !all {
                bail!("name the apps to export, or use --all");
            }
            let f = export::build(if all { &[] } else { &ids }, settings)?;
            let text = serde_json::to_string_pretty(&f)?;
            let out = output.unwrap_or_else(|| match f.apps.as_slice() {
                [one] => format!("{}.penates", one.config.id),
                _ => "penates-backup.penates".into(),
            });
            if out == "-" {
                println!("{text}");
            } else {
                std::fs::write(&out, text)?;
                println!("exported {} app{} to {out}", f.apps.len(), if f.apps.len() == 1 { "" } else { "s" });
                println!("(contains configs and icons only; no cookies or logins)");
            }
        }
        Cmd::Import { source, on_conflict, allow_js, settings, yes } => {
            let f = export::parse(&export::read_source(&source)?)?;
            println!("{} app{} in {source}:", f.apps.len(), if f.apps.len() == 1 { "" } else { "s" });
            let mut any_js = false;
            for s in export::inspect(&f) {
                any_js |= s.custom_js;
                let mut flags = vec![];
                if s.exists { flags.push("already exists"); }
                if s.custom_js { flags.push("CUSTOM JAVASCRIPT"); }
                if s.custom_css { flags.push("custom css"); }
                if s.user_agent { flags.push("custom user agent"); }
                if s.all_links_in_app { flags.push("keeps ALL links in the app"); }
                else if s.extra_hosts > 0 { flags.push("keeps extra sites in the app"); }
                println!("  {:<18} {}  {}", s.id, s.url, if flags.is_empty() { String::new() } else { format!("[{}]", flags.join(", ")) });
            }
            if f.settings.is_some() {
                println!("  (file also has settings{})", if settings { ", which WILL be applied, including the update source" } else { "; ignored unless --settings" });
            }
            if any_js && !allow_js {
                println!("custom JavaScript will be removed (it runs inside the site). Use --allow-js to keep it, only if you trust the source.");
            }
            if !yes {
                use std::io::{IsTerminal, Write};
                if !std::io::stdin().is_terminal() {
                    bail!("not a terminal; pass --yes to import without asking");
                }
                print!("import? [y/N] ");
                std::io::stdout().flush()?;
                let mut line = String::new();
                std::io::stdin().read_line(&mut line)?;
                if !matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                    bail!("cancelled");
                }
            }
            let opts = export::ImportOptions { on_conflict: on_conflict.parse()?, allow_js, settings };
            let r = export::apply(&f, &opts);
            for id in &r.imported { println!("imported {id}"); }
            for (a, b) in &r.renamed { println!("  ({a} already existed; saved as {b})"); }
            for id in &r.skipped { println!("skipped {id} (already exists)"); }
            for id in &r.js_removed { println!("removed custom JavaScript from {id}"); }
            if r.settings_applied { println!("applied settings"); }
            for e in &r.errors { eprintln!("error: {e}"); }
            if !r.errors.is_empty() { std::process::exit(1); }
        }
        Cmd::Remove { id, purge } => {
            store::load(&id)?;
            store::remove(&id, purge)?;
            println!("removed '{id}'{}", if purge && store::CAN_CLEAR_DATA { " and its data" } else { "" });
            if purge && !store::CAN_CLEAR_DATA {
                eprintln!("note: on macOS its browsing data is kept (clearing is not supported there yet)");
            }
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
                None => match &r.penates_error {
                    Some(e) => println!("  could not check for a new Penates: {e}"),
                    None => println!("  penates is up to date"),
                },
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
                    // The edit must still parse and validate; then save the cleaned values and refresh the launcher.
                    let mut app = store::load(&id).context("the edited config no longer parses; fix it and run `penates edit` again")?;
                    ops::validate(&mut app).context("the edited config is not valid; fix it and run `penates edit` again")?;
                    store::save(&app)?;
                    ops::reinstall(Some(&app), &app);
                    println!("saved '{id}'");
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
