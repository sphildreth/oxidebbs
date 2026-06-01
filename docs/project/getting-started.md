# Getting Started

OxideBBS is currently a starter Rust workspace. The first useful milestone is a
telnet caller who can connect, create or log into an account, view ANSI menus,
read and post local messages, launch one configured DOS door, and disconnect
cleanly.

## Requirements

- Rust stable with `rustfmt` and `clippy`
- Node.js 20 or newer for the documentation site
- Native build dependencies needed by the pinned DecentDB Rust dependency

On Debian or Ubuntu CI images, the docs workflow installs:

```bash
sudo apt-get install -y clang libclang-dev
```

## Rust Checks

```bash
./scripts/dev-check.sh
```

That script runs formatting, workspace checks, tests, and clippy with the
committed lockfile.

## Create a Board Config

Use the setup wizard to create a local board config:

```bash
cargo run -p oxidebbs-server -- setup
```

The default output is `config/oxidebbs.toml`. See the [Setup Wizard](./setup)
guide for the prompts and generated paths.

## Run the Telnet Server

Start the listener:

```bash
cargo run -p oxidebbs-server -- serve
```

After `setup`, the server defaults to `config/oxidebbs.toml`. In a clean
checkout without a local config, it falls back to `config/oxidebbs.example.toml`.
Pass `--config <path>` to force a specific file.

The first server runtime accepts telnet callers, assigns node slots, records
session/audit rows in DecentDB, renders configured login and main menu screens,
and routes menu hotkeys. Callers can create an account, log in with an Argon2id
password hash, read/post local messages, and launch enabled configured doors.
Live door sessions bridge caller bytes to the configured runner, record
`door_runs`, enforce timeouts, and return normal exits or timeouts to the main
menu.

The pre-alpha schema is versioned. Supported older development databases at
`data/oxidebbs.ddb` are migrated before startup; unsupported stale, missing, or
future schema markers are refused rather than silently using incompatible
tables.

## Local Admin Commands

The starter server binary now exposes CLI-first sysop command groups:

```bash
cargo run -p oxidebbs-server -- users list
cargo run -p oxidebbs-server -- nodes list
cargo run -p oxidebbs-server -- audit recent
cargo run -p oxidebbs-server -- db doctor
```

Use `--json` on commands that support machine-readable output. See the
[Sysop CLI](./sysop-cli) guide for the full command surface and current
live-control limits.

## Documentation Site

```bash
npm ci
npm run docs:dev
```

The VitePress development server serves the `docs/` source directory locally.
Production builds are generated in `docs/.vitepress/dist`.

## Current Layout

- `crates/oxidebbs-server`: daemon entrypoint
- `crates/oxidebbs-core`: domain, sessions, menus, users
- `crates/oxidebbs-term`: ANSI/CP437 helpers
- `crates/oxidebbs-telnet`: telnet transport
- `crates/oxidebbs-db`: DecentDB repository layer
- `crates/oxidebbs-door`: drop files and door runners
- `crates/oxidebbs-sysop`: local sysop tooling
