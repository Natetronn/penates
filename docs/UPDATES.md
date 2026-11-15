# Updates

Two kinds of updates, kept apart in the manager's **Updates** tab, each with its own check
(**Settings → Updates** only holds the settings: checking at startup, and the app catalog address):

1. **Penates itself** comes from `latest.json` attached to the latest GitHub release (Tauri updater format, signed).
   The desktop app offers **Install and restart** where the install type allows it; `penates check`
   reports it. See [RELEASING](RELEASING.md).
2. **App presets** come from the app catalog: small per-site fixes (extra sign-in hosts, a user agent, CSS)
   synced into installed apps. The catalog's apps can also be browsed and added in one click under
   **Create → Popular apps** (`penates catalog` on the command line).

The two are checked separately, so a catalog that cannot be read never hides whether a new Penates exists.

## Where the catalog comes from
`~/.config/penates/settings.toml`:
```toml
[updates]
catalog_url = ""       # https:// or file:// URL, or a path; empty = the Penates app catalog
check_on_start = true
```
By default Penates reads the shared app catalog from
[Natetronn/penates-apps](https://github.com/Natetronn/penates-apps) (`catalog.json`, built from one file per
app). New or fixed presets reach everyone without a new Penates release. To add or fix an app, use
**⋮ → Share with the app catalog…** in the manager (or `penates submit <id> --notes "…"`): it opens GitHub with
the entry filled in. Or open a pull request there by hand. When the catalog cannot be reached, Penates uses the copy built into each release
(`crates/penates-core/data/presets.json`). Point `catalog_url` at your own file (for example
`file:///path/to/catalog.json`) to use a different catalog. (`manifest_url` is accepted as another name for `catalog_url`.)

## Catalog format (`schema: 1`)
```json
{
  "schema": 1,
  "catalog": [
    {
      "id": "github",
      "version": "1.1.0",
      "name": "GitHub",
      "url": "https://github.com/",
      "internal_hosts": ["*.github.com"],
      "user_agent": null, "inject_css": null, "icon_url": null,
      "notes": "Keep gists and other github.com pages in the window"
    }
  ]
}
```
Unknown fields are ignored.

## How presets apply
- An app links to a catalog entry by `catalog_id`, or automatically when its site matches (`www.` ignored). New apps link at create time.
- A preset applies only when its `version` is newer than the app's `catalog_version`.
- Hosts are added (never removed, and cleaned like hand-typed ones); `user_agent` and `inject_css` are replaced when the preset sets them. A user agent or CSS you gave at create time wins.
- **Presets never inject JavaScript.** An `inject_js` field in a catalog entry is ignored, so whoever controls the catalog address cannot run script inside your apps.
- **A custom icon you chose is never replaced.** `update` re-downloads the site's own icon only for apps with `custom_icon = false`, and rewrites files and the launcher only when something actually changed.
