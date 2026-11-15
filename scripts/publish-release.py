#!/usr/bin/env python3
"""Publish a finished release build to the public releases repo.

    scripts/publish-release.py v0.1.0 [--dry-run]

1. Downloads the files the Release workflow attached to the private draft release for the tag.
2. Collects installers and their updater signatures under unique names.
3. Writes latest.json in Tauri's updater format (what the app checks and installs from).
4. Creates the GitHub release on the public repo with those files (skipped with --dry-run).
5. Updates packaging/aur/penates-bin (PKGBUILD + .SRCINFO) for the AUR; pushing to the AUR is manual.
"""
import datetime, hashlib, json, pathlib, re, shutil, subprocess, sys

SRC = "Natetronn/penates"
REL = "Natetronn/penates-releases"
ROOT = pathlib.Path(__file__).resolve().parent.parent


def gh(*args, capture=True):
    return subprocess.run(["gh", *args], check=True, text=True, capture_output=capture).stdout


def main():
    if len(sys.argv) < 2 or not re.fullmatch(r"v\d+\.\d+\.\d+([-+].*)?", sys.argv[1]):
        sys.exit(__doc__)
    tag, dry = sys.argv[1], "--dry-run" in sys.argv
    ver = tag[1:]

    work = ROOT / "dist" / f"release-{tag}"
    shutil.rmtree(work, ignore_errors=True)
    (work / "upload").mkdir(parents=True)
    try:
        gh("release", "download", tag, "-R", SRC, "-D", str(work / "artifacts"), capture=False)
    except subprocess.CalledProcessError:
        sys.exit(f"no draft release {tag} in {SRC} yet (has the Release workflow finished?)")
    files = [p for p in (work / "artifacts").iterdir() if p.is_file()]

    up = work / "upload"
    base = f"https://github.com/{REL}/releases/download/{tag}/"
    platforms, assets = {}, []

    def add(path, name=None):
        dst = up / (name or path.name)
        shutil.copy2(path, dst)
        assets.append(dst)
        return dst.name

    def updater(keys, path, name=None):
        sig = path.with_name(path.name + ".sig")
        asset = add(path, name)
        if sig.exists():
            add(sig, asset + ".sig")
            for k in keys:
                platforms[k] = {"signature": sig.read_text().strip(), "url": base + asset}
        return asset

    def one(suffix):
        hits = [p for p in files if p.name.endswith(suffix)]
        return hits[0] if hits else None

    if p := one(".AppImage"): updater(["linux-x86_64-appimage", "linux-x86_64"], p)
    if p := one(".deb"): updater(["linux-x86_64-deb"], p)
    if p := one(".rpm"): updater(["linux-x86_64-rpm"], p)
    tarball = one("-x86_64-unknown-linux-gnu.tar.gz")
    if tarball: add(tarball)
    if p := one("-setup.exe"): updater(["windows-x86_64-nsis", "windows-x86_64"], p)
    for arch, label in [("aarch64", "aarch64"), ("x86_64", "x64")]:
        if p := one(f"_{label}.app.tar.gz"): updater([f"darwin-{arch}-app", f"darwin-{arch}"], p)
        if p := one(f"_{label}.dmg"): add(p)
    missing = [k for k in ("linux-x86_64-appimage", "linux-x86_64-deb", "linux-x86_64-rpm", "windows-x86_64-nsis",
                           "darwin-aarch64-app", "darwin-x86_64-app") if k not in platforms]
    if missing:
        print("warning: no signed build for:", ", ".join(missing))

    if not platforms:
        sys.exit("no signed updater files found; was the build signed?")

    cl = (ROOT / "CHANGELOG.md").read_text()
    m = re.search(rf"^## \[{re.escape(ver)}\][^\n]*\n(.*?)(?=^## \[|\Z)", cl, re.S | re.M)
    notes = m.group(1).strip() if m else f"Penates {ver}"
    (work / "notes.md").write_text(notes + "\n")
    latest = {
        "version": ver,
        "notes": notes.split("\n\n")[0],
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }
    (up / "latest.json").write_text(json.dumps(latest, indent=2) + "\n")
    assets.append(up / "latest.json")

    print(f"{len(assets)} files for {tag}:")
    for a in assets:
        print("  ", a.name)
    print("updater targets:", ", ".join(sorted(platforms)))

    # AUR files (pushed by hand: they need your AUR account). A dry run writes them into the work
    # folder instead of the tracked packaging folder.
    if tarball:
        src = ROOT / "packaging" / "aur" / "penates-bin"
        aur = work / "aur" if dry else src
        aur.mkdir(parents=True, exist_ok=True)
        if dry:
            shutil.copy2(src / "PKGBUILD", aur / "PKGBUILD")
        pkg = (aur / "PKGBUILD").read_text()
        pkg = re.sub(r"^pkgver=.*$", f"pkgver={ver}", pkg, flags=re.M)
        pkg = re.sub(r"^pkgrel=.*$", "pkgrel=1", pkg, flags=re.M)
        pkg = re.sub(r"^sha256sums=.*$", f"sha256sums=('{hashlib.sha256(tarball.read_bytes()).hexdigest()}')", pkg, flags=re.M)
        (aur / "PKGBUILD").write_text(pkg)
        srcinfo = subprocess.run(["makepkg", "--printsrcinfo"], cwd=aur, check=True, text=True, capture_output=True).stdout
        (aur / ".SRCINFO").write_text(srcinfo)
        print(f"updated {aur}/PKGBUILD and .SRCINFO")

    if dry:
        print("dry run: nothing published")
        return
    gh("release", "create", tag, "-R", REL, "--title", f"Penates {ver}", "--notes-file", str(work / "notes.md"),
       *[str(a) for a in assets], capture=False)
    print(f"published https://github.com/{REL}/releases/tag/{tag}")
    print("AUR: copy packaging/aur/penates-bin/{PKGBUILD,.SRCINFO} into your aur penates-bin clone, commit and push.")


if __name__ == "__main__":
    main()
