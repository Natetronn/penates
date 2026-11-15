#!/usr/bin/env bash
# Runs the real binaries on macOS (CI): create an app, check its .app launcher, launch it through
# the bundle, launch the manager, export/remove/import, and save screenshots to smoke-out/.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=smoke-out; mkdir -p "$OUT"
export PENATES_DESKTOP="$PWD/target/debug/penates-desktop"
A=target/debug/penates
B="$HOME/Applications/Penates/Smoke.app"

$A create https://example.com --name Smoke --id smoke --icon src-tauri/icons/icon.png
test -x "$B/Contents/MacOS/launcher"
test -f "$B/Contents/MacOS/penates-desktop"
test -f "$B/Contents/Resources/icon.icns"
plutil -lint "$B/Contents/Info.plist"
echo "launcher bundle OK"

open "$B"
sleep 15
pgrep -fl "penates-desktop --run smoke" || { echo "app did not start from its bundle"; exit 1; }
screencapture -x "$OUT/app.png" || echo "screenshot failed"

"$PENATES_DESKTOP" & MGR=$!
sleep 12
kill -0 "$MGR" || { echo "manager exited"; exit 1; }
screencapture -x "$OUT/manager.png" || echo "screenshot failed"
kill "$MGR" || true
pkill -f "penates-desktop --run smoke" || true
sleep 2

$A export smoke -o "$OUT/smoke.penates"
$A remove smoke --purge
test ! -e "$B" || { echo "bundle not removed"; exit 1; }
$A import "$OUT/smoke.penates" --yes
test -d "$B" || { echo "import did not recreate the bundle"; exit 1; }
echo "smoke OK"
