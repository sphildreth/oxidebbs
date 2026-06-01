# Getting Started

OxideBBS is a local Rust BBS server with:

- Telnet caller runtime
- ANSI/CP437 rendering
- DecentDB persistence
- DOS door launch support
- CLI-first sysop operations

## Prerequisites

- Rust stable with `rustfmt` and `clippy`
- Node.js 20 or newer (documentation site only)
- Native DecentDB headers:

```bash
sudo apt-get install -y clang libclang-dev
```

From repository root, the normal validation command is:

```bash
./scripts/dev-check.sh
```

## 1) Create a board

```bash
cargo run -p oxidebbs-server -- setup
```

`setup` creates `config/oxidebbs.toml`, initializes `data/oxidebbs.ddb`,
creates the initial sysop account, and prepares runtime/asset directories.

For unattended setup, pass required values:

```bash
cargo run -p oxidebbs-server -- setup \
  --board-name "My BBS" \
  --sysop-alias sysop \
  --sysop-password "change-this" \
  --nodes 4
```

## 2) Validate config and runtime paths

```bash
cargo run -p oxidebbs-server -- check
cargo run -p oxidebbs-server -- config check
```

Validation checks:

- socket address parsing
- config paths and screen assets
- door definitions and runner availability
- drop-file format and timeout constraints
- runtime directory writability

`check` errors on missing/invalid configuration and reports warnings for optional
but missing directories or assets.

## 3) Start serving

```bash
cargo run -p oxidebbs-server -- serve
```

`serve` binds telnet, accepts caller sessions, persists session/audit rows, and
starts the local Unix control socket:

```text
runtime/oxidebbs-control.sock
```

If that socket path is already active, startup fails to avoid clobbering an
already-running server.

## 4) Confirm runtime

```bash
cargo run -p oxidebbs-server -- status
cargo run -p oxidebbs-server -- nodes list
cargo run -p oxidebbs-server -- nodes watch
```

While running, node status comes from live runtime registry and includes heartbeat
age. If the socket is unreachable, status/list/watch read through active session
rows from DecentDB.

## 5) Use local sysop controls

```bash
cargo run -p oxidebbs-server -- nodes message 1 "Maintenance in 10 minutes."
cargo run -p oxidebbs-server -- nodes disconnect 1
cargo run -p oxidebbs-server -- nodes broadcast "Server restart at 00:00 UTC."
cargo run -p oxidebbs-server -- nodes reset-stale
```

When the control socket is unavailable, these commands still record explicit
sysop intent in audit rows and return explicit messaging explaining the delivery
gap.

## 6) Doors and data safety checks

```bash
cargo run -p oxidebbs-server -- doors check example
cargo run -p oxidebbs-server -- doors test example --user sysop --dry-run
```

Caller `Doors` menu launch uses live door execution in the caller path and records
`door_runs` rows with timeout and byte counters.

## 7) Backup, export, and restore

```bash
cargo run -p oxidebbs-server -- db backup backups/oxidebbs.ddb
cargo run -p oxidebbs-server -- db export --format json > backups/oxidebbs.json
cargo run -p oxidebbs-server -- db import --format json backups/oxidebbs.json
```

`db import --format json` is a full restore into schema-only targets only. It is
transactional and validates IDs, relationships, and schema compatibility.

`db compact` is explicitly unsupported in this release because DecentDB has no
safe compaction API contract.

## 8) Local-only boundary

There is no remote web or TCP admin interface in this phase. All operational
control is local to the host running `oxidebbs-server`.
