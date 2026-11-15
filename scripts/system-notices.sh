#!/usr/bin/env bash
# Write the license notices for the system libraries an AppImage bundles (GTK, WebKitGTK, GLib and what
# they load): which package each comes from, its exact version and source package, and the package's
# own copyright/license file. Release AppImages are built on Ubuntu (dpkg); local builds on Arch (pacman).
#   scripts/system-notices.sh <penates-desktop binary> <output file>
set -euo pipefail
bin=$1 out=$2

# Every shared library the program and WebKit's helper processes load, plus the plugins GTK and
# gdk-pixbuf pull in (linuxdeploy bundles these).
files() {
  local targets=("$bin")
  for d in /usr/lib/*/webkit2gtk-4.1 /usr/lib/webkit2gtk-4.1 /usr/lib/*/gdk-pixbuf-2.0 /usr/lib/gdk-pixbuf-2.0 \
           /usr/lib/*/gtk-3.0 /usr/lib/gtk-3.0; do
    [[ -d $d ]] && while IFS= read -r f; do targets+=("$f"); done < <(find "$d" -type f \( -name '*.so' -o -perm -u+x \))
  done
  for t in "${targets[@]}"; do
    [[ $t != "$bin" ]] && echo "$t"
    ldd "$t" 2>/dev/null | awk '/=> \// {print $3}'
  done | sort -u | while IFS= read -r f; do readlink -f "$f"; done | sort -u
}

{
  echo "System libraries bundled in the Penates AppImage"
  echo "================================================"
  echo
  if command -v dpkg-query >/dev/null; then
    . /etc/os-release
    echo "These libraries come from ${PRETTY_NAME:-the build system} and are distributed under their own licenses"
    echo "(mostly LGPL; see each package below). Their source code is available from the distribution:"
    echo "  apt-get source <source package>=<version>   or   https://launchpad.net/ubuntu/+source/<source package>"
    echo
    pkgs=$(files | xargs -r dpkg -S 2>/dev/null | cut -d: -f1 | sort -u)
    printf '%-40s %-40s %s\n' "Package" "Version" "Source package"
    for p in $pkgs; do
      printf '%-40s %-40s %s\n' "$p" "$(dpkg-query -W -f='${Version}' "$p")" "$(dpkg-query -W -f='${source:Package} ${source:Version}' "$p")"
    done
    for p in $pkgs; do
      echo; echo "------------------------------------------------------------"; echo "$p"; echo
      cat "/usr/share/doc/$p/copyright" 2>/dev/null || echo "(no copyright file in /usr/share/doc/$p)"
    done
  elif command -v pacman >/dev/null; then
    echo "These libraries come from Arch Linux packages and are distributed under their own licenses (see each"
    echo "package below). Their source code is available from https://gitlab.archlinux.org/archlinux/packaging/packages"
    echo
    pkgs=$(files | xargs -r pacman -Qoq 2>/dev/null | sort -u)
    printf '%-40s %s\n' "Package" "Version"
    for p in $pkgs; do printf '%-40s %s\n' "$p" "$(pacman -Q "$p" | cut -d' ' -f2)"; done
    for p in $pkgs; do
      echo; echo "------------------------------------------------------------"; echo "$p ($(pacman -Qi "$p" | sed -n 's/^Licenses *: //p'))"
      for f in /usr/share/licenses/"$p"/*; do [[ -f $f ]] && { echo; cat "$f"; }; done
    done
  else
    echo "Generated without a package manager; see the build system's packages for these libraries' licenses."
  fi
} > "$out"
echo "$out: $(grep -c '^------------' "$out") packages"
