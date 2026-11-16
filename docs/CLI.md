# CLI Reference

The `penates` command makes and manages apps from a terminal: `penates <command>`. Run `penates <command> --help` for every flag.

| Command | What it does |
|---|---|
| `create <url>` | New app. Name and icon come from the site unless given; a preset from the app catalog is linked when one matches. |
| `list [--json]` | List apps. Configs that do not load are reported with the reason. |
| `show <id>` | Print an app's config. |
| `path <id>` | Print the config file path. |
| `edit <id>` | Open the config in `$VISUAL` / `$EDITOR` (which may include arguments, e.g. `code --wait`). When an editor is set, the edit is checked and the launcher refreshed afterwards; without one, the file opens in the default app and you run `penates install <id>` after saving. |
| `run <id>` | Start the app. |
| `gui` | Open the manager window. |
| `install <id>` | (Re)create the application-menu launcher (also for apps made with `--no-install`). |
| `icon <id> [file-or-url]` | Set the icon from a file or URL; with no argument, re-fetch the site's own. |
| `icons <url>` | List icons found for a site, best first (PWA manifest, apple-touch, favicon; monochrome icons left out). |
| `clear <id> [--data]` | Clear the cache (keeps logins). `--data` clears cookies and storage too. Refused while the app runs. |
| `catalog [words…]` | List the apps in the App Catalog, ready-made fixes for known sites, marked when you already have one. Popular picks come first. Words keep only apps whose name, address, category or tags contain them all; `--category <name>` keeps one category (e.g. `--category "developer tools"`). |
| `submit <id> --notes "…"` | Propose one of your apps for the app catalog: prints the entry and opens GitHub with it filled in (`--no-open` prints the address instead). Custom JavaScript is never included. |
| `report [--what "…"] [--open]` | Report a problem: prints the version, system and recent log lines with personal details removed, and the log file's place; `--open` opens a new GitHub issue with them (and `--what`) filled in, which is sent only when you submit it there. `--clear-log` deletes the log instead. |
| `check` | Check for a new Penates version and newer app presets (each reported separately). |
| `update <id>` or `update --all` | Apply newer presets and refresh the site icon (never replaces a custom icon). `--all` updates four apps at a time and exits with an error if any failed. |
| `export <id>... \| --all` | Write a `.penates` file: configs and icons only, never logins. `--settings` adds look, accent and the app catalogs (for backups); `-o <file>` (`-` for stdout). |
| `import <file-or-url>` | Review a `.penates` file, then recreate its apps. `--on-conflict skip\|rename\|overwrite` (default rename), `--allow-js` keeps custom JavaScript (dropped by default), `--settings` applies its settings, `-y` skips the prompt. Camera/microphone access and developer tools are always switched off on import. |
| `remove <id> [--purge]` | Remove the app (also one whose config does not load). `--purge` also deletes its cookies and data, and is refused while the app runs. |

## `create` Options
`-n/--name`, `--id` (lowercase letters, digits, `-`, `_`), `-i/--icon <file-or-url>`, `--user-agent`, `--width`, `--height`, `--zoom`, `--internal-host <host>` (repeatable), `--inject-css <file>`, `--inject-js <file>`, `--devtools`, `--no-toolbar` (no toolbar above the site), `--no-install` (no menu launcher; the app also won't add one when run), `--force` (replace an existing app with the same id).

Without `--id`, the id comes from the name (or, for names in other alphabets, from the address) and gets `-2`, `-3`... if it is taken.

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
