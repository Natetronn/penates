#!/usr/bin/env bash
# Runs the real binaries on Linux (CI, under a virtual display): create an app, check its .desktop
# launcher, launch the app through the launcher's command, launch the manager, export/remove/import,
# and save screenshots to smoke-out/.
#   xvfb-run -a dbus-run-session -- ./scripts/smoke-linux.sh
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=smoke-out; mkdir -p "$OUT"
# Throwaway config and data folders, so a local run never touches real apps.
TMP=$(mktemp -d); APP=; MGR=
# Stop only what this script started (never other Penates windows on a developer's machine).
trap 'kill $APP $MGR 2>/dev/null || true; rm -rf "$TMP"' EXIT
export XDG_CONFIG_HOME="$TMP/config" XDG_DATA_HOME="$TMP/data"
export PENATES_DESKTOP="$PWD/target/debug/penates-desktop"
A=target/debug/penates
L="$XDG_DATA_HOME/applications/penates-smoke.desktop"

shot() { command -v import >/dev/null && import -window root "$OUT/$1.png" 2>/dev/null || echo "screenshot $1 skipped"; }

$A create https://example.com --name Smoke --id smoke --icon src-tauri/icons/icon.png
test -f "$L"
if command -v desktop-file-validate >/dev/null; then desktop-file-validate "$L"; fi
grep -q '^StartupWMClass=penates-smoke$' "$L"
echo "launcher OK"

# 1) run the launcher's own Exec= line, like a desktop menu would
exec_line=$(sed -n 's/^Exec=//p' "$L")
bash -c "exec $exec_line" >"$OUT/app.log" 2>&1 & APP=$!
sleep 12
if kill -0 "$APP" 2>/dev/null; then echo "app OK"; else echo "app exited:"; cat "$OUT/app.log"; exit 1; fi
shot app

# 2) single-window mode: a second launch must hand over to the first and exit
timeout 20 "$PENATES_DESKTOP" --run smoke
echo "second launch handed over OK"

# 3) the manager, and a second manager start that must exit (one manager at a time)
"$PENATES_DESKTOP" >"$OUT/manager.log" 2>&1 & MGR=$!
sleep 12
kill -0 "$MGR" || { echo "manager exited:"; cat "$OUT/manager.log"; exit 1; }
timeout 20 "$PENATES_DESKTOP"
echo "manager OK"
shot manager
if $A remove smoke --purge 2>/dev/null; then echo "removing a running app's data was not refused"; exit 1; fi
echo "purge of a running app refused OK"
kill "$MGR" "$APP" || true; APP=; MGR=
sleep 2

# 4) sharing round trip
$A export smoke -o "$OUT/smoke.penates"
$A remove smoke --purge
test ! -e "$L" || { echo "launcher not removed"; exit 1; }
$A import "$OUT/smoke.penates" --yes
test -f "$L" || { echo "import did not recreate the launcher"; exit 1; }

# 5) an app with the toolbar: it starts, and the screenshot shows the toolbar above the site
$A create https://example.com --name "Smoke toolbar" --id smoke-toolbar --toolbar --no-install
"$PENATES_DESKTOP" --run smoke-toolbar >"$OUT/toolbar.log" 2>&1 & APP=$!
sleep 12
kill -0 "$APP" || { echo "toolbar app exited:"; cat "$OUT/toolbar.log"; exit 1; }
echo "toolbar app OK"
shot toolbar
kill "$APP" || true; APP=
echo "smoke OK"
