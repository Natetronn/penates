# Releasing

Releases live on `Natetronn/penates`: installers, the in-app update file (`latest.json`) and the AUR
tarball are attached to each GitHub release. Builds sit in a draft (visible only to the repo's owners)
until you publish them.

## Before the first release (once)
- The repo secret `TAURI_SIGNING_PRIVATE_KEY` holds the updater key (`gh secret list` shows it).
- `gh` is signed in with access to the repo.
- For the AUR: an AUR account with your SSH key, and the package repo created once with
  `git clone ssh://aur@aur.archlinux.org/penates-bin.git` (an empty clone creates it on first push).

## Steps
1. Bump `version` in the root `Cargo.toml` (workspace) and `pkgver` in `packaging/arch/PKGBUILD`, then run
   `cargo check` so `Cargo.lock` picks up the new version (every `--locked` build fails otherwise).
   Move the `[Unreleased]` notes in `CHANGELOG.md` under the new version and add its compare link at the bottom.
   If dependencies changed, regenerate the credits and license texts: `python3 scripts/gen-credits.py` (CI checks them).
   Refresh the built-in preset copy: `curl -fsSL https://raw.githubusercontent.com/Natetronn/penates-apps/main/catalog.json`
   into `crates/penates-core/data/presets.json` (keep its `_comment`), or copy it from a `penates-apps` checkout.
2. Optional rehearsal: **Actions → Release → Run workflow** builds and signs every installer without uploading anything.
3. Commit, then tag and push: `git tag v0.2.0 && git push origin v0.2.0`.
4. The **Release** workflow checks that the tag matches `Cargo.toml`, builds and signs installers on Linux,
   Windows and macOS (Apple Silicon and Intel) and attaches them to a **draft** release. Nothing is public yet.
5. When it is green: `scripts/publish-release.py v0.2.0 --dry-run`, then without `--dry-run`. It downloads the
   draft's files (refusing duplicates or files of another version), writes `latest.json`, uploads it to the draft,
   sets the notes from `CHANGELOG.md`, publishes the release, and updates both AUR packages in `packaging/aur/`
   (`penates-bin` from the release tarball, `penates` from the tag's source).
6. AUR: for each of `penates-bin` and `penates`, copy `packaging/aur/<name>/{PKGBUILD,.SRCINFO}` into your
   clone of that AUR package, commit and push (`git push origin HEAD:master`). Then commit the updated files here too.

**Pre-releases** (`v0.2.0-rc.1`): `scripts/publish-release.py v0.2.0-rc.1 --prerelease`. They are published as
pre-releases, never as "latest", so installed copies do not update to them, and the AUR is left alone.

**A bad draft**: delete the draft release and the tag (`git push --delete origin v0.2.0 && git tag -d v0.2.0`),
fix, and tag again.

## In-app updates
The desktop app uses Tauri's updater. It reads
`https://github.com/Natetronn/penates/releases/latest/download/latest.json`, verifies the
signature of the download with the public key in `src-tauri/tauri.conf.json`, installs, and restarts.

| Install type | Updates |
|---|---|
| AppImage, `.deb`, `.rpm`, Windows installer, macOS `.app` | in the app: **Install and restart** |
| AUR (`penates-bin`), other package managers | `paru -Syu` / `yay`; the app says so |
| Tarball, dev builds | the app links to the releases page |

## Signing key
- Private key: `~/.tauri/penates.key` on the release machine (no password), and the repo secret
  `TAURI_SIGNING_PRIVATE_KEY` used by the Release workflow.
- Public key: `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`.
- **Back up `~/.tauri/penates.key`.** If it is lost, installed copies can never verify another update; a new
  key means users must reinstall by hand. Never commit it.
