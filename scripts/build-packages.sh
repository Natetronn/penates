#!/usr/bin/env bash
# Build installable packages with the Tauri CLI: .deb, .rpm and .AppImage on Linux.
# Set BUNDLES to override, e.g. BUNDLES=dmg (macOS) or BUNDLES=nsis (Windows).
# Set TARGET to cross-compile (e.g. TARGET=x86_64-apple-darwin on an Apple Silicon Mac).
# The `penates` CLI is bundled as a sidecar so it installs next to `penates-desktop`,
# which is where the CLI looks for the GUI.
# Needs: cargo install tauri-cli --version "^2" --locked
set -euo pipefail
cd "$(dirname "$0")/.."

host="$(rustc -vV | sed -n 's/^host: //p')"
triple="${TARGET:-$host}"
ext=""; [[ "$triple" == *windows* ]] && ext=".exe"
if [[ -n "${TARGET:-}" ]]; then
  rustup target add "$TARGET"
  cargo build --release -p penates-cli --target "$TARGET"
  cli="target/$TARGET/release/penates${ext}"
  target_args=(--target "$TARGET")
else
  cargo build --release -p penates-cli
  cli="target/release/penates${ext}"
  target_args=()
fi
mkdir -p src-tauri/binaries
cp "$cli" "src-tauri/binaries/penates-${triple}${ext}"

# Updater artifacts are signed. CI passes the key as a secret; locally use ~/.tauri/penates.key.
if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" && -f "$HOME/.tauri/penates.key" ]]; then
  export TAURI_SIGNING_PRIVATE_KEY="$(cat "$HOME/.tauri/penates.key")"
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
fi
if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  echo "error: no signing key (set TAURI_SIGNING_PRIVATE_KEY or create ~/.tauri/penates.key)" >&2; exit 1
fi

# The AppImage carries the system libraries it bundles, so it also carries their license notices,
# made from the libraries the program loads (built first, so ldd can look at it).
bundles="${BUNDLES:-deb,rpm,appimage}"
if [[ $bundles == *appimage* ]]; then
  cargo build --release -p penates-desktop --features penates-desktop/custom-protocol ${target_args[@]+"${target_args[@]}"}
  mkdir -p dist
  ./scripts/system-notices.sh "target/${TARGET:+$TARGET/}release/penates-desktop" dist/SYSTEM-LIBRARIES.txt
fi

# linuxdeploy (AppImage) trips over newer binutils without this
export NO_STRIP="${NO_STRIP:-true}"
cargo tauri build --config src-tauri/tauri.bundle.conf.json --bundles "$bundles" ${target_args[@]+"${target_args[@]}"} "$@"
echo
echo "Packages:"
find target -path '*/bundle/*' -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' -o -name '*.dmg' -o -name '*-setup.exe' -o -name '*.sig' -o -name '*.app.tar.gz' \) -print
