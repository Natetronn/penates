# penates

Turn websites into native Linux desktop apps. Rust + Tauri 2.

- `penates` — CLI (`create`, `list`, `run`, `edit`, `check`, `update`, `install`, `icon`, `remove`, `gui`)
- `penates-desktop` — Tauri app: manager GUI (no args) and app runner (`--run <id>`)
- `crates/penates-core` — shared config/store/icon/.desktop logic

Each app gets an isolated profile (cookies/storage) in `~/.local/share/penates/profiles/<id>`
and a launcher in `~/.local/share/applications`. Config: `~/.config/penates/apps/<id>.toml`.

## Setup (Arch)
    sudo pacman -S --needed rustup webkit2gtk-4.1 libayatana-appindicator xdotool
    rustup default stable

## Dev
    cargo build --workspace
    cargo run -p penates-desktop                 # manager
    ./target/debug/penates create https://github.com
    ./target/debug/penates run github

Open `penates.code-workspace` in VS Code.

## Updates
`penates check` / the Updates tab read one JSON manifest (semver). It carries the latest Penates release
(Tauri-updater-shaped `platforms` map) and a catalog of per-site presets synced into your apps.
Set `[updates] manifest_url` in `~/.config/penates/settings.toml` (https:// or file://). Empty uses the
bundled FAKE sample at `crates/penates-core/data/sample-manifest.json`.
