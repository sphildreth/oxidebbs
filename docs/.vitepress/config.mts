import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'OxideBBS',
  description:
    'A Rust BBS engine for telnet callers, ANSI/CP437 screens, DecentDB persistence, and DOS doors.',
  lang: 'en-US',
  base: '/',
  cleanUrls: true,
  lastUpdated: true,
  head: [
    ['link', { rel: 'icon', href: '/favicon.svg' }],
    ['meta', { name: 'theme-color', content: '#0f766e' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'OxideBBS' }]
  ],
  themeConfig: {
    logo: '/favicon.svg',
    siteTitle: 'OxideBBS',
    nav: [
      { text: 'Guide', link: '/project/getting-started' },
      { text: 'Setup', link: '/project/setup' },
      { text: 'Architecture', link: '/project/architecture' },
      { text: 'Menus', link: '/project/menus' },
      { text: 'Sysop CLI', link: '/project/sysop-cli' },
      { text: 'Versioning', link: '/project/versioning' },
      { text: 'Changelog', link: '/about/changelog' }
    ],
    sidebar: [
      {
        text: 'Project',
        items: [
          { text: 'Overview', link: '/' },
          { text: 'Getting Started', link: '/project/getting-started' },
          { text: 'Setup Wizard', link: '/project/setup' },
          { text: 'Architecture', link: '/project/architecture' },
          { text: 'Menu System', link: '/project/menus' },
          { text: 'Sysop CLI', link: '/project/sysop-cli' },
          { text: 'Versioning', link: '/project/versioning' },
          { text: 'Deployment', link: '/project/deployment' }
        ]
      },
      {
        text: 'About',
        items: [{ text: 'Changelog', link: '/about/changelog' }]
      }
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/sphildreth/oxidebbs' }
    ],
    search: {
      provider: 'local'
    },
    footer: {
      message: 'Apache-2.0 licensed.',
      copyright: 'Copyright © 2026 OxideBBS contributors'
    }
  }
})
