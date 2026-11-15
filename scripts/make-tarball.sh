#!/usr/bin/env bash
# Portable tarball for any Linux/*nix: both binaries, launcher, icon, license and an installer.
#   ./scripts/make-tarball.sh            -> dist/penates-<version>-<triple>.tar.gz
#   tar xf ... && cd penates-... && PREFIX=$HOME/.local ./install.sh
set -euo pipefail
cd "$(dirname "$0")/.."
ver="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
triple="$(rustc -vV | sed -n 's/^host: //p')"
name="penates-${ver}-${triple}"
cargo build --release --workspace --locked --features penates-desktop/custom-protocol
rm -rf "dist/$name"; mkdir -p "dist/$name/bin"
cp target/release/penates target/release/penates-desktop "dist/$name/bin/"
mkdir -p "dist/$name/icons"
cp src-tauri/icons/{32x32,64x64,128x128,128x128@2x,icon}.png src-tauri/icons/icon.svg "dist/$name/icons/"
cp packaging/penates.desktop LICENSE README.md "dist/$name/"
cat > "dist/$name/install.sh" <<'INSTALL'
#!/usr/bin/env sh
# Install into $PREFIX (default /usr/local). Use PREFIX=$HOME/.local for a per-user install.
set -e
PREFIX="${PREFIX:-/usr/local}"
here="$(cd "$(dirname "$0")" && pwd)"
install -Dm755 "$here/bin/penates"         "$PREFIX/bin/penates"
install -Dm755 "$here/bin/penates-desktop" "$PREFIX/bin/penates-desktop"
install -Dm644 "$here/penates.desktop"     "$PREFIX/share/applications/penates.desktop"
  for s in 32 64 128; do install -Dm644 "$here/icons/${s}x${s}.png" "$PREFIX/share/icons/hicolor/${s}x${s}/apps/penates.png"; done
  install -Dm644 "$here/icons/128x128@2x.png" "$PREFIX/share/icons/hicolor/256x256/apps/penates.png"
  install -Dm644 "$here/icons/icon.png"       "$PREFIX/share/icons/hicolor/512x512/apps/penates.png"
  install -Dm644 "$here/icons/icon.svg"       "$PREFIX/share/icons/hicolor/scalable/apps/penates.svg"
echo "Installed to $PREFIX. Run: penates --help   or   penates gui"
INSTALL
chmod +x "dist/$name/install.sh"
tar -C dist -czf "dist/$name.tar.gz" "$name"
echo "dist/$name.tar.gz"
