# Packaging

| Format | Command | Output |
|---|---|---|
| `.deb`, `.rpm`, `.AppImage` | `./scripts/build-packages.sh` | `target/release/bundle/{deb,rpm,appimage}/` |
| AUR (`penates-bin`) | installs the release tarball's binaries; files in `packaging/aur/penates-bin` | see [RELEASING](RELEASING.md) |
| AUR (`penates`) | builds the tagged source with `cargo` (x86_64 and aarch64); files in `packaging/aur/penates` | both refreshed by `scripts/publish-release.py` |
| Arch / CachyOS (local) | `cd packaging/arch && makepkg -si` | `penates-<ver>-1-x86_64.pkg.tar.zst` |
| Portable tarball (any *nix) | `./scripts/make-tarball.sh` | `dist/penates-<ver>-<triple>.tar.gz` (contains `install.sh`; `PREFIX=$HOME/.local ./install.sh`) |
| Windows (NSIS), macOS (`.app`, `.dmg`) | push a `v*` tag; `.github/workflows/release.yml` | a draft GitHub release |

Build tools: `cargo install tauri-cli --version "^2" --locked`. The first AppImage build downloads `linuxdeploy`.
Signed builds need the updater key (`~/.tauri/penates.key`, or `TAURI_SIGNING_PRIVATE_KEY`); see [RELEASING](RELEASING.md).

## How it fits together
- `penates` (CLI) and `penates-desktop` must sit side by side; the CLI finds the GUI next to itself. The Tauri bundle ships the CLI as a **sidecar** (`bundle.externalBin`), and both `.deb` and `.rpm` install both binaries into `/usr/bin`.
- Packaging-only settings live in `src-tauri/tauri.bundle.conf.json` (full icon list, sidecar, targets) so plain `cargo build` needs no prepared files. `scripts/build-packages.sh` stages the CLI as `src-tauri/binaries/penates-<target-triple>` and passes `--config` to `cargo tauri build`.
- `src-tauri/tauri.linux.conf.json` (merged automatically on Linux) sets `productName: penates` and a desktop template so the launcher is `penates.desktop` with `StartupWMClass=penates`, matching the manager window's app ID.
- Inside an AppImage the running binary is in a temporary mount, so launchers use the `$APPIMAGE` path. The CLI is inside the AppImage but not on your `PATH`; use the `.deb`, `.rpm`, AUR package or tarball for the command line.
- `NO_STRIP=true` is exported for AppImage builds because linuxdeploy trips on newer binutils.
- Plain `cargo build --release` (tarball, PKGBUILD) needs `--features penates-desktop/custom-protocol` so the manager's page is embedded.
- The release profile optimizes for size (`opt-level = "s"`): about 10 MB for `penates-desktop` and 4 MB for `penates`.

## The tarball's `install.sh`
Installs both binaries, the manager's launcher (pointing at `$PREFIX/bin/penates-desktop` by full path, so
it works when that folder is not on the desktop session's `PATH`) and the icons, then refreshes the desktop's
caches. If the icon theme folder is not writable (it is sometimes left owned by root), the launcher points at
a copy of the icon in `$PREFIX/share/penates/` instead.

## Checked here (Linux)
`.deb` and `.rpm` contents (both binaries, launcher, icons); the AppImage starts; the tarball installs into a
scratch prefix and into `~/.local`, and its launcher passes `desktop-file-validate`; `makepkg` builds the Arch
package.

## Not checked
Windows and macOS installers by hand (CI builds, tests and smoke-tests the apps on every push; the Release
workflow builds and signs them), installing the `.deb`/`.rpm` on their target distros, Apple notarization and
Windows code signing (the installers are not signed with a platform certificate; see the README).
