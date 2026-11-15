# Updates

Two kinds of updates, each with its own check under **Settings → Updates** (the Updates tab lists what is available):

1. **Penates itself** comes from `latest.json` attached to the latest GitHub release (Tauri updater format, signed).
   The desktop app offers **Install and restart** where the install type allows it; `penates check`
   reports it. See [RELEASING](RELEASING.md).
2. **App presets** come from a catalog of per-site fixes (extra sign-in hosts, a user agent, CSS) synced
   into installed apps. That catalog is the manifest described below.

## Where the manifest comes from
`~/.config/penates/settings.toml`:
```toml
[updates]
manifest_url = ""      # https:// or file:// URL; empty = the Penates app catalog
check_on_start = true
```
By default Penates reads the shared app catalog from
[Natetronn/penates-apps](https://github.com/Natetronn/penates-apps) (`catalog.json`, built from one file per
app). New or fixed presets reach everyone without a new Penates release; to add or fix an app, open a pull
request there. When the catalog cannot be reached, Penates uses the copy built into each release
(`crates/penates-core/data/presets.json`). Point `manifest_url` at your own file (for example
`file:///path/to/manifest.json`) to use a different catalog.

## Preset manifest format (`schema: 1`)
```json
{
  "schema": 1,
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
An `penates` block from older manifests is ignored.

## How presets apply
- An app links to a catalog entry by `catalog_id`, or automatically by matching its URL's host.
- A preset applies only when its `version` is newer than the app's `catalog_version`.
- Hosts are added (never removed); `user_agent` and `inject_css` are replaced when the preset sets them.
- **Presets never inject JavaScript.** An `inject_js` field in a catalog entry is ignored, so whoever controls the manifest URL cannot run script inside your apps.
- **A custom icon you chose is never replaced.** `update` re-downloads the site's own icon only for apps with `custom_icon = false`.
