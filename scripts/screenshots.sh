#!/usr/bin/env bash
# Regenerate the manager screenshots in docs/images/ (README, website): the real manager, a release build,
# with five sample apps, on a virtual display in an Arch Linux container, so they never come from or
# touch a real desktop. Rendered at 2x for sharp screens. Needs docker and network (site icons, catalog).
#   scripts/screenshots.sh
set -euo pipefail
cd "$(dirname "$0")/.."
ver="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
# Built as the release it is for, so the version badge reads plain "v<version>".
GITHUB_REF_TYPE=tag GITHUB_REF_NAME="v$ver" cargo build --release --workspace --locked --features penates-desktop/custom-protocol
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp target/release/penates target/release/penates-desktop "$work/"
cat > "$work/inside.sh" <<'INSIDE'
set -euo pipefail
pacman -Syu --noconfirm --needed webkit2gtk-4.1 libayatana-appindicator xorg-server-xvfb imagemagick dbus noto-fonts >/dev/null
export DISPLAY=:99 GDK_SCALE=2 XDG_CONFIG_HOME=/tmp/c XDG_DATA_HOME=/tmp/d XDG_CACHE_HOME=/tmp/k XDG_RUNTIME_DIR=/tmp/r
mkdir -p /tmp/r /tmp/c/penates && chmod 700 /tmp/r
Xvfb :99 -screen 0 1720x1440x24 >/dev/null 2>&1 &
sleep 1
while IFS='|' read -r name url; do
  /w/penates create "$url" --name "$name" --no-install >/dev/null
done <<'APPS'
Discord|https://discord.com/app
GitHub|https://github.com/
Notion|https://www.notion.so/
Proton Mail|https://mail.proton.me/
YouTube Music|https://music.youtube.com/
APPS
# One D-Bus session for all shots; each shot starts the manager and stops that same process.
cat > /tmp/shots.sh <<'SHOTS'
set -euo pipefail
shot() { # theme, --tab value, file name
  printf '[updates]\ncheck_on_start = true\n\n[appearance]\ntheme = "%s"\nskin = "default"\naccent = "violet"\n' "$1" > /tmp/c/penates/settings.toml
  /w/penates-desktop --tab "$2" >/dev/null 2>&1 & local pid=$!
  sleep 6
  import -window root "/out/$3.png"
  kill "$pid"; wait "$pid" 2>/dev/null || true; sleep 1
}
shot dark apps manager-apps
shot dark catalog manager-catalog
shot light apps manager-apps-light
shot dark edit:github manager-edit
shot dark settings manager-settings
shot dark about manager-about
shot dark report manager-report
SHOTS
dbus-run-session -- bash /tmp/shots.sh
chown "$OWNER" /out/manager-*.png
INSIDE
docker run --rm -e OWNER="$(id -u):$(id -g)" -v "$work:/w" -v "$PWD/docs/images:/out" archlinux:latest bash /w/inside.sh
file docs/images/manager-*.png
