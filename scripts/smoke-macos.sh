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

# 1) the runner binary directly
"$PENATES_DESKTOP" --run smoke >"$OUT/direct.log" 2>&1 & DIRECT=$!
sleep 12
if kill -0 "$DIRECT" 2>/dev/null; then echo "direct run OK"; kill "$DIRECT"; sleep 2
else echo "direct run exited:"; cat "$OUT/direct.log"; exit 1; fi

# 2) through the .app bundle, like Finder/Launchpad
open --stdout "$OUT/bundle.out" --stderr "$OUT/bundle.err" "$B"
sleep 15
if ! pgrep -fl "penates-desktop --run smoke"; then
  echo "app did not start from its bundle"; echo "--- stdout"; cat "$OUT/bundle.out" || true
  echo "--- stderr"; cat "$OUT/bundle.err" || true
  echo "--- launcher"; cat "$B/Contents/MacOS/launcher"; ls -la "$B/Contents/MacOS"
  echo "--- crash reports"; ls -t ~/Library/Logs/DiagnosticReports 2>/dev/null | head -3
  exit 1
fi
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

# an app with the toolbar: it starts, and the screenshot shows the toolbar above the site
$A create https://example.com --name "Smoke toolbar" --id smoke-toolbar --toolbar --no-install
"$PENATES_DESKTOP" --run smoke-toolbar >"$OUT/toolbar.log" 2>&1 & TB=$!
sleep 12
if kill -0 "$TB" 2>/dev/null; then echo "toolbar app OK"; screencapture -x "$OUT/toolbar.png" || echo "screenshot failed"; kill "$TB"
else echo "toolbar app exited:"; cat "$OUT/toolbar.log"; exit 1; fi
echo "smoke OK"
