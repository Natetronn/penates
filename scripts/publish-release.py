#!/usr/bin/env python3
"""Publish the draft release that the Release workflow built.

    scripts/publish-release.py v0.1.0 [--dry-run]
    scripts/publish-release.py v0.2.0-rc.1 --prerelease [--dry-run]

1. Downloads the files the Release workflow attached to the draft release for the tag.
2. Collects installers and their updater signatures under unique names.
3. Writes latest.json in Tauri's updater format (what the app checks and installs from).
4. Uploads latest.json to the draft, sets the notes from CHANGELOG.md and publishes it (skipped with --dry-run).
5. Updates packaging/aur/penates-bin (PKGBUILD + .SRCINFO) for the AUR; pushing to the AUR is manual.

A tag with a suffix (v0.2.0-rc.1) is refused unless --prerelease is given; it is then published as a
pre-release (never "latest", so installed copies do not update to it) and the AUR files are left alone.
"""
import datetime, hashlib, json, pathlib, re, shutil, subprocess, sys

REPO = "Natetronn/penates"
ROOT = pathlib.Path(__file__).resolve().parent.parent


def gh(*args, capture=True):
    return subprocess.run(["gh", *args], check=True, text=True, capture_output=capture).stdout


def main():
    if len(sys.argv) < 2 or not re.fullmatch(r"v\d+\.\d+\.\d+([-+].*)?", sys.argv[1]):
        sys.exit(__doc__)
    tag, dry, pre = sys.argv[1], "--dry-run" in sys.argv, "--prerelease" in sys.argv
    ver = tag[1:]
    if re.search(r"[-+]", ver) and not pre:
        sys.exit(f"{tag} looks like a pre-release; pass --prerelease to publish it as one (it will not reach installed copies)")

    work = ROOT / "dist" / f"release-{tag}"
    shutil.rmtree(work, ignore_errors=True)
    (work / "upload").mkdir(parents=True)
    try:
        gh("release", "download", tag, "-R", REPO, "-D", str(work / "artifacts"), capture=False)
    except subprocess.CalledProcessError:
        sys.exit(f"no draft release {tag} in {REPO} yet (has the Release workflow finished?)")
    files = [p for p in (work / "artifacts").iterdir() if p.is_file()]

    up = work / "upload"
    base = f"https://github.com/{REPO}/releases/download/{tag}/"
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
        """The one file ending in `suffix`. More than one, or one from another version, is an error:
        guessing could publish an old installer."""
        hits = [p for p in files if p.name.endswith(suffix)]
        if len(hits) > 1:
            sys.exit(f"more than one *{suffix} in the draft: {', '.join(h.name for h in hits)}; remove the extra ones")
        if hits and ver not in hits[0].name:
            sys.exit(f"{hits[0].name} is not version {ver}; rebuild the draft")
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
    if tarball and not pre:
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
    # The installers are already on the draft under these names; add the updater file and publish.
    gh("release", "upload", tag, "-R", REPO, "--clobber", str(up / "latest.json"), capture=False)
    gh("release", "edit", tag, "-R", REPO, "--title", f"Penates {ver}", "--notes-file", str(work / "notes.md"),
       "--draft=false", *(["--prerelease", "--latest=false"] if pre else ["--latest"]), capture=False)
    print(f"published https://github.com/{REPO}/releases/tag/{tag}")
    if not pre:
        print("AUR: copy packaging/aur/penates-bin/{PKGBUILD,.SRCINFO} into your aur penates-bin clone, commit and push.")
        print("Then commit the updated packaging/aur/penates-bin files here.")


if __name__ == "__main__":
    main()
