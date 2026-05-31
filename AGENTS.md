# AGENTS.md — Coding Agent Instructions for OxideBBS

This repository is intended to be friendly to local coding agents.

## Project identity

Project name: **OxideBBS**

Canonical repository: `https://github.com/sphildreth/oxidebbs`

License: Apache-2.0

Purpose: A Rust-built BBS engine for telnet callers, ANSI/CP437 screens, DecentDB persistence, DOS door games, and future FTN-style networking.

## Hard constraints

1. Use Rust.
2. Use DecentDB as the only system database.
3. Do not add SQLite, PostgreSQL, MySQL, Redis, MongoDB, or an ORM.
4. v1 is telnet-only.
5. Physical modem/serial support is a future transport, not v1.
6. Treat ANSI/CP437 as a first-class byte-oriented terminal format.
7. Do not use Ratatui for the remote caller UI.
8. Ratatui may be used for local sysop/admin TUI.
9. Keep door execution isolated from core session logic.
10. Do not bundle copyrighted/abandonware DOS doors.

## Preferred Rust workspace shape

Keep crates small and focused:

- `oxidebbs-server`: binary entrypoint
- `oxidebbs-core`: domain/session/menu/user logic
- `oxidebbs-term`: ANSI/CP437 rendering
- `oxidebbs-telnet`: telnet transport
- `oxidebbs-db`: DecentDB repository layer
- `oxidebbs-door`: drop files and door runners
- `oxidebbs-sysop`: local sysop tooling

## Coding style

- Prefer clear code over clever code.
- Keep functions small enough to test.
- Use explicit domain types instead of strings everywhere.
- Avoid global mutable state.
- Avoid blocking calls in async tasks unless isolated with the correct runtime pattern.
- Use structured logging.
- Prefer `Result<T, Error>` over panics.
- Add tests for parser, renderer, and drop-file behavior.

## Documentation expectations

When making a significant change:

- Update `SPEC.md` if behavior changes.
- Update `PRD.md` if product scope changes.
- Add an ADR for architectural decisions.
- Update `TASKS.md` when completing or adding work.
- Update examples in `config/oxidebbs.example.toml`.

## Definition of done

A task is not done until:

- Code compiles.
- Tests pass.
- `cargo fmt` is clean.
- Clippy has no warnings.
- Relevant docs are updated.
- New behavior has at least basic tests.
- The change does not violate the hard constraints above.

## Initial implementation order

1. Workspace compiles.
2. Config loader.
3. ANSI asset loader.
4. Telnet listener skeleton.
5. Transport abstraction.
6. Session loop.
7. User repository.
8. Login/new-user flow.
9. Menu router.
10. Local message base.
11. Door definition loader.
12. Drop-file generator.
13. DOSBox runner dry-run.
14. Real door execution.
15. Sysop CLI.
