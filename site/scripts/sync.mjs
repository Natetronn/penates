// Copy what the site shows from the rest of the repo, so there is one source for each:
//   docs/*.md             -> src/content/docs/docs/*.md   (title from the first heading; links rewritten)
//   docs/images/*         -> src/assets/screens/          (screenshots, optimized by Astro)
//   design/lockup-*.svg   -> src/assets/                  (the header logo)
//   design/preview/…html  -> public/demo/index.html       (the clickable manager)
//   the app catalog       -> src/data/catalog.json        (live, else the copy built into Penates)
// Runs before `astro dev` and `astro build`; the outputs are git-ignored.
import { cpSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const root = new URL('../..', import.meta.url).pathname;
const site = new URL('..', import.meta.url).pathname;
const REPO = 'https://github.com/Natetronn/penates/blob/main/';

// Docs: Starlight wants a title in frontmatter; the repo's docs open with "# Title" so GitHub renders them well.
const docsOut = join(site, 'src/content/docs/docs');
rmSync(docsOut, { recursive: true, force: true });
mkdirSync(docsOut, { recursive: true });
for (const file of readdirSync(join(root, 'docs')).filter((f) => f.endsWith('.md'))) {
  const text = readFileSync(join(root, 'docs', file), 'utf8');
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

cpSync(join(root, 'docs/images'), join(site, 'src/assets/screens'), { recursive: true });
mkdirSync(join(site, 'src/assets'), { recursive: true });
for (const f of ['lockup-dark.svg', 'lockup-light.svg']) cpSync(join(root, 'design', f), join(site, 'src/assets', f));
cpSync(join(root, 'src-tauri/icons/icon.svg'), join(site, 'public/favicon.svg'));
mkdirSync(join(site, 'public/demo'), { recursive: true });
cpSync(join(root, 'design/preview/penates-manager.html'), join(site, 'public/demo/index.html'));

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
