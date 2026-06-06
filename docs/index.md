---
layout: home

hero:
  name: OxideBBS
  text: A modern BBS for sysops
  tagline: ANSI/CP437 callers, browser terminal, telnet, DOS doors, file areas, DecentDB persistence, FTN/BinkP, and OxideNet in one server.
  image:
    src: /logo.png
    alt: OxideBBS logo
  actions:
    - theme: brand
      text: Get Started
      link: /project/getting-started
    - theme: alt
      text: Docker
      link: /project/docker
    - theme: alt
      text: Release Binaries
      link: /project/release-binaries

features:
  - title: Sysop-first setup
    details: Start with Docker, use packaged release binaries, or build from source only when you need local patches.
  - title: ANSI/CP437 caller UI
    details: Remote callers are treated as byte-oriented terminal users, with ANSI art, CP437 line art, plain fallback screens, and C64-friendly profiles.
  - title: Browser terminal included
    details: The optional /terminal surface gives LAN and reverse-proxy deployments a browser caller path with ZMODEM support.
  - title: Doors that behave like doors
    details: Local DOS doors run through DOSEMU2 COM1 drop-file sessions, with per-node runtime isolation and persistent door working directories.
  - title: File areas and transfers
    details: Sysops can create file areas, import files, review uploads, and offer caller ZMODEM or negotiated XMODEM downloads.
  - title: FTN and OxideNet
    details: Built-in FTN packet processing, BinkP polling/listening, AreaFix, nodelists, and first-party OxideNet workflows.
  - title: Local sysop tools
    details: Use the CLI or Ratatui sysop console for users, nodes, messages, doors, files, logs, audits, database checks, and network operations.
  - title: DecentDB persistence
    details: Users, sessions, messages, files, doors, audits, and network state live in DecentDB with backup, export, import, and compact commands.
---
