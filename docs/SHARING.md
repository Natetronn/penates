# Sharing and backup

An Penates "app" is a small config, an icon and a launcher; it is not a standalone program. To move apps between machines or people, Penates exports them to a single `.penates` file and imports them back.

## Export
```sh
penates export github                    # -> github.penates
penates export github notion -o pair.penates
penates export --all                     # -> penates-backup.penates
penates export --all --settings          # also theme, accent and update source (backups)
```
In the manager: **Export all** and each app's **⋮ → Export app…** save into your Downloads folder; **Settings → Backup** saves apps and settings.

## Import
```sh
penates import github.penates
penates import https://example.com/github.penates
penates import backup.penates --on-conflict skip
```
In the manager: **Apps → Import…** (a file or a link), review, then Import.

Before anything is added you see each app, its address and flags for existing apps, custom JavaScript, custom CSS and custom user agents.

## What is in the file
JSON with `format: "penates-export"` and `version`, then for each app its configuration and its icon (our PNG, base64). Optionally the manager and update settings. **Never** cookies, logins, local storage or cache: those live in the browsing profile, which is not exported.

## Safety
Treat a `.penates` file like a script from its sender.
- **Custom JavaScript** runs inside the site and can read what you type there. It is **removed on import** unless you pass `--allow-js` or tick "Keep custom JavaScript".
- **Settings** include the update source, which decides which presets and download links you see. They are applied only with `--settings` or the matching checkbox.
- Ids, URLs and sizes from the file are validated (a path-like id becomes a safe one; only http and https addresses are accepted). Files over 20 MB or with more than 500 apps are rejected.
- A file made by a newer Penates is refused with a clear message.

## Not included (yet)
- Logins and browsing data (a possible `--with-data`, with its own warning).
- Standalone per-app installers (`.AppImage`, `.deb`, `.exe`, `.dmg`); those need a build per app, and signing on macOS and Windows.
- A hosted gallery or `penates://` links.
