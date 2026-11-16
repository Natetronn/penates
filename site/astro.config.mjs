// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://getpenates.com',
  integrations: [
    starlight({
      title: 'Penates',
      description: 'Turn websites into desktop apps, each with its own window, icon, taskbar entry and login.',
      logo: { light: './src/assets/lockup-light.svg', dark: './src/assets/lockup-dark.svg', replacesTitle: true, alt: 'Penates' },
      favicon: '/favicon.svg',
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/Natetronn/penates' }],
      editLink: { baseUrl: 'https://github.com/Natetronn/penates/edit/main/site/' },
			// Cloudflare Web Analytics: cookieless page counts; also on the home page (src/pages/index.astro).
			head: [
				{
					tag: 'script',
					attrs: {
						type: 'module',
						src: 'https://static.cloudflareinsights.com/beacon.min.js',
						'data-cf-beacon': '{"token": "3601b300af25499b83bbddf47fd614c1"}',
					},
				},
			],
			routeMiddleware: './src/routeData.ts',
			customCss: ['@fontsource-variable/space-grotesk', './src/styles/penates.css'],
      lastUpdated: false,
      sidebar: [
        { label: 'Start here', items: [{ slug: 'getting-started' }, { label: 'Popular apps', link: '/apps/' }, { label: 'Try the manager', link: '/demo/' }] },
        { label: 'Using Penates', items: ['cli', 'configuration', 'updates', 'sharing', 'theming'].map((d) => ({ slug: `docs/${d}` })) },
        { label: 'Under the hood', items: ['architecture', 'packaging', 'releasing', 'status', 'security'].map((d) => ({ slug: `docs/${d}` })) },
      ],
    }),
  ],
});
