# CLI reference

`penates <command>`. Run `penates <command> --help` for every flag.

| Command | What it does |
|---|---|
| `create <url>` | New app. Name and icon come from the site unless given. |
| `list [--json]` | List apps. |
| `show <id>` | Print an app's config. |
| `path <id>` | Print the config file path. |
| `edit <id>` | Open the config in `$VISUAL` / `$EDITOR` (or the default app); refreshes the launcher afterward. |
| `run <id>` | Start the app. |
| `gui` | Open the manager window. |
| `install <id>` | (Re)create the application-menu launcher. |
| `icon <id> [file-or-url]` | Set the icon from a file or URL; with no argument, re-fetch the site's own. |
| `icons <url>` | List icons found for a site, best first (PWA manifest, apple-touch, favicon). |
| `clear <id> [--data]` | Clear the cache (keeps logins). `--data` clears cookies and storage too. Refused while the app runs. |
| `check` | Check for a new Penates version and newer app presets. |
| `update <id>` or `update --all` | Apply newer presets and refresh the site icon (never replaces a custom icon). |
| `remove <id> [--purge]` | Remove the app. `--purge` also deletes its cookies and data. |

## `create` options
`-n/--name`, `--id`, `-i/--icon <file-or-url>`, `--user-agent`, `--width`, `--height`, `--zoom`, `--internal-host <host>` (repeatable), `--inject-css <file>`, `--inject-js <file>`, `--devtools`, `--no-install`, `--force`.

## Examples
```sh
penates create https://github.com
penates create notion.so --internal-host accounts.google.com --internal-host '*.notion.com'
penates create example.com --name Demo --icon ~/Pictures/logo.png
penates icons squoosh.app            # what would be found, best first
penates update --all
penates clear github                 # free disk space, stay signed in
penates clear github --data          # sign out and wipe local data
```
