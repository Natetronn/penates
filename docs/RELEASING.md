# Releasing

Releases live on `Natetronn/penates`: installers, the in-app update file (`latest.json`) and the AUR
tarball are attached to each GitHub release. Builds sit in a draft (visible only to the repo's owners)
until you publish them.

## Steps
1. Bump `version` in the root `Cargo.toml` (workspace) and move the `[Unreleased]` notes in `CHANGELOG.md`
   under the new version.
2. Commit, then tag and push: `git tag v0.2.0 && git push origin v0.2.0`.
3. The **Release** workflow builds and signs installers on Linux, Windows and macOS (Apple Silicon and
   Intel) and attaches them to a **draft** release. Nothing is public yet.
4. When it is green: `scripts/publish-release.py v0.2.0` (try `--dry-run` first). It downloads the draft's files,
   writes `latest.json`, uploads it to the draft, sets the notes from `CHANGELOG.md`, publishes the release, and updates
   `packaging/aur/penates-bin/{PKGBUILD,.SRCINFO}`.
5. AUR: copy those two files into your `ssh://aur@aur.archlinux.org/penates-bin.git` clone, commit, push.

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
