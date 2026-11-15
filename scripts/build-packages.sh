#!/usr/bin/env bash
# Build installable packages (.deb and .rpm) with the Tauri CLI.
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

cargo tauri build --config src-tauri/tauri.bundle.conf.json --bundles "${BUNDLES:-deb,rpm}" "$@"
echo
echo "Packages:"
find target/release/bundle -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) -print
