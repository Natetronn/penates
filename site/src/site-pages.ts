// Titles and descriptions of the site's own pages (not the Starlight docs), used by each page and by its
// Open Graph image (src/pages/og/[...route].ts), so the two always match.
import catalog from './data/catalog.json';

export const sitePages = {
  index: {
    title: 'Penates: turn websites into desktop apps',
    ogTitle: 'Every site gets a home of its own',
    description: 'Penates turns websites and web apps into desktop apps, each with its own window, icon, taskbar entry and separate login. Free and open source, for Linux, Windows and macOS.',
  },
  apps: {
    title: 'Popular apps: Penates',
    ogTitle: 'Popular apps',
    description: `${catalog.length} sites ready in one click in Penates, each set up to work well as an app.`,
  },
  demo: {
    title: 'Try the Penates manager',
    ogTitle: 'Try the manager',
    description: 'The Penates manager with sample apps, running in your browser: create, edit and theme apps without installing anything.',
  },
  credits: {
    title: 'Credits: Penates',
    ogTitle: 'Credits',
    description: 'Who made Penates, what it is built with, and the licenses of the software in the app and this website.',
  },
  privacy: {
    title: 'Privacy: Penates',
    ogTitle: 'Privacy',
    description: 'What the Penates app and this website collect (very little) and what they connect to.',
  },
  terms: {
    title: 'Terms: Penates',
    ogTitle: 'Terms',
    description: 'The terms for using Penates and this website: the MIT License, no warranty, and other sites\' own terms.',
  },
} as const;

export type SitePage = keyof typeof sitePages;
