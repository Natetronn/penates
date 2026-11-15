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
cp packaging/penates.desktop LICENSE README.md CHANGELOG.md "dist/$name/"
mkdir -p "dist/$name/docs/images"; cp docs/images/*.png "dist/$name/docs/images/"   # README pictures
cat > "dist/$name/install.sh" <<'INSTALL'
#!/usr/bin/env sh
# Install into $PREFIX (default /usr/local). Use PREFIX=$HOME/.local for a per-user install.
set -e
PREFIX="${PREFIX:-/usr/local}"
here="$(cd "$(dirname "$0")" && pwd)"
install -Dm755 "$here/bin/penates"         "$PREFIX/bin/penates"
install -Dm755 "$here/bin/penates-desktop" "$PREFIX/bin/penates-desktop"

# Icons go into the icon theme; if that folder is not writable (it is sometimes left owned by root),
# the launcher points at a copy in Penates's own folder instead.
hicolor="$PREFIX/share/icons/hicolor"
if mkdir -p "$hicolor/scalable/apps" 2>/dev/null && [ -w "$hicolor/scalable/apps" ]; then
  for s in 32 64 128; do install -Dm644 "$here/icons/${s}x${s}.png" "$hicolor/${s}x${s}/apps/penates.png"; done
  install -Dm644 "$here/icons/128x128@2x.png" "$hicolor/256x256/apps/penates.png"
  install -Dm644 "$here/icons/icon.png"       "$hicolor/512x512/apps/penates.png"
  install -Dm644 "$here/icons/icon.svg"       "$hicolor/scalable/apps/penates.svg"
  icon=penates
else
  install -Dm644 "$here/icons/icon.png" "$PREFIX/share/penates/penates.png"
  icon="$PREFIX/share/penates/penates.png"
  echo "note: $hicolor is not writable; the launcher uses $icon"
fi

# The launcher names the program by full path, so it works even when $PREFIX/bin is not on the
# desktop session's PATH (e.g. PREFIX=$HOME/.local).
mkdir -p "$PREFIX/share/applications"
sed -e "s|^Exec=penates-desktop|Exec=\"$PREFIX/bin/penates-desktop\"|" -e "s|^Icon=.*|Icon=$icon|" \
  "$here/penates.desktop" > "$PREFIX/share/applications/penates.desktop"

# Let the desktop pick up the new launcher and icon now rather than after the next login.
command -v update-desktop-database >/dev/null && update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
[ "$icon" = penates ] && command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$hicolor" 2>/dev/null || true
for k in kbuildsycoca6 kbuildsycoca5; do command -v $k >/dev/null && { $k >/dev/null 2>&1; break; }; done
echo "Installed to $PREFIX. Run: penates --help   or   penates gui"
INSTALL
chmod +x "dist/$name/install.sh"
tar -C dist -czf "dist/$name.tar.gz" "$name"
echo "dist/$name.tar.gz"
