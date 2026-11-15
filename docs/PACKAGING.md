# Packaging

| Format | Command | Output |
|---|---|---|
| `.deb`, `.rpm`, `.AppImage` | `./scripts/build-packages.sh` | `target/release/bundle/{deb,rpm,appimage}/` |
| Arch / CachyOS | `cd packaging/arch && makepkg -si` | `penates-<ver>-1-x86_64.pkg.tar.zst` |
| Portable tarball (any *nix) | `./scripts/make-tarball.sh` | `dist/penates-<ver>-<triple>.tar.gz` (contains `install.sh`; `PREFIX=$HOME/.local ./install.sh`) |
| Windows (NSIS), macOS (`.app`, `.dmg`) | push a `v*` tag; `.github/workflows/release.yml` | workflow artifacts |

Build tools: `cargo install tauri-cli --version "^2" --locked`. The first AppImage build downloads `linuxdeploy`.

## How it fits together
- `penates` (CLI) and `penates-desktop` must sit side by side; the CLI finds the GUI next to itself. The Tauri bundle ships the CLI as a **sidecar** (`bundle.externalBin`), and both `.deb` and `.rpm` install both binaries into `/usr/bin`.
- Packaging-only settings live in `src-tauri/tauri.bundle.conf.json` (full icon list, sidecar, targets) so plain `cargo build` needs no prepared files. `scripts/build-packages.sh` stages the CLI as `src-tauri/binaries/penates-<target-triple>` and passes `--config` to `cargo tauri build`.
- `src-tauri/tauri.linux.conf.json` (merged automatically on Linux) sets `productName: penates` and a desktop template so the launcher is `penates.desktop` with `StartupWMClass=penates`, matching the manager window's app ID.
- Inside an AppImage the running binary is in a temporary mount, so launchers use the `$APPIMAGE` path. The CLI is not part of the AppImage.
- `NO_STRIP=true` is exported for AppImage builds because linuxdeploy trips on newer binutils.

## Checked here (Linux)
`.deb` and `.rpm` contents (both binaries, launcher, icons); the AppImage starts; the tarball installs into a scratch prefix and runs; `makepkg` builds the Arch package. None were installed system-wide.

## Not checked
Windows and macOS installers (the release workflow has never run; CI only builds and tests the code), installing the `.deb`/`.rpm` on their target distros, signing and notarization.
