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
      { text: 'Docker', link: '/project/docker' },
      { text: 'Setup', link: '/project/setup' },
      { text: 'Architecture', link: '/project/architecture' },
      { text: 'FTN', link: '/ftn/architecture' },
      { text: 'Menus', link: '/project/menus' },
      { text: 'Caller Commands', link: '/project/caller-commands' },
      { text: 'Doors', link: '/project/doors' },
      { text: 'Security Levels', link: '/project/security-levels' },
      { text: 'Sysop CLI', link: '/project/sysop-cli' },
      { text: 'OxideNet', link: '/oxidenet/overview' },
      { text: 'Sysop Themes', link: '/project/sysop-tui-themes' },
      { text: 'Versioning', link: '/project/versioning' },
      { text: 'Changelog', link: '/about/changelog' }
    ],
    sidebar: [
      {
        text: 'Project',
        items: [
          { text: 'Overview', link: '/' },
          { text: 'Getting Started', link: '/project/getting-started' },
          { text: 'Docker Deployment', link: '/project/docker' },
          { text: 'Setup Wizard', link: '/project/setup' },
          { text: 'DOSEMU2 On Fedora', link: '/project/dosemu2-fedora' },
          { text: 'Architecture', link: '/project/architecture' },
          { text: 'Menu System', link: '/project/menus' },
          { text: 'Caller Commands', link: '/project/caller-commands' },
          { text: 'Doors', link: '/project/doors' },
          { text: 'File Transfers', link: '/project/file-transfers' },
          { text: 'Serial And Modem', link: '/project/serial' },
          { text: 'User Security Levels', link: '/project/security-levels' },
          { text: 'Sysop CLI', link: '/project/sysop-cli' },
          { text: 'Sysop TUI Themes', link: '/project/sysop-tui-themes' },
          { text: 'Remote Admin', link: '/project/remote-admin' },
          { text: 'Versioning', link: '/project/versioning' },
          { text: 'Deployment', link: '/project/deployment' }
        ]
      },
      {
        text: 'FTN Networking',
        items: [
          { text: 'Architecture', link: '/ftn/architecture' },
          { text: 'Configuration', link: '/ftn/configuration' },
          { text: 'CLI', link: '/ftn/cli' },
          { text: 'Tosser', link: '/ftn/tosser' },
          { text: 'Scanner', link: '/ftn/scanner' },
          { text: 'Nodelists', link: '/ftn/nodelist' },
          { text: 'Packet Format', link: '/ftn/packet-format' },
          { text: 'Echomail', link: '/ftn/echomail' },
          { text: 'Netmail', link: '/ftn/netmail' },
          { text: 'Netmail Routing', link: '/ftn/netmail-routing' },
          { text: 'AreaFix', link: '/ftn/areafix' },
          { text: 'Bundles', link: '/ftn/bundles' },
          { text: 'BinkP', link: '/ftn/binkp' },
          { text: 'Troubleshooting', link: '/ftn/troubleshooting' }
        ]
      },
      {
        text: 'OxideNet',
        items: [
          { text: 'Overview', link: '/oxidenet/overview' },
          { text: 'Addressing', link: '/oxidenet/addressing' },
          { text: 'Policy', link: '/oxidenet/policy' },
          { text: 'Areas', link: '/oxidenet/areas' },
          { text: 'Registry', link: '/oxidenet/registry' },
          { text: 'Config Package', link: '/oxidenet/config-package' },
          { text: 'Setup Member', link: '/oxidenet/setup-member' },
          { text: 'Hub Admin', link: '/oxidenet/hub-admin' },
          { text: 'Troubleshooting', link: '/oxidenet/troubleshooting' }
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
