#!/usr/bin/env bash
# Copy this machine's release files into release-files/ with names that are unique across platforms
# (macOS Apple Silicon and Intel both produce "Penates.app.tar.gz"). Used by the Release workflow.
set -euo pipefail
cd "$(dirname "$0")/.."
rm -rf release-files; mkdir release-files
ver="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
triple="${TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
case "$triple" in
  aarch64-apple-darwin) mac=aarch64 ;;
  x86_64-apple-darwin) mac=x64 ;;
  *) mac="" ;;
esac
while IFS= read -r -d '' f; do
  name="$(basename "$f")"
  if [[ -n "$mac" && "$name" == Penates.app.tar.gz* ]]; then
    name="Penates_${ver}_${mac}${name#Penates}"
  fi
  cp "$f" "release-files/$name"
done < <(find target -path '*/bundle/*' -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \
  -o -name '*.dmg' -o -name '*-setup.exe' -o -name '*.app.tar.gz' -o -name '*.sig' \) -print0)
if [[ -d dist ]]; then find dist -maxdepth 1 -name 'penates-*.tar.gz' -exec cp {} release-files/ \; ; fi
ls -la release-files
