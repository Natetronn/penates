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

## Packaging
| Format | How | Notes |
|---|---|---|
| `.deb`, `.rpm`, `.AppImage` | `./scripts/build-packages.sh` | needs `cargo install tauri-cli --version "^2" --locked`; output in `target/release/bundle/` |
| Arch / CachyOS | `cd packaging/arch && makepkg -si` | builds from this checkout; swap in a release tarball or git URL before publishing to the AUR |
| Portable tarball (any *nix) | `./scripts/make-tarball.sh` | `PREFIX=$HOME/.local ./install.sh` inside the tarball |
| Windows (NSIS), macOS (`.app`, `.dmg`) | push a `v*` tag; `.github/workflows/release.yml` builds them | cannot be built on Linux; untested so far |

`.deb` and `.rpm` install `penates` (CLI) and `penates-desktop` side by side in `/usr/bin`; the CLI finds the GUI next to itself.
Packaging-only settings (sidecar CLI, full icon set, targets) live in `src-tauri/tauri.bundle.conf.json`.
The updater manifest's `signature` fields are empty because the official Tauri updater (which requires signed releases) is not wired in yet.
