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
