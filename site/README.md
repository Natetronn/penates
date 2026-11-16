# Penates site

The website and docs for Penates, built with [Astro](https://astro.build) and
[Starlight](https://starlight.astro.build).

```sh
npm install
npm run dev     # http://localhost:4321/
npm run build   # into dist/
```

The docs pages come from the repo's `docs/*.md`, the screenshots from `docs/images/`, the logo from `design/`,
the clickable manager from `design/preview/`, and the app list from the app catalog: `scripts/sync.mjs` copies
them in before each build. Edit them there, not here. Pages that exist only on the site live in
`src/content/docs/` (Starlight) and `src/pages/` (the home, demo and app gallery, with `src/layouts/Site.astro`).
The app logos in `src/assets/app-icons/` come from `scripts/app-icons.sh`; run it after the catalog gains apps.
The Credits page and `THIRD-PARTY-LICENSES.txt` list the site's own packages and licenses; `scripts/licenses.mjs`
regenerates them on every build. It is served by Cloudflare Workers (`wrangler.jsonc`) at
https://getpenates.com; Cloudflare builds and deploys it on each push to `main` that touches it or its sources.
