# Updates

Two kinds of updates, kept apart in the manager's **Updates** tab, each with its own check
(**Settings → Updates** only holds the settings: checking at startup, and the app catalogs):

1. **Penates itself** comes from `latest.json` attached to the latest GitHub release (Tauri updater format, signed).
   The desktop app offers **Install and restart** where the install type allows it; `penates check`
   reports it. See [RELEASING](RELEASING.md).
2. **App presets** come from the app catalog: small per-site fixes (extra sign-in hosts, a user agent, CSS)
   synced into installed apps. The catalog's apps can also be browsed and added in one click under
   the **Catalog** tab (`penates catalog` on the command line).

The two are checked separately, so a catalog that cannot be read never hides whether a new Penates exists.

## Where the Catalog Comes From
`~/.config/penates/settings.toml`:
```toml
[updates]
penates_catalog = true   # use the Penates app catalog
catalogs = []            # your own catalogs too: https:// or file:// URLs, or paths
check_on_start = true
```
By default Penates reads the shared app catalog from
[Natetronn/penates-apps](https://github.com/Natetronn/penates-apps) (`catalog.json`, built from one file per
app). New or fixed presets reach everyone without a new Penates release. To add or fix an app, use
**⋮ → Share with the app catalog…** in the manager (or `penates submit <id> --notes "…"`): it opens GitHub with
the entry filled in. Or open a pull request there by hand. When the catalog cannot be reached, Penates uses the copy built into each release
(`crates/penates-core/data/presets.json`).

### Your Own Catalogs
Add catalogs of your own under **Settings → Updates → Your Catalogs** (Add takes an `https://` or `file://`
address or a path, and reads the catalog first, so one that cannot be used is never added; **Choose file…**
picks a `.json` file). They are used together with the Penates app catalog, for sites of your own or presets you
are trying out before sharing them:
- Your catalogs come first: an entry with the same `id` as one in the Penates catalog replaces it, and among
  yours the first one listed wins.
- A catalog of yours that cannot be read is reported (in Settings, and as a warning by `penates check` and
  `penates catalog`) and the others are still used.
- Turn off **Use the Penates app catalog** to use only your own.

A single `catalog_url` (or `manifest_url`) address in `settings.toml` is read as the only entry of `catalogs`,
with `penates_catalog` off.

## Catalog Format (`schema: 1`)
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
      "notes": "Keep gists and other github.com pages in the window",
      "category": "Developer Tools",
      "tags": ["code", "git", "pull requests"],
      "popular": true
    }
  ]
}
```
Unknown fields are ignored.

`category`, `tags` and `popular` place an app in the **Catalog** tab (and the App Catalog on the website): its
category chip, the words search also looks in, and whether it shows under Popular. They are not part of the
preset, so changing them never offers an app an update. Apps without a category are listed under Other. Your own
catalogs can use them too.

## How Presets Apply
- An app links to a catalog entry by `catalog_id`, or automatically when its site matches (`www.` ignored). New apps link at create time.
- A preset applies only when its `version` is newer than the app's `catalog_version`.
- Hosts are added (never removed, and cleaned like hand-typed ones); `user_agent` and `inject_css` are replaced when the preset sets them. A user agent or CSS you gave at create time wins.
- **Presets never inject JavaScript.** An `inject_js` field in a catalog entry is ignored, so whoever controls a catalog cannot run script inside your apps.
- **A custom icon you chose is never replaced.** `update` re-downloads the site's own icon only for apps with `custom_icon = false`, and rewrites files and the launcher only when something actually changed.
