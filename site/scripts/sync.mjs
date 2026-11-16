// Copy what the site shows from the rest of the repo, so there is one source for each. The site describes
// the latest release, so these come from that release's source on GitHub; `--local` (npm run dev) reads the
// working copy instead, to preview edits before a release. The app catalog is always the live one.
//   docs/*.md             -> src/content/docs/docs/*.md   (title from the first heading; links rewritten)
//   docs/images/*         -> src/assets/screens/          (screenshots, optimized by Astro)
//   design/lockup-*.svg   -> src/assets/                  (the header logo)
//   design/preview/…html  -> public/demo/manager.html     (the clickable manager, shown by src/pages/demo.astro)
//   the app catalog       -> src/data/catalog.json        (live, else the copy built into Penates)
// Runs before `astro dev` and `astro build`; the outputs are git-ignored.
import { cpSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const root = new URL('../..', import.meta.url).pathname;
const site = new URL('..', import.meta.url).pathname;
const REPO = 'https://github.com/Natetronn/penates/blob/main/';

// Where the docs, screenshots, logo and demo come from: the latest release, or the working copy.
let src = root;
if (!process.argv.includes('--local')) {
  const latest = await fetch('https://github.com/Natetronn/penates/releases/latest', { redirect: 'manual' });
  const tag = latest.headers.get('location')?.match(/\/tag\/([^/?#]+)/)?.[1];
  if (!tag) throw new Error('could not find the latest release (run with --local to use the working copy)');
  const archive = await fetch(`https://github.com/Natetronn/penates/archive/refs/tags/${tag}.tar.gz`);
  if (!archive.ok) throw new Error(`could not download ${tag}: ${archive.status}`);
  src = mkdtempSync(join(tmpdir(), 'penates-release-'));
  writeFileSync(join(src, 'source.tar.gz'), Buffer.from(await archive.arrayBuffer()));
  execFileSync('tar', ['-xzf', join(src, 'source.tar.gz'), '-C', src, '--strip-components=1']);
  console.log(`site content from ${tag}`);
}

// Docs: Starlight wants a title in frontmatter; the repo's docs open with "# Title" so GitHub renders them well.
const docsOut = join(site, 'src/content/docs/docs');
rmSync(docsOut, { recursive: true, force: true });
mkdirSync(docsOut, { recursive: true });
for (const file of readdirSync(join(src, 'docs')).filter((f) => f.endsWith('.md'))) {
  const text = readFileSync(join(src, 'docs', file), 'utf8');
  const [, title = file.replace(/\.md$/, ''), body = text] = text.match(/^#\s+(.+)\n+([\s\S]*)$/) ?? [];
  const linked = body.replace(/\]\(([^)\s]+)\)/g, (all, href) => {
    if (/^(https?:|mailto:|#)/.test(href)) return all;
    const [path, hash = ''] = href.split('#');
    const anchor = hash ? `#${hash}` : '';
    if (/^[A-Z][A-Z_]*\.md$/.test(path)) return `](../${path.replace(/\.md$/, '').toLowerCase()}/${anchor})`;
    return `](${REPO}${join('docs', path).replace(/^docs\/\.\.\//, '')}${anchor})`; // e.g. ../CHANGELOG.md
  });
  const description = (linked.split('\n\n').find((p) => p && !p.startsWith('#') && !p.startsWith('|') && !p.startsWith('`')) ?? '')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1').replace(/[*`]/g, '').replace(/\s+/g, ' ').trim().slice(0, 160);
  writeFileSync(join(docsOut, file.toLowerCase()),
    `---\ntitle: ${JSON.stringify(title)}\ndescription: ${JSON.stringify(description)}\neditUrl: ${REPO}docs/${file}\n---\n\n${linked}`);
}

// SECURITY.md (repo root) is a docs page too, so the policy has one source.
{
  const text = readFileSync(join(src, 'SECURITY.md'), 'utf8');
  const body = text.replace(/^#\s+.+\n+/, '');
  writeFileSync(join(docsOut, 'security.md'),
    `---\ntitle: "Security"\ndescription: "How to report a security problem in Penates, and what is in scope."\neditUrl: ${REPO}SECURITY.md\n---\n\n${body}`);
}
cpSync(join(src, 'docs/images'), join(site, 'src/assets/screens'), { recursive: true });
mkdirSync(join(site, 'src/assets'), { recursive: true });
for (const f of ['lockup-dark.svg', 'lockup-light.svg']) cpSync(join(src, 'design', f), join(site, 'src/assets', f));
cpSync(join(src, 'src-tauri/icons/icon.svg'), join(site, 'public/favicon.svg'));
// Open Graph images: the background (src/og/background.svg) and the logo lockup as PNGs for astro-og-canvas.
{
  const sharp = (await import('sharp')).default;
  await sharp(join(site, 'src/og/background.svg')).png().toFile(join(site, 'src/og/background.png'));
  await sharp(join(src, 'design/lockup-dark.svg'), { density: 300 }).resize({ width: 660 }).png().toFile(join(site, 'src/og/logo.png'));
}
rmSync(join(site, 'public/demo'), { recursive: true, force: true });
mkdirSync(join(site, 'public/demo'), { recursive: true });
cpSync(join(src, 'design/preview/penates-manager.html'), join(site, 'public/demo/manager.html'));

// The catalog as people get it: the live file, or Penates's built-in copy when offline.
let catalog;
try {
  const res = await fetch('https://raw.githubusercontent.com/Natetronn/penates-apps/main/catalog.json', { signal: AbortSignal.timeout(10000) });
  if (!res.ok) throw new Error(res.statusText);
  catalog = await res.json();
} catch {
  catalog = JSON.parse(readFileSync(join(root, 'crates/penates-core/data/presets.json'), 'utf8'));
}
mkdirSync(join(site, 'src/data'), { recursive: true });
writeFileSync(join(site, 'src/data/catalog.json'), JSON.stringify(catalog.catalog.map(({ id, name, url, notes }) => ({ id, name, url, notes }))
  .sort((a, b) => a.name.localeCompare(b.name)), null, 2));
console.log(`synced ${readdirSync(docsOut).length} docs, images, logos, demo and ${catalog.catalog.length} catalog apps`);

// The website's own third-party licenses (src/data/site-licenses.json, public/THIRD-PARTY-LICENSES.txt).
await import('./licenses.mjs');
