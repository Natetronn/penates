use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use penates_core::{export, icon, integration, normalize_url, ops, store, updates};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "penates", version = updates::BUILD_VERSION, about = "Turn websites into desktop apps")]
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
        /// Custom id (default: from the name)
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
        /// Show a toolbar above the site: back, forward, reload, start page, the address, open in browser
        #[arg(long)]
        toolbar: bool,
        /// Don't add a launcher to the application menu (`penates install` adds one later)
        #[arg(long)]
        no_install: bool,
        /// Replace an existing app with the same id
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
        #[arg(required_unless_present = "all", conflicts_with = "all")]
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
        /// When an app with the same id already exists
        #[arg(long, value_enum, default_value_t = OnConflict::Rename)]
        on_conflict: OnConflict,
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
    /// List the apps in the app catalog (ready-made fixes for known sites)
    Catalog,
    /// Propose one of your apps for the app catalog (opens GitHub with the entry filled in)
    Submit {
        id: String,
        /// One line: what the preset fixes (e.g. "Keep Google sign-in inside the window")
        #[arg(long)]
        notes: String,
        /// Print the entry and the address instead of opening the browser
        #[arg(long)]
        no_open: bool,
    },
    /// Check for Penates and app-preset updates
    Check,
    /// Update one app (preset sync + icon refresh), or all with --all
    Update {
        #[arg(required_unless_present = "all", conflicts_with = "all")]
        id: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Open an app's config file in $VISUAL/$EDITOR (or the default app)
    Edit { id: String },
    /// Open the desktop manager
    Gui,
}

#[derive(Clone, Copy, ValueEnum)]
enum OnConflict {
    /// Skip apps that already exist
    Skip,
    /// Keep both (the new one gets a new id)
    Rename,
    /// Replace the existing app
    Overwrite,
}

impl From<OnConflict> for export::Conflict {
    fn from(c: OnConflict) -> Self {
        match c {
            OnConflict::Skip => export::Conflict::Skip,
            OnConflict::Rename => export::Conflict::Rename,
            OnConflict::Overwrite => export::Conflict::Overwrite,
        }
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
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

fn warn(w: impl std::fmt::Display) {
    eprintln!("warning: {w}");
}

fn spawn_desktop(args: &[&str]) -> Result<()> {
    Command::new(desktop_bin()?).args(args).stdin(Stdio::null()).spawn().context("failed to launch penates-desktop")?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_create(
    url: String, name: Option<String>, id: Option<String>, icon: Option<String>, user_agent: Option<String>,
    width: f64, height: f64, zoom: f64, internal_hosts: Vec<String>, inject_css: Option<PathBuf>,
    inject_js: Option<PathBuf>, devtools: bool, toolbar: bool, no_install: bool, force: bool,
) -> Result<()> {
    let c = ops::create(ops::NewApp {
        url,
        name,
        id,
        icon: icon.map(ops::IconChoice::Source).unwrap_or_default(),
        user_agent,
        width,
        height,
        zoom,
        internal_hosts,
        inject_css: read_opt(inject_css)?,
        inject_js: read_opt(inject_js)?,
        devtools,
        toolbar,
        launcher: !no_install,
        replace: force,
    })?;
    println!("created '{}' ({})", c.app.name, c.app.id);
    if let Some(v) = &c.preset {
        println!("preset: {} {v} from the app catalog", c.app.catalog_id.as_deref().unwrap_or(&c.app.id));
    }
    if let Some(p) = &c.launcher {
        println!("launcher: {}", p.display());
    }
    c.warnings.iter().for_each(warn);
    Ok(())
}

fn cmd_list(json: bool) -> Result<()> {
    let (apps, broken) = store::scan();
    for b in &broken {
        warn(format!("'{}' has a broken config and is skipped: {} (fix it with `penates edit {0}` or remove it)", b.id, b.error));
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&apps)?);
    } else if apps.is_empty() {
        println!("no apps yet; try `penates create https://example.com`");
    } else {
        for a in apps {
            println!("{:<20} {:<24} {}", a.id, a.name, a.url);
        }
    }
    Ok(())
}

fn cmd_icon(id: &str, source: Option<String>) -> Result<()> {
    let app = store::load(id)?;
    let (p, custom) = match source {
        Some(f) => (icon::import_source(&f, id)?, true),
        None => (icon::fetch_for(&app.url, id)?, false),
    };
    let saved = ops::set_icon_file(id, p.clone(), custom)?;
    println!("icon: {}", p.display());
    saved.warning.into_iter().for_each(warn);
    Ok(())
}

fn cmd_clear(id: &str, data: bool) -> Result<()> {
    ops::ensure_closed(id)?;
    if data {
        store::clear_profile(id)?;
        println!("cleared cookies and data for '{id}'");
    } else {
        println!("cleared {} of cache for '{id}'", fmt_bytes(store::clear_cache(id)));
    }
    Ok(())
}

fn cmd_export(ids: Vec<String>, settings: bool, output: Option<String>) -> Result<()> {
    let f = export::build(&ids, settings)?;
    let text = serde_json::to_string_pretty(&f)?;
    let out = output.unwrap_or_else(|| match f.apps.as_slice() {
        [one] => format!("{}.penates", one.config.id),
        _ => "penates-backup.penates".into(),
    });
    if out == "-" {
        println!("{text}");
    } else {
        std::fs::write(&out, text)?;
        println!("exported {} app{} to {out}", f.apps.len(), plural(f.apps.len()));
        println!("(contains configs and icons only; no cookies or logins)");
    }
    Ok(())
}

/// What a person should know about one app in an import before saying yes.
fn import_flags(s: &export::Summary) -> Vec<String> {
    let mut flags = vec![];
    if s.exists {
        flags.push("already exists".into());
    }
    if s.custom_js {
        flags.push("CUSTOM JAVASCRIPT".into());
    }
    if s.custom_css {
        flags.push("custom css".into());
    }
    if s.user_agent {
        flags.push("custom user agent".into());
    }
    if s.all_links_in_app {
        flags.push("keeps ALL links in the app".into());
    } else if !s.extra_hosts.is_empty() {
        flags.push(format!("keeps these sites in the app: {}", s.extra_hosts.join(", ")));
    }
    if s.camera_microphone || s.devtools {
        flags.push("asks for camera/microphone or developer tools (switched off on import)".into());
    }
    flags
}

fn confirm(question: &str) -> Result<()> {
    use std::io::{IsTerminal, Write};
    if !std::io::stdin().is_terminal() {
        bail!("not a terminal; pass --yes to import without asking");
    }
    print!("{question} [y/N] ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    if !matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        bail!("cancelled");
    }
    Ok(())
}

fn cmd_import(source: &str, on_conflict: OnConflict, allow_js: bool, settings: bool, yes: bool) -> Result<()> {
    let f = export::parse(&export::read_source(source)?)?;
    println!("{} app{} in {source}:", f.apps.len(), plural(f.apps.len()));
    let summary = export::inspect(&f);
    for s in &summary {
        let flags = import_flags(s);
        println!("  {:<18} {}  {}", s.id, s.url, if flags.is_empty() { String::new() } else { format!("[{}]", flags.join("; ")) });
    }
    if f.settings.is_some() {
        println!("  (file also has settings{})", if settings { ", which WILL be applied, including the update source" } else { "; ignored unless --settings" });
    }
    if summary.iter().any(|s| s.custom_js) && !allow_js {
        println!("custom JavaScript will be removed (it runs inside the site). Use --allow-js to keep it, only if you trust the source.");
    }
    if !yes {
        confirm("import?")?;
    }
    let r = export::apply(&f, &export::ImportOptions { on_conflict: on_conflict.into(), allow_js, settings });
    r.imported.iter().for_each(|id| println!("imported {id}"));
    r.renamed.iter().for_each(|(a, b)| println!("  ({a} already existed; saved as {b})"));
    r.skipped.iter().for_each(|id| println!("skipped {id} (already exists)"));
    r.js_removed.iter().for_each(|id| println!("removed custom JavaScript from {id}"));
    r.permissions_reset.iter().for_each(|id| println!("switched off camera/microphone and developer tools for {id}"));
    if r.settings_applied {
        println!("applied settings");
    }
    r.errors.iter().for_each(|e| eprintln!("error: {e}"));
    if !r.errors.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_remove(id: &str, purge: bool) -> Result<()> {
    ops::remove(id, purge)?;
    println!("removed '{id}'{}", if purge && store::CAN_CLEAR_DATA { " and its data" } else { "" });
    if purge && !store::CAN_CLEAR_DATA {
        eprintln!("note: on macOS its browsing data is kept (clearing is not supported there yet)");
    }
    Ok(())
}

fn cmd_check() -> Result<()> {
    println!("penates {}", updates::BUILD_VERSION);
    match updates::check_penates() {
        Ok(Some(u)) => {
            println!("  new version {} available: {}", u.latest, u.notes.lines().next().unwrap_or(""));
            println!("  download: {}", updates::RELEASES_PAGE);
        }
        Ok(None) => println!("  penates is up to date"),
        Err(e) => println!("  could not check for a new Penates: {e:#}"),
    }
    let r = updates::check_apps().context("could not check your apps")?;
    println!("apps (catalog: {})", r.source);
    for a in &r.apps {
        println!("  {:<18} preset {} -> {}  {}", a.id, a.from.as_deref().unwrap_or("none"), a.to, a.notes);
    }
    if r.app_count == 0 {
        println!("  no apps yet; `penates catalog` lists {} ready-made ones", r.catalog_size);
    } else if r.apps.is_empty() {
        println!("  all apps are up to date");
    } else {
        println!("run `penates update --all` to apply");
    }
    Ok(())
}

fn cmd_catalog() -> Result<()> {
    let l = updates::catalog_listing()?;
    println!("{} ({} apps):", l.source, l.apps.len());
    for a in &l.apps {
        let mark = a.added.as_ref().map(|id| format!("  [added as {id}]")).unwrap_or_default();
        println!("  {:<16} {:<34} {}{mark}", a.name, a.url, a.notes);
    }
    println!("add one with `penates create <url>`; its fixes apply automatically");
    Ok(())
}

fn cmd_submit(id: &str, notes: &str, no_open: bool) -> Result<()> {
    let s = updates::submission(&store::load(id)?, notes)?;
    println!("{}", s.json.trim_end());
    if s.kind == "change" {
        println!("this site is already in the catalog; the link suggests the change in an issue");
    }
    if no_open {
        println!("{}", s.url);
    } else {
        open::that(&s.url)?;
        println!("opened GitHub in your browser (you need a GitHub account to propose it)");
    }
    Ok(())
}

fn print_outcome(o: &ops::Outcome) {
    println!(
        "{:<18} icon {}{}",
        o.app.id,
        if o.icon_refreshed { "refreshed" } else { "unchanged" },
        o.preset.as_ref().map(|v| format!(", preset {v} applied")).unwrap_or_default()
    );
    o.warning.iter().for_each(warn);
}

fn cmd_update(id: Option<String>) -> Result<()> {
    match id {
        Some(id) => print_outcome(&ops::update(&id)?),
        None => {
            let r = ops::update_all();
            r.updated.iter().for_each(print_outcome);
            for f in &r.failed {
                eprintln!("error: {}: {}", f.id, f.error);
            }
            if !r.failed.is_empty() {
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

/// Run `$VISUAL`/`$EDITOR` (which may carry arguments, e.g. `code --wait`) on `path`.
fn run_editor(editor: &str, path: &std::path::Path) -> Result<()> {
    let mut parts = editor.split_whitespace();
    let program = parts.next().context("$EDITOR is empty")?;
    let status = Command::new(program).args(parts).arg(path).status().with_context(|| format!("running {program}"))?;
    anyhow::ensure!(status.success(), "{program} exited with {status}");
    Ok(())
}

fn cmd_edit(id: &str) -> Result<()> {
    store::check_id(id)?;
    let path = store::path_for(id);
    anyhow::ensure!(path.exists(), "no app with id '{id}'");
    let Ok(editor) = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")) else {
        open::that(&path)?;
        println!("opened {} (save it, then run `penates install {id}` to refresh the launcher)", path.display());
        return Ok(());
    };
    run_editor(&editor, &path)?;
    // The edit must still parse and validate; then save the cleaned values and refresh the launcher.
    let mut app = store::load(id).context("the edited config no longer parses; fix it and run `penates edit` again")?;
    ops::validate(&mut app).context("the edited config is not valid; fix it and run `penates edit` again")?;
    store::save(&app)?;
    if let Err(e) = ops::reinstall(None, &app) {
        warn(format!("launcher not updated: {e:#}"));
    }
    println!("saved '{id}'");
    Ok(())
}

/// Exit quietly when the reader goes away (`penates list | head`), like other command line tools.
/// Rust ignores SIGPIPE by default, which turns a closed pipe into a panic on the next `println!`.
fn quiet_on_closed_pipe() {
    #[cfg(unix)]
    // SAFETY: restoring the default disposition of one signal at startup, before any threads exist.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

fn main() -> Result<()> {
    quiet_on_closed_pipe();
    match Cli::parse().cmd {
        Cmd::Create {
            url, name, id, icon, user_agent, width, height, zoom, internal_hosts, inject_css, inject_js, devtools,
            toolbar, no_install, force,
        } => cmd_create(url, name, id, icon, user_agent, width, height, zoom, internal_hosts, inject_css, inject_js, devtools, toolbar, no_install, force),
        Cmd::List { json } => cmd_list(json),
        Cmd::Show { id } => {
            store::load(&id)?;
            print!("{}", std::fs::read_to_string(store::path_for(&id))?);
            Ok(())
        }
        Cmd::Path { id } => {
            store::check_id(&id)?;
            println!("{}", store::path_for(&id).display());
            Ok(())
        }
        Cmd::Run { id } => {
            store::load(&id)?;
            spawn_desktop(&["--run", &id])
        }
        Cmd::Install { id } => {
            let mut app = store::load(&id)?;
            app.launcher = true;
            store::save(&app)?;
            let p = integration::install(&app, &desktop_bin()?)?;
            println!("launcher: {}", p.display());
            Ok(())
        }
        Cmd::Icon { id, source } => cmd_icon(&id, source),
        Cmd::Icons { url } => {
            let page = penates_core::site::Page::fetch(&normalize_url(&url)?)?;
            for (i, c) in page.icon_candidates().into_iter().enumerate() {
                println!("{:>2}. {c}", i + 1);
            }
            Ok(())
        }
        Cmd::Clear { id, data } => cmd_clear(&id, data),
        Cmd::Export { ids, all: _, settings, output } => cmd_export(ids, settings, output),
        Cmd::Import { source, on_conflict, allow_js, settings, yes } => cmd_import(&source, on_conflict, allow_js, settings, yes),
        Cmd::Remove { id, purge } => cmd_remove(&id, purge),
        Cmd::Catalog => cmd_catalog(),
        Cmd::Submit { id, notes, no_open } => cmd_submit(&id, &notes, no_open),
        Cmd::Check => cmd_check(),
        Cmd::Update { id, all: _ } => cmd_update(id),
        Cmd::Edit { id } => cmd_edit(&id),
        Cmd::Gui => spawn_desktop(&[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn update_takes_an_id_or_all_not_both() {
        assert!(Cli::try_parse_from(["penates", "update", "gh", "--all"]).is_err());
        assert!(Cli::try_parse_from(["penates", "update"]).is_err());
        assert!(Cli::try_parse_from(["penates", "update", "--all"]).is_ok());
        assert!(Cli::try_parse_from(["penates", "import", "x.penates", "--on-conflict", "nope"]).is_err());
    }

    #[test]
    fn editor_with_arguments() {
        let p = std::env::temp_dir();
        assert!(run_editor("true --wait", &p).is_ok());
        assert!(run_editor("false", &p).is_err());
    }
}
