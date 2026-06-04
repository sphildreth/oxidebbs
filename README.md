<p align="center">
    <img src="graphics/logo.png" alt="OxideBBS logo" width="440" />
</p>

<p align="center">
    <a href="https://www.rust-lang.org">
        <img src="https://img.shields.io/badge/language-Rust-orange" alt="Language: Rust" />
    </a>
    <a href="LICENSE">
        <img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="License: Apache-2.0" />
    </a>
    <a href="https://github.com/sphildreth/oxidebbs/actions/workflows/ci.yml">
        <img src="https://github.com/sphildreth/oxidebbs/actions/workflows/ci.yml/badge.svg" alt="CI" />
    </a>
    <a href="https://github.com/sphildreth/oxidebbs/releases">
        <img src="https://img.shields.io/github/v/release/sphildreth/oxidebbs?sort=semver" alt="Latest release" />
    </a>
    <a href="https://oxidebbs.com">
        <img src="https://img.shields.io/badge/docs-oxidebbs.com-2ea44f" alt="Documentation" />
    </a>
</p>

# OxideBBS

**OxideBBS** is a modern Rust BBS engine for telnet callers, ANSI/CP437
screens, DecentDB persistence, local sysop operations, and DOS door games.

It keeps the caller experience byte-oriented and classic while giving sysops a
clean Rust codebase, a real database layer, auditable runtime operations, Docker
deployment, and a GitHub-native release workflow.

> **Built for sysops. Driven by code.**

## Features

- 📡 **Telnet Caller Runtime** - Multi-node telnet serving with session
  lifecycle tracking, idle timeout handling, graceful shutdown, and live node
  state.
- 🎨 **ANSI/CP437-First UI** - Raw ANSI assets, CP437 conversion, 40-column and
  80-column screen profiles, paging, caller-safe prompts, and CRLF-normalized
  telnet output.
- 🧭 **Configurable Menus** - Login, main, info, message, door, logoff, and
  nested submenu routing with hotkey-driven caller commands.
- 👤 **Accounts And Auth** - New-user creation, Argon2id password hashing,
  sysop accounts, security levels, persistent lockout state, and audit-friendly
  login outcomes.
- 💬 **Local Message Areas** - DecentDB-backed message areas, posting, reading,
  replies, private-mail foundation, moderation fields, and SQL-side visibility
  filtering.
- 🕹️ **DOS Door Support** - DOSEMU2-based live door launches, `DOOR.SYS` and
  `DORINFO1.DEF` drop files, per-node runtime directories, byte bridging,
  timeout cleanup, and durable `door_runs` records.
- 🧪 **Bundled Door Fixture** - Oxide-owned `oxide-check` DOS test door for
  validating drop files, DOSEMU2 command planning, and live COM1 byte flow
  without bundling third-party doorware.
- 🗄️ **DecentDB Persistence** - [DecentDB](https://github.com/sphildreth/decentdb) is the only system database, with
  schema markers, migrations, users, sessions, messages, doors, audit events,
  backup/export/import, and doctor checks.
- 🧑‍💻 **Sysop CLI And TUI** - Local CLI commands plus a Ratatui sysop console
  for dashboard, nodes, users, doors, messages, database, logs, config, ANSI,
  audit, doctor, and read-only workflows.
- 🔌 **Local Control Socket** - Same-UID Unix control channel for live status,
  node list/show, disconnects, direct node messages, broadcasts, and stale-node
  recovery.
- 📝 **Operational Logs And Audit Trail** - Configurable text or JSON logs,
  rotation, audit retention, startup/shutdown events, door events, auth events,
  and sysop action records.
- 🐳 **Docker Deployment** - Cross-platform Docker Compose path for Windows,
  macOS, and Linux hosts with OxideBBS, DOSEMU2, assets, config, and the test
  door inside a Linux runtime image.
- 🧱 **Modular Rust Workspace** - Focused crates for server, core domain, term,
  telnet, DecentDB repositories, doors, and local sysop tooling.

## Sysop TUI

<p align="center">
    <img src="graphics/screenshots/sysop-tui-oxide-theme.png" alt="OxideBBS sysop TUI dashboard showing node map, recent events, health, and alerts" width="900" />
</p>

## Quick Start With Docker

Docker Compose is the recommended cross-platform deployment path. It includes
DOSEMU2 and the bundled Oxide-owned test door fixture.

```bash
OXIDEBBS_SYSOP_PASSWORD='choose-a-real-password' docker compose up -d --build
```

Connect with SyncTERM or telnet:

```bash
telnet localhost 2323
```

Common Docker sysop commands:

```bash
docker compose run --rm oxidebbs status
docker compose run --rm oxidebbs nodes list
docker compose run --rm oxidebbs doors check oxide-check
docker compose run --rm oxidebbs doors test oxide-check --user sysop --dry-run
```

See [docs/project/docker.md](docs/project/docker.md) for first-boot variables,
volumes, reset steps, door testing, and Windows/macOS notes.

## Install From Releases

GitHub Releases publish `oxidebbs-server` packages and SHA-256 checksums for:

- `oxidebbs-<tag>-linux-x86_64-gnu.tar.gz`
- `oxidebbs-<tag>-macos-x86_64.tar.gz`
- `oxidebbs-<tag>-windows-x86_64-msvc.zip`

Each package includes the server binary, bundled assets, example configs, the
Oxide-owned `oxide-check` test door fixture, license files, and security policy.
Docker remains the simplest deployment path for Windows and macOS sysops who
want DOS door support because live door execution targets a Linux runtime with
DOSEMU2.

## Build From Source

Prerequisites:

- Rust stable with `rustfmt` and `clippy`
- `clang` and `libclang-dev` for DecentDB's native build integration
- DOSEMU2 for live DOS door execution
- Node.js for building the documentation site

On Debian/Ubuntu:

```bash
sudo apt-get install -y clang libclang-dev
```

Create a local board:

```bash
cargo run -p oxidebbs-server -- setup
```

Validate the configuration:

```bash
cargo run -p oxidebbs-server -- check
cargo run -p oxidebbs-server -- config check
```

Start the telnet listener:

```bash
cargo run -p oxidebbs-server -- serve
```

After `setup`, OxideBBS uses `config/oxidebbs.toml` by default. A clean checkout
without that file falls back to `config/oxidebbs.example.toml`.

## Sysop Operations

OxideBBS is local-admin-first: there is no remote web admin surface in the
current release line. Use the CLI, the local sysop TUI, and the local control
socket from the host running `oxidebbs-server`.

```bash
cargo run -p oxidebbs-server -- status
cargo run -p oxidebbs-server -- nodes list
cargo run -p oxidebbs-server -- users list
cargo run -p oxidebbs-server -- messages areas list
cargo run -p oxidebbs-server -- doors list
cargo run -p oxidebbs-server -- logs recent
cargo run -p oxidebbs-server -- db doctor
```

Launch the local sysop TUI:

```bash
cargo run -p oxidebbs-server -- sysop
```

The TUI can attach to a running server through
`runtime/oxidebbs-control.sock`; when no socket is available it can start an
embedded serve runtime for the TUI session. Selectable themes include
`oxide-classic`, `wildcat`, `telegard`, `vbbs`, `mystic`, `midnight`, and
`high-contrast`.

## Doors

OxideBBS isolates door execution from core session logic. A live DOS door
session uses this byte path:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOSEMU2-emulated COM1 UART
  <-> DOS door program
```

Validate the bundled `oxide-check` fixture without launching a live caller
session:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors check oxide-check
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors dropfile oxide-check --user sysop --node 1 --format DORINFO1.DEF
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors test oxide-check --user sysop --dry-run
```

OxideBBS does not bundle copyrighted or abandonware DOS doors. Add third-party
doors only when you have the rights to run and distribute them.

## Documentation

The documentation site is published at [oxidebbs.com](https://oxidebbs.com).
Source lives in [docs/](docs/).

```bash
npm ci
npm run docs:dev
npm run docs:build
```

Useful docs:

- [Getting Started](docs/project/getting-started.md)
- [Deployment and Operations](docs/project/deployment.md)
- [Docker Deployment](docs/project/docker.md)
- [Sysop CLI](docs/project/sysop-cli.md)
- [Caller Commands](docs/project/caller-commands.md)
- [Architecture](docs/project/architecture.md)
- [Changelog](docs/about/changelog.md)

## Repository Layout

```text
.
├── crates/
│   ├── oxidebbs-server/     # binary entrypoint, CLI, telnet server
│   ├── oxidebbs-core/       # users, sessions, nodes, permissions, menus
│   ├── oxidebbs-term/       # ANSI/CP437 rendering and terminal helpers
│   ├── oxidebbs-telnet/     # telnet transport and negotiation
│   ├── oxidebbs-db/         # DecentDB repository layer and schema helpers
│   ├── oxidebbs-door/       # door metadata, drop files, runners
│   ├── oxidebbs-sysop/      # local Ratatui sysop console and services
│   ├── oxidebbs-transfer/   # file-transfer protocols (ZMODEM, XMODEM-CRC)
│   ├── oxidebbs-network/    # shared network protocol types and tables
│   ├── oxidebbs-ftn/        # legacy FTN packet engine and tosser/scanner
│   ├── oxidebbs-binkp/      # BinkP client/server for FTN/OxideNet transport
│   └── oxidebbs-oxidenet/   # OxideNet first-party network profile
├── assets/                  # bundled ANSI, ASCII, text, and screen assets
├── config/                  # example board and door configuration
├── design/                  # specs, roadmap, ADRs, and architecture notes
├── docs/                    # VitePress documentation site
├── graphics/                # project logo and README graphics
├── scripts/                 # development and door validation scripts
└── tools/doors/             # Oxide-owned DOS test door fixture
```

## Development

The CI gate is:

```bash
./scripts/dev-check.sh
```

It runs:

```bash
cargo fmt --all --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Use `--locked` when validating Rust changes because `Cargo.lock` is committed.
Meaningful behavior, architecture, config, or product-scope changes should also
update the relevant design docs and ADRs.

## Current Boundaries

- Telnet is the supported caller transport today. Serial/modem config and test
  scaffolding exist for v1.2 work, but physical serial/modem serving is not yet
  release-ready.
- DecentDB is the only system database.
- Remote caller UI is ANSI/CP437 byte-oriented, not Unicode-first.
- Ratatui is used only for the local sysop console, not caller screens.
- Physical modem/serial support, BinkP polling for FTN/FidoNet mail exchange,
  full FTN/OxideNet runtime, caller file-area transfer support, remote admin,
  and other previously deferred features are tracked in the v1.2 release plan
  and are not all complete yet.
  See [`design/RELEASE_v1_2_PLAN.md`](design/RELEASE_v1_2_PLAN.md) for the
  full deferred-scope implementation map.
- Caller file-transfer design uses ZMODEM as the primary protocol with
  XMODEM-CRC as the fallback protocol. Repository, CLI, XMODEM-CRC, and ZMODEM
  framing foundations exist, but caller file-area workflows are still in
  progress.
- Public telnet exposure is an operator decision. Telnet sends credentials and
  caller traffic in plaintext; the generated config binds to `127.0.0.1:2323`
  by default.

## Contributing

Issues and pull requests are welcome. Start with
[CONTRIBUTING.md](CONTRIBUTING.md), keep the caller experience ANSI-first, and
run `./scripts/dev-check.sh` before submitting changes.

## Security

Report security issues privately to the repository owner. See
[SECURITY.md](SECURITY.md) for the current policy and priorities.

## License

OxideBBS is licensed under the Apache License, Version 2.0. See
[LICENSE](LICENSE) and [NOTICE](NOTICE). The repository licensing decision and
contribution policy are recorded in
[design/adr/0007-use-github-and-apache-2.md](design/adr/0007-use-github-and-apache-2.md).
