# AGENTS.md — OxideBBS

Rust BBS engine for telnet callers, ANSI/CP437 screens, DecentDB persistence, and DOS door games.

## Validate changes

The CI gate is `./scripts/dev-check.sh`. It runs in this order:

```bash
cargo fmt --all --check        # must be clean
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Always pass all four before considering a change done. Use `--locked` because `Cargo.lock` is committed.

## Native build prerequisite

DecentDB requires `clang` and `libclang-dev`. Install before building:

```bash
sudo apt-get install -y clang libclang-dev
```

## Workspace layout

```
crates/
  oxidebbs-server/   # binary entrypoint (main.rs)
  oxidebbs-core/     # domain: sessions, menus, users, permissions
  oxidebbs-term/     # ANSI/CP437 rendering, AnsiBuffer, CP437 encode/decode
  oxidebbs-telnet/   # telnet transport and negotiation (stub)
  oxidebbs-db/       # DecentDB repository layer, OxideDb, schema init
  oxidebbs-door/     # door definitions, drop files, runners (stub)
  oxidebbs-sysop/    # local sysop admin TUI/CLI (stub)
design/              # ARCHITECTURE.md, SPEC.md, PRD.md, TASKS.md, ADRs
docs/                # VitePress documentation site (Node/npm)
config/              # oxidebbs.example.toml
scripts/             # dev-check.sh
```

Only `oxidebbs-db` and `oxidebbs-term` have real implementation. Everything else is scaffolded stubs.

## Dependency direction

```
server -> core -> term, db, door, telnet
sysop  -> core, db
```

Lower-level crates must not depend on `oxidebbs-server`.

## Hard constraints

1. Rust only, edition 2024.
2. DecentDB is the only database. No SQLite, Postgres, MySQL, Redis, MongoDB, or ORM.
3. Telnet-only for v1. No physical modem/serial yet.
4. ANSI/CP437 is byte-oriented, not Unicode-first for the caller UI.
5. Do not use Ratatui for remote caller UI. Ratatui is permitted for local sysop TUI only.
6. Keep door execution isolated from core session logic.
7. Do not bundle copyrighted/abandonware DOS doors.

## Workspace dependencies

All shared deps are declared in the root `[workspace.dependencies]`. Member crates reference them with `dep.workspace = true`. Use `cargo add` to add new deps; do not hand-edit versions.

Key deps: `anyhow`, `thiserror`, `serde`, `tokio` (full features), `tracing`, `clap` (derive+env), `decentdb` (git tag v2.8.0).

## Rust code generation rules

Detailed rules are in `.github/rust-code-generation/SKILL.md`. Key points:

- Prefer `Result<T, E>` with typed errors. No `unwrap()`/`expect()` in library code.
- No new crate additions without justification.
- Never hold a lock across `.await`.
- Layout/ABI changes are effectively irreversible.
- Read surrounding code before editing.

## Agent prompt templates

`.github/prompts/` has reusable prompt files for implementing features, debugging failures, and reviewing changes.

## Documentation expectations

When making a significant change:

- `design/SPEC.md` if behavior changes.
- `design/PRD.md` if product scope changes.
- `design/TASKS.md` when completing or adding work.
- Add ADR in `design/adr/` for architectural decisions.
- Update `config/oxidebbs.example.toml` if config schema changes.

## VitePress docs site

The docs site in `docs/` is built with VitePress and deployed to GitHub Pages. It is separate from the Rust build:

```bash
npm ci
npm run docs:dev      # local preview
npm run docs:build    # production build
```
