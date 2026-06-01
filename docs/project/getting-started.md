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
password hash, and read/post local messages; door launching still responds with
a placeholder while the runner is wired into live sessions.

The pre-alpha schema is versioned. If you have an older development database at
`data/oxidebbs.ddb`, recreate it after schema changes; the server refuses to run
against a stale schema marker rather than silently using incompatible tables.

## Local Admin Commands

The starter server binary includes a local `admin` command group:

```bash
cargo run -p oxidebbs-server -- admin users
cargo run -p oxidebbs-server -- admin nodes
cargo run -p oxidebbs-server -- admin recent-calls
```

Password resets accept a new password hash, not a plaintext password.

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
