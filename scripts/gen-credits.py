#!/usr/bin/env python3
"""Generate the third-party credits and license texts from Cargo.lock:

  ui/credits.js               the component list on the Credits page (name, version, license, source)
  THIRD-PARTY-LICENSES.txt    every component's copyright notices and license text, shipped with the packages
  ui/licenses.js              the same text, for "Full license texts" in the manager

Components that share a license text are grouped, with each one's copyright lines listed above the text.
Components whose source has no license file get the standard text of a license they declare (MIT where
offered), with their authors as the copyright holders.

    python3 scripts/gen-credits.py          # after changing dependencies (Cargo.lock)
    python3 scripts/gen-credits.py --check  # fail if any output is out of date (CI)
"""
import json, pathlib, re, subprocess, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT_LIST = ROOT / "ui" / "credits.js"
OUT_TEXT = ROOT / "THIRD-PARTY-LICENSES.txt"
OUT_TEXT_JS = ROOT / "ui" / "licenses.js"
LICENSE_FILE = re.compile(r"^(licen[sc]e|copying|notice|copyright|unlicense)", re.I)
COPYRIGHT_LINE = re.compile(r"^\s*(copyright\b|\(c\)|©)", re.I)

MIT = """Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction, including
without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the
following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT
LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO
EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
USE OR OTHER DEALINGS IN THE SOFTWARE."""

BSD3 = """Redistribution and use in source and binary forms, with or without modification, are permitted provided
that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the
   following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the
   following disclaimer in the documentation and/or other materials provided with the distribution.
3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or
   promote products derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED
WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY
DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE
OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
SUCH DAMAGE."""


def packages():
    meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1", "--locked"], cwd=ROOT,
                                     check=True, capture_output=True, text=True).stdout)
    ours = set(meta["workspace_members"])
    seen, out = set(), []
    for p in sorted(meta["packages"], key=lambda p: (p["name"].lower(), p["version"])):
        if p["id"] in ours or (p["name"], p["version"]) in seen:
            continue
        seen.add((p["name"], p["version"]))
        out.append(p)
    return out


def split_copyright(text):
    """(copyright lines, the license text without them)."""
    lines = text.replace("\r\n", "\n").strip("\n").split("\n")
    notices = [l.strip() for l in lines if COPYRIGHT_LINE.match(l)]
    body = "\n".join(l.rstrip() for l in lines if not COPYRIGHT_LINE.match(l)).strip("\n")
    return notices, re.sub(r"\n{3,}", "\n\n", body)


def license_files(p):
    d = pathlib.Path(p["manifest_path"]).parent
    return sorted(f for f in d.iterdir() if f.is_file() and LICENSE_FILE.match(f.name)) if d.exists() else []


def standard_text(p, mpl):
    """For a component whose source has no license file: a standard text it declares, and its authors."""
    expr = p.get("license") or ""
    authors = ", ".join(re.sub(r"\s*<[^>]*>", "", a) for a in p.get("authors") or []) or f"the {p['name']} authors"
    notice = [f"Copyright (c) {authors}"]
    if re.search(r"\bMIT\b", expr):
        return notice, MIT
    if "BSD-3-Clause" in expr:
        return notice, BSD3
    if "MPL-2.0" in expr and mpl:
        return [], mpl
    sys.exit(f"{p['name']} {p['version']}: no license file and no standard text for '{expr}'; add one to gen-credits.py")


def license_text(pkgs):
    groups = {}  # license body -> [(component, copyright lines)]
    mpl = None
    texts = []
    for p in pkgs:
        files = license_files(p)
        texts.append((p, [f.read_text(errors="replace") for f in files]))
        for t in texts[-1][1]:
            if mpl is None and t.lstrip().startswith("Mozilla Public License Version 2.0"):
                mpl = t
    for p, contents in texts:
        name = f"{p['name']} {p['version']}"
        pieces = [split_copyright(t) for t in contents] or [standard_text(p, mpl)]
        for notices, body in pieces:
            if body:
                groups.setdefault(body, []).append((name, notices))
    # Most shared texts first, so the long tail of one-off notices comes last.
    order = sorted(groups.items(), key=lambda g: (-len(g[1]), g[1][0][0]))
    out = ["Third-party software in Penates",
           "==============================",
           "",
           "Penates (MIT License) is built from the open-source components below. For each license text, the "
           "components distributed under it are listed with their copyright notices. Generated from Cargo.lock "
           "by scripts/gen-credits.py.",
           ""]
    for body, members in order:
        out.append("-" * 60)
        for name, notices in members:
            out.append(f"{name}" + (": " + " / ".join(dict.fromkeys(notices)) if notices else ""))
        out += ["", body, ""]
    return "\n".join(out).rstrip() + "\n"


def credits_list(pkgs):
    rows = ",\n".join("  " + json.dumps([p["name"], p["version"], p.get("license") or "see source", p.get("repository") or ""],
                                        ensure_ascii=False) for p in pkgs)
    return ("// Generated by scripts/gen-credits.py from Cargo.lock; do not edit.\n"
            "// [name, version, license, source]\n"
            f"window.PENATES_CREDITS = [\n{rows}\n];\n")


def main():
    pkgs = packages()
    text = license_text(pkgs)
    outputs = {
        OUT_LIST: credits_list(pkgs),
        OUT_TEXT: text,
        OUT_TEXT_JS: "// Generated by scripts/gen-credits.py from Cargo.lock; do not edit.\n"
                     f"window.PENATES_LICENSES = {json.dumps(text, ensure_ascii=False)};\n",
    }
    if "--check" in sys.argv:
        stale = [p.relative_to(ROOT).as_posix() for p, t in outputs.items() if not p.exists() or p.read_text() != t]
        if stale:
            sys.exit(f"out of date: {', '.join(stale)}; run python3 scripts/gen-credits.py and commit")
        print("credits and license texts are current")
        return
    for p, t in outputs.items():
        p.write_text(t)
    print(f"{len(pkgs)} components; THIRD-PARTY-LICENSES.txt {len(text) // 1024} KB")


if __name__ == "__main__":
    main()
