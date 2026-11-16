// Starlight route middleware: point every docs page at its Open Graph image (src/pages/og/[...route].ts).
import { defineRouteMiddleware } from '@astrojs/starlight/route-data';

export const onRequest = defineRouteMiddleware((context) => {
  const route = context.locals.starlightRoute;
  if (route.id === '404') return;
  const image = new URL(`${import.meta.env.BASE_URL.replace(/\/?$/, '/')}og/${route.id}.png`, context.site).href;
  const meta = (attrs: Record<string, string>) => route.head.push({ tag: 'meta', attrs });
  meta({ property: 'og:image', content: image });
  meta({ property: 'og:image:width', content: '1200' });
  meta({ property: 'og:image:height', content: '630' });
  meta({ property: 'og:image:alt', content: `Penates: ${route.entry.data.title}` });
  meta({ name: 'twitter:image', content: image });
});
