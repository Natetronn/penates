#!/usr/bin/env python3
"""Download counts of every release file, grouped by platform, from GitHub's API (through `gh`).

    python3 scripts/downloads.py          # a table: one column per release, plus repo traffic if visible
    python3 scripts/downloads.py --csv    # one dated row per file (the Stats workflow keeps these on the
                                          # `stats` branch, so the counts have a history)

GitHub keeps only running totals per file. `latest.json` counts update checks (every installed copy checks
when it starts), not people. Downloading a whole release (a check, a publish dry run) adds one to every
file, so prefer fetching only the files a check needs.
"""
import collections, datetime, json, os, subprocess, sys

REPO = os.environ.get("GITHUB_REPOSITORY", "Natetronn/penates")
KINDS = [  # (file name test, platform)
    (lambda n: n == "latest.json", "update checks (latest.json)"),
    (lambda n: n.endswith(".AppImage"), "Linux: AppImage"),
    (lambda n: n.endswith(".deb"), "Linux: .deb"),
    (lambda n: n.endswith(".rpm"), "Linux: .rpm"),
    (lambda n: n.endswith(".pkg.tar.zst"), "Linux: Arch package"),
    (lambda n: "unknown-linux" in n, "Linux: tarball (AUR)"),
    (lambda n: n.endswith("-setup.exe"), "Windows: installer"),
    (lambda n: n.endswith(".dmg"), "macOS: .dmg"),
    (lambda n: n.endswith(".app.tar.gz"), "macOS: in-app update"),
]


def gh(path):
    out = subprocess.run(["gh", "api", "--paginate", "--slurp", path], check=True, capture_output=True, text=True).stdout
    pages = json.loads(out)
    return [x for page in pages for x in page] if pages and isinstance(pages[0], list) else pages[0]


def kind(name):
    return next((k for test, k in KINDS if test(name)), "other: " + name)


def main():
    releases = [r for r in gh(f"repos/{REPO}/releases") if not r.get("draft")]
    files = [(r["tag_name"], a["name"], a["download_count"]) for r in releases for a in r["assets"]
             if not a["name"].endswith(".sig")]
    if "--csv" in sys.argv:
        today = datetime.date.today().isoformat()
        print("date,release,platform,file,downloads")
        for tag, name, n in files:
            print(f'{today},{tag},"{kind(name)}",{name},{n}')
        return
    tags = [r["tag_name"] for r in releases]
    table = collections.defaultdict(collections.Counter)
    for tag, name, n in files:
        table[kind(name)][tag] += n
    width = max(map(len, table), default=10) + 2
    print(f"{'':{width}}" + "".join(f"{t:>10}" for t in tags) + f"{'total':>10}")
    for k in [k for _, k in KINDS if k in table] + sorted(k for k in table if k.startswith("other")):
        print(f"{k:{width}}" + "".join(f"{table[k][t]:>10}" for t in tags) + f"{sum(table[k].values()):>10}")
    try:  # needs push access to the repo
        views, clones = gh(f"repos/{REPO}/traffic/views"), gh(f"repos/{REPO}/traffic/clones")
        print(f"\nlast 14 days: {views['count']} page views ({views['uniques']} visitors), "
              f"{clones['count']} clones ({clones['uniques']} unique)")
    except subprocess.CalledProcessError:
        pass


if __name__ == "__main__":
    main()
