#!/usr/bin/env bash
# Push the AUR packages from packaging/aur/<name> (written by scripts/publish-release.py) to the AUR. Each
# package is cloned into a temporary folder, updated, committed and pushed, and the folder is deleted, so
# no AUR clones are kept around. Needs your AUR SSH key (it asks for its passphrase).
#   scripts/push-aur.sh                      # both: penates-bin and penates
#   scripts/push-aur.sh penates-bin          # one
set -euo pipefail
cd "$(dirname "$0")/.."
root="$(pwd)"
# AUR_REMOTE is the SSH address prefix; the default needs aur.archlinux.org to use your AUR key (in
# ~/.ssh/config: "Host aur aur.archlinux.org" with your IdentityFile).
remote="${AUR_REMOTE:-ssh://aur@aur.archlinux.org}"
names=("$@")
[[ ${#names[@]} -gt 0 ]] || names=(penates-bin penates)
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
for name in "${names[@]}"; do
  src="$root/packaging/aur/$name"
  [[ -f "$src/PKGBUILD" && -f "$src/.SRCINFO" ]] || { echo "no PKGBUILD and .SRCINFO in $src" >&2; exit 1; }
  (cd "$src" && makepkg --printsrcinfo | cmp -s - .SRCINFO) ||
    { echo "$name: .SRCINFO does not match PKGBUILD (run makepkg --printsrcinfo > .SRCINFO)" >&2; exit 1; }
  git clone -q "$remote/$name.git" "$work/$name"
  cp "$src/PKGBUILD" "$src/.SRCINFO" "$work/$name/"
  cd "$work/$name"
  git add PKGBUILD .SRCINFO
  if git diff --cached --quiet; then
    echo "$name: the AUR already has these files"
  else
    ver="$(sed -n 's/^\s*pkgver = //p' .SRCINFO)-$(sed -n 's/^\s*pkgrel = //p' .SRCINFO)"
    git commit -q -m "$name $ver"
    git push -q origin HEAD:master
    echo "$name: pushed $ver"
  fi
  cd "$root"
done
