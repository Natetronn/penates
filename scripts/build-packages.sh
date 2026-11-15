#!/usr/bin/env bash
# Build installable packages with the Tauri CLI: .deb, .rpm and .AppImage on Linux.
# Set BUNDLES to override, e.g. BUNDLES=dmg (macOS) or BUNDLES=nsis (Windows).
# The `penates` CLI is bundled as a sidecar so it installs next to `penates-desktop`,
# which is where the CLI looks for the GUI.
# Needs: cargo install tauri-cli --version "^2" --locked
set -euo pipefail
cd "$(dirname "$0")/.."

triple="$(rustc -vV | sed -n 's/^host: //p')"
cargo build --release -p penates-cli
mkdir -p src-tauri/binaries
ext=""; [[ "$triple" == *windows* ]] && ext=".exe"
cp "target/release/penates${ext}" "src-tauri/binaries/penates-${triple}${ext}"

# linuxdeploy (AppImage) trips over newer binutils without this
export NO_STRIP="${NO_STRIP:-true}"
cargo tauri build --config src-tauri/tauri.bundle.conf.json --bundles "${BUNDLES:-deb,rpm,appimage}" "$@"
echo
echo "Packages:"
find target/release/bundle -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) -print
