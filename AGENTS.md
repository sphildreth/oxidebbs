# AGENTS.md — OxideBBS

Rust BBS engine for telnet callers, ANSI/CP437 screens, DecentDB persistence, and DOS door games.

# Personal Codex operating rules

## Model and delegation preference

Prefer GPT-5.3-Codex-Spark for coding work whenever practical.

For any task that involves codebase exploration, bug investigation, refactoring analysis, test discovery, PR review, dependency review, or multi-file implementation planning:

1. Do not perform all exploration in the main thread unless the task is tiny.
2. Spawn one or more subagents first.
3. Prefer `phase_executor_spark` for read-only discovery.
4. Prefer `cheap_code_fixer` for small or medium implementation tasks.
5. Keep the parent thread focused on orchestration, synthesis, and final decision-making.
6. Use gpt-5.5 only when explicitly requested or when the task truly requires deep reasoning that Spark cannot handle.

When spawning agents, wait for all results, consolidate findings, then proceed.

## Validate changes

The CI gate is `./scripts/dev-check.sh`. It runs in this order:

```bash
cargo fmt --all --check        # must be clean
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Always pass all four before considering a change done. Use `--locked` because `Cargo.lock` is committed.

## Git write approval

Agents must never create commits, create tags, push branches, merge pull requests,
or publish releases without explicit approval from the user in the current
conversation. Leave completed work in the worktree until approval is given.

## Native build prerequisite

DecentDB requires `clang` and `libclang-dev`. Install before building:

```bash
sudo apt-get install -y clang libclang-dev
```

## Workspace layout

```
crates/
  oxidebbs-server/    # binary entrypoint: config, telnet/serial serving, web admin UI, web terminal, binkp listener, sysop CLI
  oxidebbs-core/      # domain: sessions, menus, users, permissions, messages, nodes, network adapters
  oxidebbs-term/      # ANSI/CP437 rendering, AnsiBuffer, CP437 encode/decode
  oxidebbs-telnet/    # telnet transport/negotiation plus serial/modem transport (serialport)
  oxidebbs-db/        # DecentDB repository layer, OxideDb, schema init/migrations
  oxidebbs-door/      # door definitions, drop files, DOS door runners, OxDoor packages
  oxidebbs-sysop/     # local sysop admin ratatui TUI and CLI
  oxidebbs-network/   # protocol-neutral network types (FTN addresses, profiles, links, envelopes)
  oxidebbs-transfer/  # caller file transfer protocols (XMODEM-CRC, ZMODEM)
  oxidebbs-ftn/       # FTN packets, bundles, tosser/scanner, areafix, nodelist, routing
  oxidebbs-binkp/     # BinkP mail transport: framing, client/server sessions, TLS
  oxidebbs-oxidenet/  # OxideNet profile, addressing defaults, node registry, config packages
design/               # ARCHITECTURE.md, SPEC.md, PRD.md, TASKS.md, ADRs
docs/                 # VitePress documentation site (Node/npm)
config/               # oxidebbs.example.toml
scripts/              # dev-check.sh
```

All 12 crates are fully implemented; there are no remaining stubs. The current
release version lives in the root `VERSION` file — see `design/VERSIONING_GUIDE.md`.

## Dependency direction

```
server    -> core, term, telnet, db, door, sysop, transfer, ftn, binkp, oxidenet
             (all library crates except network, reached transitively via core)
core      -> network
door      -> core
sysop     -> db, door, oxidenet
ftn       -> network, db
oxidenet  -> network, db
transfer  -> telnet
term, telnet, db, network, binkp -> no internal deps
```

Lower-level crates must not depend on `oxidebbs-server`.

## Hard constraints

1. Rust only, edition 2024.
2. DecentDB is the only database. No SQLite, Postgres, MySQL, Redis, MongoDB, or ORM.
3. v1 is telnet-first; serial/modem transport shipped in v1.2 per ADR 0019.
4. ANSI/CP437 is byte-oriented, not Unicode-first for the caller UI.
5. Do not use Ratatui for remote caller UI. Ratatui is permitted for local sysop TUI only.
6. Keep door execution isolated from core session logic.
7. Do not bundle copyrighted/abandonware DOS doors.

## Workspace dependencies

All shared deps are declared in the root `[workspace.dependencies]`. Member crates reference them with `dep.workspace = true`. Use `cargo add` to add new deps; do not hand-edit versions.

Key deps: `thiserror`, `serde`, `tokio` (full features), `tracing`, `clap` (derive+env), `decentdb` (git tag v2.8.0), `axum` (server web UI), `argon2`, `serialport`, `zip`, `time`, `ratatui`/`crossterm` (sysop TUI only).

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
