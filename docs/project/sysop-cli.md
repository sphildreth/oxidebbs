# Sysop CLI

OxideBBS is CLI-first. The server process is controlled with local commands in
`oxidebbs-server`, not a remote admin API.

Examples:

```bash
cargo run -p oxidebbs-server -- status
cargo run -p oxidebbs-server -- nodes list
cargo run -p oxidebbs-server -- doors test lord --user sysop --dry-run
cargo run -p oxidebbs-server -- db export --format json
```

Global options:

- `-c, --config <PATH>`
- `--data <PATH>`
- `--json`
- `--no-color`
- `-v, --verbose`

JSON outputs are stable objects for `--json`:

- `status`
- `users list`
- `nodes list`
- `messages areas list`
- `doors list`
- `db stats`

## Setup flow

Start with:

```bash
cargo run -p oxidebbs-server -- setup
```

Important setup behavior:

- Writes default config and creates `runtime/`, `assets/`, `doors/`, and `data/`
  directories if needed.
- Creates starter `data/oxidebbs.ddb`.
- Creates default sysop and default local message area.
- Requires `--sysop-password` for unattended setup.

After setup:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml check
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml config paths
```

## Config validation

Use:

- `oxidebbs-server check` for a full file-level configuration sanity pass.
- `oxidebbs-server config check` for the same check through the config command group.

The check validates:

- config file existence/parsing
- `telnet.bind`
- node count
- configured screen paths/assets
- door working directory + command + runner availability
- drop-file format (`DOOR.SYS` or `DORINFO1.DEF`)
- runtime directory writability

Missing optional directories and assets are surfaced as warnings; parse failures and
invalid bind/state values are errors.

## Starting `serve`

```bash
cargo run -p oxidebbs-server -- serve
```

or:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml serve
```

`serve` starts telnet on the configured bind address and opens:

- DecentDB
- terminal assets
- user/session loops
- door execution bridge
- local control socket listener at `runtime/oxidebbs-control.sock` (Unix only)

When `runtime/oxidebbs-control.sock` already exists and is actively bound by a
running process, startup exits with a clear error instead of entering offline mode.

## Local control socket behavior (Unix)

The local control plane is:

- Local-only (`runtime/oxidebbs-control.sock`)
- Newline-delimited JSON request/response per connection
- Single command per connection
- Used by `status`, `nodes list`, `nodes show`, `nodes watch`,
  `nodes disconnect`, `nodes message`, `nodes broadcast`, and `nodes reset-stale`

If the socket is unreachable:

- `status`, `nodes list`, and `nodes show` fall back to persisted session rows.
- `nodes disconnect`, `nodes message`, `nodes broadcast`, and
  `nodes reset-stale` record sysop intent in audit events and return an explicit
  "live server not reachable" message.

Socket text fields are sanitized for control transport (newlines are normalized to
spaces).

## Node monitoring and control

```bash
cargo run -p oxidebbs-server -- nodes list
cargo run -p oxidebbs-server -- nodes show 1
cargo run -p oxidebbs-server -- nodes watch
cargo run -p oxidebbs-server -- nodes disconnect 1
cargo run -p oxidebbs-server -- nodes message 1 "System will restart in 1 minute."
cargo run -p oxidebbs-server -- nodes broadcast "Welcome to the night shift."
cargo run -p oxidebbs-server -- nodes reset-stale
```

When live, node rows include states:

- `available`, `connecting`, `login`, `main_menu`, `reading_messages`,
  `posting_message`, `in_door`, `disconnecting`, `offline`, `stale`

Each live row may include heartbeat age in seconds.

## Doors and caller launch

Door management:

- `oxidebbs-server doors list`
- `oxidebbs-server doors check` (or `doors check <key>`)
- `oxidebbs-server doors test <key> --user sysop --dry-run`
- `oxidebbs-server doors dropfile <key> --user sysop --node 1 --format DORINFO1.DEF`

Meaning:

- `--dry-run` generates drop files and validates input without launching a child.
- Live interactive DOS door testing requires a caller session. Start `serve`,
  connect over telnet, and launch the door from the caller `Doors` menu.
- The bundled test door is `oxide-check` (`OXIDECHK.EXE`) for validating the DOSBox
  path and door runtime contract.
- Enabled configured doors are the only ones selectable by live caller menu.
- Live launch writes drop files in the node runtime directory, tracks `door_runs`,
  records `door_started`/`door_finished`/`door_timed_out` events, and returns
  the caller to menu on completion or timeout.

Recommended smoke-test flow:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors check oxide-check
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors dropfile oxide-check --user sysop --node 1 --format DORINFO1.DEF
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors test oxide-check --user sysop --dry-run
```

Live test expectation:

- DOSBox receives a run-local config with
  `serial1=nullmodem server:127.0.0.1 port:<bridge_port> transparent:1 rxdelay:1000 txdelay:10`.
- DOSBox also receives quiet runtime settings:
  `startup_verbosity=quiet`, `waitonerror=false`, `pause_when_inactive=false`,
  and `mute_when_inactive=true`.
- The door believes it is reading and writing `COM1`; it is not reading from
  DOSBox console stdin/stdout.
- OxideBBS receives caller telnet bytes and forwards them to the run-local TCP
  bridge. DOSBox converts those bridge bytes into COM1 input for the door.
- Door output follows the reverse path: COM1 output becomes DOSBox nullmodem TCP
  bytes, OxideBBS reads them from the bridge, and OxideBBS writes them to the
  caller's telnet connection.
- On a clean run, `OXNODE.TXT` and `OXIDECHK.RPT` should be written to the node
  runtime directory and include matching node metadata.

Byte path:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> run-local 127.0.0.1 TCP bridge
  <-> DOSBox nullmodem serial backend
  <-> DOSBox-emulated COM1 UART
  <-> DOS door program
```

- `doors dropfile ...` and `doors test ... --dry-run` generate `DORINFO1.DEF`,
  `DOOR.SYS` when requested by command, and the Oxide diagnostic `OXNODE.TXT`
  beside the drop files.
- Live execution requires DOSBox and the serial bridge; it should return a clear missing-runner
  or bridge-start error when either component is unavailable.
- To run DOSBox without a visible SDL window, install Xvfb and configure the
  door runner as an absolute path to `scripts/run-dosbox-headless.sh`, or put
  that wrapper on `PATH`.

`nodes disconnect <n>` also closes an active door bridge for that node before
normal disconnect cleanup.

## Database operations

```bash
cargo run -p oxidebbs-server -- db backup backups/oxidebbs.ddb
cargo run -p oxidebbs-server -- db export --format json > backups/oxidebbs.json
cargo run -p oxidebbs-server -- db import --format json backups/oxidebbs.json
cargo run -p oxidebbs-server -- db compact
```

- `db backup` copies the active database file.
- `db export --format json` is read-only and safe.
- `db import --format json <path>` performs a full restore only:
  - requires a schema-3, schema-only target
  - validates schema and all foreign-key references before writing
  - preserves UUIDs and load ordering
  - executes in one transaction and fails atomically
- `db compact` currently returns a hard unsupported error because DecentDB has no
  safe compaction API contract in this release.

## Schema migration notes

- Schema version is currently `3`.
- Existing schema `2` databases migrate automatically to `3` on first open.
- Databases with missing, malformed, or future markers are rejected with explicit
  operator-facing errors.
- `status`/`nodes` do not attempt to operate on incompatible databases.

## Local-only boundary

All operations here are local to the current machine:

- no remote TCP admin interface
- no remote secret/token auth model
- control socket path must be local filesystem access only
