#!/usr/bin/env bash
# Fetch each catalog app's icon with Penates's own icon finder (the same code the app uses) and save it as
# src/assets/app-icons/<id>.png, so the site serves the logos itself. Run it after the catalog gains apps
# (npm run sync first, for src/data/catalog.json) and commit the icons. It works in a scratch config, never
# your own apps. Apps without an icon get a letter tile on the site.
#   scripts/app-icons.sh [path to the penates command]
set -euo pipefail
site="$(cd "$(dirname "$0")/.." && pwd)"
# The repo's own build first: it may be newer than an installed penates.
bin="${1:-}"
if [[ -z "$bin" ]]; then
  if [[ -x "$site/../target/release/penates" ]]; then bin="$site/../target/release/penates"; else bin="$(command -v penates)"; fi
fi
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export XDG_CONFIG_HOME="$tmp/config" XDG_DATA_HOME="$tmp/data" XDG_CACHE_HOME="$tmp/cache"
out="$site/src/assets/app-icons"
mkdir -p "$out"
node -e 'for (const a of require(process.argv[1])) console.log(a.id, a.url)' "$site/src/data/catalog.json" |
  while read -r id url; do
    if "$bin" create "$url" --id "$id" --no-install >/dev/null 2>"$tmp/err" && [[ -f "$XDG_CONFIG_HOME/penates/icons/$id.png" ]]; then
      cp "$XDG_CONFIG_HOME/penates/icons/$id.png" "$out/$id.png"
      echo "ok      $id"
    else
      echo "no icon $id: $(head -c 160 "$tmp/err" | tr '\n' ' ')"
    fi
  done
