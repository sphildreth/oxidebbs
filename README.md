# OxideBBS

**OxideBBS** is a modern, Rust-built BBS engine for classic ANSI/telnet culture, DOS door games, and FTN-style message networking.

The project is intentionally retro in user experience and modern in implementation:

- Rust-first BBS runtime
- Telnet-only for v1
- ANSI/CP437-first terminal rendering
- DecentDB as the only system database
- DOSBox/DOSEMU-compatible door runner architecture
- Future-ready transport layer for physical serial/modem support
- Future-ready message-network layer for OxideNet / FTN-style networks

> Working motto: **Built for sysops. Driven by code.**

## Current status

This is a starter implementation with tested Rust crates for configuration,
terminal assets, telnet negotiation, DecentDB repositories, menu routing, local
message commands, door drop files, a CLI-first sysop interface, and a first
telnet `serve` runtime that accepts callers and routes the configured starter
menus.

## Repository layout

```text
.
├── crates/
│   ├── oxidebbs-server/     # main daemon binary
│   ├── oxidebbs-core/       # users, sessions, nodes, permissions, menu routing
│   ├── oxidebbs-term/       # ANSI/CP437 rendering and terminal input helpers
│   ├── oxidebbs-telnet/     # telnet transport and negotiation
│   ├── oxidebbs-db/         # DecentDB repository layer and schema helpers
│   ├── oxidebbs-door/       # door metadata, drop files, runners
│   └── oxidebbs-sysop/      # local sysop/admin TUI and CLI helpers
├── design/
│   ├── adr/                 # architecture decision records
│   ├── ARCHITECTURE.md
│   ├── ANSI_CP437.md
│   ├── DECENTDB_SCHEMA.md
│   ├── DOORS.md
│   ├── FTN_NETWORKING.md
│   ├── LICENSING.md
│   ├── PRD.md
│   ├── ROADMAP.md
│   ├── RUNBOOK.md
│   ├── SPEC.md
│   ├── TASKS.md
│   └── TELNET.md
├── docs/
│   ├── .vitepress/          # VitePress config
│   ├── index.md             # documentation home
│   ├── project/             # project documentation pages
│   └── public/              # static files copied into the site build
├── config/
│   ├── doors.example.toml
│   └── oxidebbs.example.toml
├── assets/
│   ├── ansi/
│   └── screens/             # login, info, and menu screen assets
├── Cargo.lock
├── package.json
├── package-lock.json
└── AGENTS.md
```

## Non-goals for v1

OxideBBS v1 is not trying to be every BBS ever written.

Explicit non-goals:

- No web-first architecture
- No PostgreSQL, SQLite, MySQL, or external system database
- No file-transfer subsystem as a primary v1 feature
- No physical modem support until the telnet version is stable
- No attempt to perfectly emulate Telegard, Renegade, WWIV, Synchronet, or Mystic feature-for-feature
- No bundling of questionable abandonware doors

## First development target

The first useful milestone is:

```text
A caller can connect over telnet, create/log into an account, view ANSI menus,
read/post a local message, launch one configured DOS door, and disconnect cleanly.
```

## Development commands

```bash
cargo fmt --all
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Setup wizard

Create a starter local board config with:

```bash
cargo run -p oxidebbs-server -- setup
```

The wizard writes `config/oxidebbs.toml` by default and creates the starter
paths for data, assets, doors, runtime files, and logs.

## Run the server

Start the telnet listener with:

```bash
cargo run -p oxidebbs-server -- serve
```

After `setup`, the CLI uses `config/oxidebbs.toml` by default. A clean checkout
without that file falls back to `config/oxidebbs.example.toml`.

The current runtime accepts telnet callers, assigns node slots, writes session
and audit records to DecentDB, renders configured login/main menu screens, and
routes starter menu keys. User registration, login authentication, and local
message reading/posting are wired into DecentDB-backed sessions. The caller
`Doors` menu lists enabled configured doors, validates the selection, launches
the configured runner, bridges caller/process bytes, records `door_runs`, and
returns normal exits or timeouts to the main menu.

During pre-alpha, delete or recreate old development `.ddb` files after schema
version changes; OxideBBS refuses to run against stale schema markers.

## Sysop CLI

Common local administration uses top-level command groups:

```bash
cargo run -p oxidebbs-server -- check
cargo run -p oxidebbs-server -- status
cargo run -p oxidebbs-server -- users list
cargo run -p oxidebbs-server -- nodes list
cargo run -p oxidebbs-server -- messages areas list
cargo run -p oxidebbs-server -- doors list
cargo run -p oxidebbs-server -- ansi list
cargo run -p oxidebbs-server -- db doctor
```

## Documentation site

The documentation site is built with VitePress from `docs/` and deployed to
GitHub Pages for `https://oxidebbs.com`.

```bash
npm ci
npm run docs:dev
npm run docs:build
```

## Canonical repository

GitHub is the canonical home for OxideBBS:

```text
https://github.com/sphildreth/oxidebbs
```

A Codeberg mirror may be added later if the project wants a secondary FOSS-community presence.

## License

OxideBBS is licensed under the Apache License, Version 2.0. See `LICENSE` and `design/LICENSING.md`.
