// The website's third-party licenses, from the packages npm installs for it (production dependencies;
// most are build tools that never reach a visitor, but listing all of them is simpler than guessing):
//   src/data/site-licenses.json        name, version, license and source of each, for the Credits page
//   public/THIRD-PARTY-LICENSES.txt    every license text, each with the packages under it and their
//                                       copyright lines (the same format as the app's file)
// Run by scripts/sync.mjs before each build, so the list always matches what is installed.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const site = new URL('..', import.meta.url).pathname;
const LICENSE_FILE = /^(licen[sc]e|copying|notice|copyright|unlicense)/i;
// A copyright notice, not a disclaimer line that starts with "COPYRIGHT HOLDERS" or "COPYRIGHT OWNER".
const COPYRIGHT = /^\s*(copyright\b(?!\s+(holders?|owners?)\b)|\(c\)|©)/i;
// For packages that ship no license file: the standard text of the license they declare (as the app's
// scripts/gen-credits.py does), with their author as the copyright holder.
const STANDARD = {
  MIT: `Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the Software without restriction, including without limitation the
rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of
the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.`,
  ISC: `Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby
granted, provided that the above copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
THIS SOFTWARE.`,
};
const authorOf = (pkg) => {
  const a = pkg.author ?? pkg.contributors?.[0];
  const name = typeof a === 'string' ? a.replace(/\s*[<(].*$/, '') : a?.name;
  return name?.trim() || `the ${pkg.name} authors`;
};

const paths = execFileSync('npm', ['ls', '--omit=dev', '--all', '--parseable'], { cwd: site, encoding: 'utf8' })
  .split('\n').map((p) => p.trim()).filter((p) => p && p !== site.replace(/\/$/, ''));

const seen = new Map();
for (const dir of paths) {
  const file = join(dir, 'package.json');
  if (!existsSync(file)) continue;
  const pkg = JSON.parse(readFileSync(file, 'utf8'));
  const key = `${pkg.name}@${pkg.version}`;
  if (seen.has(key)) continue;
  const license = typeof pkg.license === 'string' ? pkg.license : pkg.license?.type ?? 'see source';
  const repo = typeof pkg.repository === 'string' ? pkg.repository : pkg.repository?.url ?? '';
  const texts = readdirSync(dir).filter((f) => LICENSE_FILE.test(f) && statSync(join(dir, f)).isFile()).sort()
    .map((f) => readFileSync(join(dir, f), 'utf8'));
  if (!texts.length && STANDARD[license]) texts.push(`Copyright (c) ${authorOf(pkg)}\n\n${STANDARD[license]}`);
  seen.set(key, { name: pkg.name, version: pkg.version, license, source: repo.replace(/^git\+/, '').replace(/\.git$/, ''), texts });
}
const pkgs = [...seen.values()].sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));

// Group by license text, with each package's copyright lines listed above the text.
const groups = new Map();
for (const p of pkgs) {
  const pieces = p.texts.length ? p.texts : [`${p.license}: the package ships no license file; its source is ${p.source || 'on npm'}. (Used only while building the site; not served.)`];
  for (const t of pieces) {
    const lines = t.replace(/\r\n/g, '\n').split('\n');
    const notices = lines.filter((l) => COPYRIGHT.test(l)).map((l) => l.trim());
    const body = lines.filter((l) => !COPYRIGHT.test(l)).join('\n').trim().replace(/\n{3,}/g, '\n\n');
    if (!body) continue;
    if (!groups.has(body)) groups.set(body, []);
    groups.get(body).push([`${p.name} ${p.version}`, [...new Set(notices)]]);
  }
}
const ordered = [...groups.entries()].sort((a, b) => b[1].length - a[1].length || a[1][0][0].localeCompare(b[1][0][0]));
const out = [
  'Third-party software in the Penates website (getpenates.com)',
  '=============================================================',
  '',
  'The website is built from the open-source packages below. For each license text, the packages under it are',
  'listed with their copyright notices. Generated from the installed packages by site/scripts/licenses.mjs.',
  'The Penates app has its own list: https://github.com/Natetronn/penates/blob/main/THIRD-PARTY-LICENSES.txt',
  '',
];
for (const [body, members] of ordered) {
  out.push('-'.repeat(60));
  for (const [name, notices] of members) out.push(name + (notices.length ? ': ' + notices.join(' / ') : ''));
  out.push('', body, '');
}
mkdirSync(join(site, 'src/data'), { recursive: true });
writeFileSync(join(site, 'src/data/site-licenses.json'),
  JSON.stringify(pkgs.map(({ name, version, license, source }) => ({ name, version, license, source })), null, 1));
writeFileSync(join(site, 'public/THIRD-PARTY-LICENSES.txt'), out.join('\n').trimEnd() + '\n');
console.log(`licenses: ${pkgs.length} packages, ${groups.size} license texts`);
