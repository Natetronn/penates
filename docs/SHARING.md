# Sharing and backup

A Penates "app" is a small config, an icon and a launcher; it is not a standalone program. To move apps between machines or people, Penates exports them to a single `.penates` file and imports them back.

## Export
```sh
penates export github                    # -> github.penates
penates export github notion -o pair.penates
penates export --all                     # -> penates-backup.penates
penates export --all --settings          # also look, accent and app catalog address (backups)
```
In the manager: **Export all** and each app's **⋮ → Export app…** save into your export folder, Downloads unless you choose another under **Settings → Backup and export**; **Back up apps and settings** there saves apps and settings. An existing file is never overwritten (the new one gets "(2)" and so on).

## Import
```sh
penates import github.penates
penates import https://example.com/github.penates
penates import backup.penates --on-conflict skip
```
In the manager: **Apps → Import…** (a file or a link), review, then Import.

Before anything is added you see each app and its address, flagged when it already exists, has custom JavaScript, custom CSS or a custom user agent, keeps other sites in its window (listed by name), keeps all links in the app, or asks for camera/microphone or developer tools.

## What is in the file
JSON with `format: "penates-export"` and `version`, then for each app its configuration and its icon (our PNG, base64). Optionally the manager and update settings. **Never** cookies, logins, local storage or cache: those live in the browsing profile, which is not exported.

## Safety
Treat a `.penates` file like a script from its sender.
- **Custom JavaScript** runs inside the site and can read what you type there. It is **removed on import** unless you pass `--allow-js` or tick "Keep custom JavaScript".
- **Camera, microphone and developer tools** are always **switched off** on import. Turn them back on under Edit if you want them.
- **Extra in-app sites** are shown by name before you import: there is no address bar in an app window, so a page from one of them looks like part of the app.
- **Settings** include the app catalog address, which decides which presets your apps get. They are applied only with `--settings` or the matching checkbox.
- Ids, URLs, hosts and sizes from the file are validated (a path-like id becomes a safe one; only http and https addresses are accepted). Files over 20 MB or with more than 500 apps are rejected.
- A file made by a newer Penates is refused with a clear message.

## Not included (yet)
- Logins and browsing data (a possible `--with-data`, with its own warning).
- Standalone per-app installers (`.AppImage`, `.deb`, `.exe`, `.dmg`); those need a build per app, and signing on macOS and Windows.
- A hosted gallery or `penates://` links.
