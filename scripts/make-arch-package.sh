#!/usr/bin/env bash
# Build the Arch Linux package, penates-bin-<version>-1-x86_64.pkg.tar.zst, from the portable tarball: the
# AUR package's PKGBUILD (packaging/aur/penates-bin), pointed at the local tarball. It installs with
# `sudo pacman -U <file>`, and an AUR helper can update it later since it is the same package.
# Needs makepkg (Arch) and must not run as root. Used by the Release workflow.
#   scripts/make-arch-package.sh [tarball] [output folder]
set -euo pipefail
cd "$(dirname "$0")/.."
ver="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
tarball="$(realpath "${1:-dist/penates-$ver-x86_64-unknown-linux-gnu.tar.gz}")"
out="$(realpath -m "${2:-dist}")"
[[ "$(basename "$tarball")" == "penates-$ver-x86_64-unknown-linux-gnu.tar.gz" ]] ||
  { echo "expected the tarball for version $ver, got $(basename "$tarball")" >&2; exit 1; }
mkdir -p "$out"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp "$tarball" "$work/"
sum="$(sha256sum "$tarball" | cut -d' ' -f1)"
sed -e "s/^pkgver=.*/pkgver=$ver/" -e "s/^pkgrel=.*/pkgrel=1/" \
    -e "s|^source=.*|source=(\"$(basename "$tarball")\")|" -e "s/^sha256sums=.*/sha256sums=('$sum')/" \
    packaging/aur/penates-bin/PKGBUILD > "$work/PKGBUILD"
# --nodeps: the package only copies prebuilt files, so building it does not need the runtime libraries.
(cd "$work" && PKGDEST="$out" PACKAGER="${PACKAGER:-Natetronn}" makepkg --nodeps --force --noconfirm)
ls "$out/penates-bin-$ver-1-x86_64.pkg.tar.zst"
