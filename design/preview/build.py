#!/usr/bin/env python3
"""Build a self-contained preview of the Penates manager: the real ui/ files, the real preset catalog
and an in-memory stand-in for the Tauri backend (mock.js). Open the result in any browser.

    python3 design/preview/build.py      -> design/preview/penates-manager.html
"""
import base64, json, pathlib, re

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent.parent
UI = ROOT / "ui"

src = (UI / "index.html").read_text()
tokens = (UI / "tokens.css").read_text() + "\n" + (UI / "skins.css").read_text()
style = re.search(r"<style>(.*?)</style>", src, re.S).group(1)
body = re.search(r'<body>(.*?)<script src="app.js"></script>', src, re.S).group(1)
boot = (UI / "boot.js").read_text()
credits = (UI / "credits.js").read_text() + (UI / "licenses.js").read_text()
app = (UI / "app.js").read_text()
catalog = json.loads((ROOT / "crates/penates-core/data/presets.json").read_text())
catalog.pop("_comment", None)
# What the manager passes in before the page loads (see manager_init_script in src-tauri/src/main.rs).
init = 'window.__PENATES_START = location.hash.slice(1);\nwindow.__PENATES_APPEARANCE = {"theme":"system","skin":"default","accent":"violet"};\n'
version = re.search(r'^version = "([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M).group(1)
mock = "const CATALOG = " + json.dumps(catalog) + ";\nconst VERSION = " + json.dumps(version) + ";\n" + (HERE / "mock.js").read_text()


def data_uri(name, mime):
    return f"data:{mime};base64," + base64.b64encode((UI / name).read_bytes()).decode()


body = body.replace('src="logo-small.svg"', f'src="{data_uri("logo-small.svg", "image/svg+xml")}"')
body = body.replace('src="lockup.png"', f'src="{data_uri("lockup.png", "image/png")}"')
frame_css = """
body{padding-block:24px;padding-left:max(16px,env(safe-area-inset-left,0px));padding-right:max(16px,env(safe-area-inset-right,0px))}
.frame{max-width:880px;margin:0 auto;border:1px solid var(--line);border-radius:10px;overflow:hidden;box-shadow:0 12px 40px rgba(0,0,0,.14)}
.pnote{max-width:880px;margin:0 auto 10px;color:var(--muted);font-size:12px}
header{position:relative}
.toast{position:fixed}
main{min-height:520px}
"""
out = (
    '<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n'
    '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
    '<meta name="claude:viewport-fit-sides" content="cover">\n<title>Penates Manager</title>\n'
    "<style>\n/* tokens.css + skins.css */\n" + tokens + "\nbody{background:var(--bg)}\n" + style + frame_css + "</style>\n"
    "<script>\n" + init + boot + "</script>\n</head>\n<body>\n"
    '<p class="pnote">Preview of the Penates manager with sample data and the real app catalog. '
    "Every tab and button works here; nothing is saved or launched.</p>\n"
    '<div class="frame">' + body + "</div>\n<script>\n" + credits + "\n" + mock + "\n" + app + "\n</script>\n</body>\n</html>\n"
)
(HERE / "penates-manager.html").write_text(out)
print(f"wrote {HERE / 'penates-manager.html'} ({len(out) // 1024} KB, {len(catalog['catalog'])} catalog apps)")
