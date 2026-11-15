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
			components: { Hero: './src/components/Hero.astro' },
			customCss: ['./src/styles/penates.css'],
      lastUpdated: false,
      sidebar: [
        { label: 'Start here', items: [{ slug: 'getting-started' }, { slug: 'apps' }, { label: 'Try the manager', link: '/demo/', attrs: { target: '_blank' } }] },
        { label: 'Using Penates', items: ['cli', 'configuration', 'updates', 'sharing', 'theming'].map((d) => ({ slug: `docs/${d}` })) },
        { label: 'Under the hood', items: ['architecture', 'packaging', 'releasing', 'status'].map((d) => ({ slug: `docs/${d}` })) },
      ],
    }),
  ],
});
