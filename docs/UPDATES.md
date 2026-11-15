# Updates

`penates check` and the Updates tab read one JSON **manifest** that describes two things:

1. **Penates itself**: the latest release, compared with the running version using [semver](https://semver.org/).
2. **App presets**: a catalog of per-site fixes (extra sign-in hosts, a user agent, CSS) synced into installed apps.

The check only reports and opens the download link. It does not install Penates.

## Where the manifest comes from
`~/.config/penates/settings.toml`:
```toml
[updates]
manifest_url = ""      # https:// or file:// URL; empty = the bundled FAKE sample manifest
check_on_start = true
```
The sample lives at `crates/penates-core/data/sample-manifest.json`. Point `manifest_url` at your own file (for example `file:///path/to/manifest.json`) to test.

## Manifest format (`schema: 1`)
```json
{
  "schema": 1,
  "penates": {
    "version": "0.2.0",
    "notes": "What changed",
    "pub_date": "2026-11-20T00:00:00Z",
    "platforms": {
      "linux-x86_64":   { "url": "https://…/penates.AppImage", "signature": "" },
      "darwin-aarch64": { "url": "https://…/penates.dmg",      "signature": "" },
      "windows-x86_64": { "url": "https://…/penates-setup.exe", "signature": "" }
    }
  },
  "catalog": [
    {
      "id": "github",
      "version": "1.1.0",
      "name": "GitHub",
      "url": "https://github.com/",
      "internal_hosts": ["*.githubusercontent.com"],
      "user_agent": null, "inject_css": null, "icon_url": null,
      "notes": "Keep avatars and assets in-app"
    }
  ]
}
```
The `penates` block mirrors the shape of Tauri's updater JSON (`version`, `notes`, `pub_date`, `platforms` keyed `OS-ARCH`), so moving to the official updater later is a small change.

## How presets apply
- An app links to a catalog entry by `catalog_id`, or automatically by matching its URL's host.
- A preset applies only when its `version` is newer than the app's `catalog_version`.
- Hosts are added (never removed); `user_agent` and `inject_css` are replaced when the preset sets them.
- **Presets never inject JavaScript.** An `inject_js` field in a catalog entry is ignored, so whoever controls the manifest URL cannot run script inside your apps.
- **A custom icon you chose is never replaced.** `update` re-downloads the site's own icon only for apps with `custom_icon = false`.

## Signing (future)
The official Tauri updater plugin requires signed releases and the docs say signing cannot be disabled. The `signature` fields are empty here because that plugin is not wired in yet. Doing so needs a signing key and signed build artifacts.
