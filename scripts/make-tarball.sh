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
cp packaging/penates.desktop src-tauri/icons/icon.png LICENSE README.md "dist/$name/"
cat > "dist/$name/install.sh" <<'INSTALL'
#!/usr/bin/env sh
# Install into $PREFIX (default /usr/local). Use PREFIX=$HOME/.local for a per-user install.
set -e
PREFIX="${PREFIX:-/usr/local}"
here="$(cd "$(dirname "$0")" && pwd)"
install -Dm755 "$here/bin/penates"         "$PREFIX/bin/penates"
install -Dm755 "$here/bin/penates-desktop" "$PREFIX/bin/penates-desktop"
install -Dm644 "$here/penates.desktop"     "$PREFIX/share/applications/penates.desktop"
install -Dm644 "$here/icon.png"           "$PREFIX/share/icons/hicolor/512x512/apps/penates.png"
echo "Installed to $PREFIX. Run: penates --help   or   penates gui"
INSTALL
chmod +x "dist/$name/install.sh"
tar -C dist -czf "dist/$name.tar.gz" "$name"
echo "dist/$name.tar.gz"
