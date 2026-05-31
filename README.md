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

This is a starter repository scaffold. It is meant to give the project shape before implementation begins.

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
├── docs/
│   ├── adr/                 # architecture decision records
│   ├── ARCHITECTURE.md
│   ├── ANSI_CP437.md
│   ├── DECENTDB_SCHEMA.md
│   ├── DOORS.md
│   ├── FTN_NETWORKING.md
│   ├── TELNET.md
│   └── RUNBOOK.md
├── config/
│   └── oxidebbs.example.toml
├── assets/
│   └── ansi/
├── PRD.md
├── SPEC.md
├── ROADMAP.md
├── TASKS.md
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
cargo check --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

## Canonical repository

GitHub is the canonical home for OxideBBS:

```text
https://github.com/sphildreth/oxidebbs
```

A Codeberg mirror may be added later if the project wants a secondary FOSS-community presence.

## License

OxideBBS is licensed under the Apache License, Version 2.0. See `LICENSE` and `docs/LICENSING.md`.
