# Recommended Stack

## Language

Rust.

## Runtime

Tokio is expected for async networking and session tasks.

## Database

DecentDB only.

No SQLite, PostgreSQL, MySQL, Redis, MongoDB, or ORM layer.

Use the native Rust `decentdb` crate pinned to a released Git tag from
`https://github.com/sphildreth/decentdb`. Do not require developers to keep a
separate local DecentDB checkout just to build OxideBBS.

## Terminal

Remote caller UI:

- custom ANSI/CP437 renderer
- byte-oriented output
- SyncTERM-friendly behavior

Local sysop console:

- Ratatui
- Crossterm backend

## Configuration

- TOML
- serde

## Logging

- tracing
- tracing-subscriber

## CLI

- clap

## Door runtime

v1:

- DOSBox runner

Future:

- DOSBox-X runner
- DOSEMU2 runner
- Native door API
