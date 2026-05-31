# Recommended Stack

## Language

Rust.

## Runtime

Tokio is expected for async networking and session tasks.

## Database

DecentDB only.

No SQLite, PostgreSQL, MySQL, Redis, MongoDB, or ORM layer.

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
