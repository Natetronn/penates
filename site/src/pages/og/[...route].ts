// Open Graph images (the previews shown when a page is shared), one per page, at /og/<page>.png: the site's
// own pages from src/site-pages.ts and every docs page from the Starlight collection.
import { getCollection } from 'astro:content';
import { OGImageRoute } from 'astro-og-canvas';
import { sitePages } from '../../site-pages';

const docs = Object.fromEntries(
  (await getCollection('docs')).map(({ id, data }) => [id, { title: data.title, description: data.description ?? '' }]),
);
const own = Object.fromEntries(Object.entries(sitePages).map(([key, p]) => [key, { title: p.ogTitle, description: p.description }]));

export const { getStaticPaths, GET } = await OGImageRoute({
  param: 'route',
  pages: { ...own, ...docs },
  getImageOptions: (_path, page: { title: string; description: string }) => ({
    title: page.title,
    description: page.description,
    logo: { path: './src/og/logo.png', size: [450] },
    bgImage: { path: './src/og/background.png', fit: 'cover' },
    padding: 72,
    font: {
      title: { families: ['Space Grotesk'], weight: 'Bold', size: 60, lineHeight: 1.1, color: [244, 242, 255] },
      description: { families: ['Noto Sans'], size: 30, lineHeight: 1.45, color: [169, 165, 196] },
    },
    fonts: [
      'https://api.fontsource.org/v1/fonts/space-grotesk/latin-700-normal.ttf',
      'https://api.fontsource.org/v1/fonts/noto-sans/latin-400-normal.ttf',
    ],
  }),
});
