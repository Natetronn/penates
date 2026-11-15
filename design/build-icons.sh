#!/usr/bin/env bash
# Rebuild the app icons in src-tauri/icons/ from the SVG sources. Small sizes use the size-tuned
# drawings (icon-16/32/64.svg: no glow, thicker strokes); 128 px and up use icon.svg.
#   design/build-icons.sh [out-dir]      (default: src-tauri/icons)
# Needs rsvg-convert (librsvg), ImageMagick 7 (magick) and the Tauri CLI (for icon.icns).
set -euo pipefail
for tool in rsvg-convert magick cargo; do
  command -v "$tool" >/dev/null || { echo "missing: $tool" >&2; exit 1; }
done
# An out-dir is relative to where you ran the script; resolve it before moving to the repo root.
if [[ -n "${1:-}" ]]; then mkdir -p "$1"; out="$(cd "$1" && pwd)"; fi
cd "$(dirname "$0")/.."
src=src-tauri/icons
out="${out:-$src}"
mkdir -p "$out"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
r() { rsvg-convert -w "$2" -h "$2" "$src/$1" -o "$3"; }

r icon-32.svg 32  "$out/32x32.png"
r icon-64.svg 64  "$out/64x64.png"
r icon.svg    128 "$out/128x128.png"
r icon.svg    256 "$out/128x128@2x.png"
r icon.svg    512 "$out/icon.png"

# Windows: one .ico holding the sizes Explorer and the taskbar ask for.
r icon-16.svg 16 "$tmp/16.png"; r icon-32.svg 24 "$tmp/24.png"; r icon-32.svg 32 "$tmp/32.png"
r icon-64.svg 48 "$tmp/48.png"; r icon-64.svg 64 "$tmp/64.png"; r icon.svg 256 "$tmp/256.png"
magick "$tmp"/{16,24,32,48,64,256}.png "$out/icon.ico"

# macOS: let the Tauri CLI build the .icns from a 1024 px render, keep only that file.
r icon.svg 1024 "$tmp/1024.png"
if ! log="$(cargo tauri icon "$tmp/1024.png" -o "$tmp/tauri" 2>&1)"; then echo "$log" >&2; exit 1; fi
cp "$tmp/tauri/icon.icns" "$out/icon.icns"
echo "icons written to $out"
