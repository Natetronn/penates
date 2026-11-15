# Penates

Turns websites into desktop apps (own window, icon, taskbar entry, isolated login). Rust + Tauri 2.
Linux (WebKitGTK) is the main platform; Windows and macOS are built, tested and smoke-tested in CI.

## Layout
- `crates/penates-core`: all non-UI logic. `ops.rs` holds the flows the CLI and the manager share
  (create, edit, update, remove); put new behavior there, not in a front end.
- `crates/penates-cli`: the `penates` command, one `cmd_*` function per subcommand.
- `src-tauri`: `penates-desktop`. `main.rs` dispatch, `commands.rs` manager IPC, `runner.rs` the per-app window.
- `ui/`: the manager front end, plain HTML/JS/CSS, no build step (`app.js`, `boot.js`, `tokens.css`, `skins.css`).
- `design/`: logo sources, brand sheet, `preview/` (a browser copy of the manager with a mock backend).
- `site/`: the website and docs (Astro + Starlight, served by Cloudflare Workers at getpenates.com). `site/scripts/sync.mjs`
  copies in `docs/*.md`, the screenshots, logo, preview and catalog; edit those at their source.
- Presets (per-site fixes) live in a separate repo, `Natetronn/penates-apps`; `crates/penates-core/data/presets.json`
  is the built-in copy of its `catalog.json`.
- Docs: `docs/*.md` (architecture, CLI, configuration, updates, sharing, packaging, releasing, status).

## Commands
```sh
cargo build --workspace
cargo test --workspace                                   # includes a Node check of the JS link rule
cargo clippy --workspace --all-targets -- -D warnings    # CI runs this on Linux, Windows and macOS
python3 design/preview/build.py                          # after any ui/ change; keep design/preview/mock.js in step with commands.rs
python3 scripts/gen-credits.py                           # after dependency changes: credits and license texts (CI checks them)
(cd site && npm ci && npm run build)                     # the website; npm run dev serves it at localhost:4321/
xvfb-run -a dbus-run-session -- ./scripts/smoke-linux.sh # CI smoke test (opens real windows without Xvfb)
cargo build --release --workspace --features penates-desktop/custom-protocol   # embeds the UI
```

## Rules
- **Never touch the developer's real Penates apps or menu when testing.** Run the binaries with both
  `XDG_CONFIG_HOME` and `XDG_DATA_HOME` pointing at scratch folders (data too, or launchers land in the real
  application menu). Stop only processes you started (by PID): never `pkill -x penates-desktop`, and never
  `pkill -f <pattern>` (it can match its own shell). No synthetic mouse or keyboard input.
- Script-made clicks cannot open new windows in WebKit, so "link to another site opens the browser" and
  "same-site popup opens an app window" can only be verified with real clicks; say so instead of claiming it.
- **Releases follow `docs/RELEASING.md`.** Run `scripts/publish-release.py <tag> --dry-run` freely; publishing,
  tagging, deleting releases and AUR pushes only when the maintainer says so.
- Keep the Rust link rule (`WebApp::is_internal`) and the JavaScript one (`matcher_script` in `runner.rs`) in step;
  the `js_and_rust_link_rules_agree` test checks them.
- App ids are lowercase `a-z 0-9 - _` (max 64), checked by `store::valid_id`; never build a path from an unchecked id.
- Presets must never apply `inject_js`. Imports drop custom JavaScript unless allowed and always switch off
  camera/microphone and devtools.
- The manager page has a content security policy: no inline scripts or `innerHTML`; build DOM with `el()`.
- Docs say what was actually verified; update `docs/STATUS.md` and `CHANGELOG.md` with behavior changes.
