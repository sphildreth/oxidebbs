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
- `-v, --verbose` (`debug`; repeat for `trace`)

`--data` accepts either an explicit DecentDB file path or a directory path. When
it points at a directory, OxideBBS uses `oxidebbs.ddb` inside that directory.

JSON outputs are stable objects for `--json`:

- `status`
- `nodes list`
- `users list`
- `messages areas list`
- `doors list`
- `db stats`

User security levels are documented in
[User Security Levels](./security-levels.md), including the current defaults,
message-area gates, and the distinction between numeric level `255` and the
separate `is_sysop` flag.

## Local Sysop TUI

Launch the local Ratatui sysop console with:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml sysop
```

`sysop` opens the full TUI by default. It first tries to attach to the live
control socket at `paths.runtime/oxidebbs-control.sock`. If no live socket is
reachable, it starts an embedded `serve` runtime in the same process, waits for
that runtime's control socket, and stops the embedded runtime when the TUI exits.

The legacy `--tui` flag is accepted for compatibility, and `--readonly` starts
the console with destructive actions disabled:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml sysop --readonly
```

Use `--connect-only` when `sysop` should never start an embedded server:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml sysop --connect-only
```

The TUI uses the configured DecentDB path, `paths.logs`, `paths.screens`, and the
Unix control socket. Live node actions such as disconnect, node message,
broadcast, and reset-stale require either an existing `serve` process or the
embedded runtime started by `sysop`.

## Logging

The configured logging policy is:

```toml
[logging]
level = "info"
file_enabled = true
file_name = "oxidebbs-server.log"
format = "text"

[logging.rotation]
strategy = "daily"
max_size_mb = 50
max_files = 14
```

Accepted levels are `error`, `warn`, `info`, `debug`, and `trace`.
Accepted file formats are `text` and `json`.

When file logging is enabled, logs are appended under `paths.logs`. With the
default paths this is:

```text
logs/oxidebbs-server.log
```

Set `format = "json"` for newline-delimited JSON log files. JSON lines include
the formatter fields `timestamp`, `level`, and `target`, plus event fields such
as `message`, `node`, `remote`, `remote_ip`, `remote_port`, `session_id`,
`menu`, `key`, `user_id`, `alias`, `event_type`, `door_key`, `door_name`,
`area_key`, `message_id`, `reason`, and related outcome fields when they apply.

Rotation strategies are:

- `daily`: rotates when the running process crosses a UTC date boundary, keeping
  dated archives such as `oxidebbs-server.2026-06-03.log`.
- `size`: rotates before a write would exceed `max_size_mb`, keeping numbered
  archives such as `oxidebbs-server.log.1`.
- `never`: appends indefinitely.

`max_files` is the number of rotated archives to retain.

`serve --log-level <level>` overrides `[logging].level` for that serve run:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml serve --log-level debug
```

The effective precedence is `serve --log-level`, then global `-v`, then
`[logging].level`. One `-v` uses `debug`; two or more use `trace`.

At `debug` and `trace`, server logs include caller connections, session opens,
menu key selections, audit events, login outcomes, door activity, message
activity, and disconnect reasons. DecentDB audit events remain the durable
activity record; file logs are the operational troubleshooting stream.

Read logs with:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml logs recent
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml logs tail --follow
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml logs search login_success
```

The log commands read regular files under `paths.logs`, including nested door
runner logs under `logs/doors/`.

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
- configured terminal welcome/logoff assets
- configured screen paths/assets
- configured menu screen paths/assets with menu-specific error context
- door working directory + command + runner availability
- drop-file format (`DOOR.SYS` or `DORINFO1.DEF`)
- runtime directory writability
- Unix runtime directory mode (`0700` expected for local control socket isolation)

During live serving, missing or unreadable caller assets are logged and printed
to the `serve` console with the screen or asset name, terminal ANSI support, and
negotiated width before fallback text is sent.

Missing optional directories are surfaced as warnings; parse failures, missing
configured assets, and invalid bind/state values are errors.
Binding telnet beyond loopback also surfaces as a warning because telnet sends
credentials and caller traffic without encryption.

`database.path` accepts either an explicit DecentDB file path or a directory
path. Directory paths, including paths written with a trailing slash, resolve to
`oxidebbs.ddb` inside that directory.

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

Before binding telnet, `serve` opens DecentDB, verifies the schema marker, reads
the core user, auth, message, session, door, door-run, and audit tables, and
writes required `config_loaded` and `server_start` audit events. Startup exits
with a blocking error if any of those reads or required audit writes fail.
`serve --dry-run` runs the same database startup health check after config
validation succeeds.

When `runtime/oxidebbs-control.sock` already exists and is actively bound by a
running process, startup exits with a clear error instead of entering offline
mode.

## Local control socket behavior (Unix)

The local control plane is:

- Local-only (`runtime/oxidebbs-control.sock`)
- Newline-delimited JSON request/response per connection
- Single command per connection
- Used by `status`, `nodes list`, `nodes show`, `nodes watch`,
  `nodes disconnect`, `nodes message`, `nodes broadcast`, and `nodes reset-stale`
- Protected on Unix by `0700` runtime-directory permissions, `0600` socket
  permissions, and peer-UID checks against the server process UID

Run sysop commands as the same OS user as the running server process:

```bash
sudo -u oxidebbs oxidebbs-server nodes list
```

Running control commands as a different local user (including root) typically
fails before request handling with a peer-UID mismatch error.

The runtime directory and any pre-existing `runtime/node-NNN` directories must
be writable by the server UID. When changing the service user or restoring
runtime files from backup, stop the server and remove stale `runtime/node-*`
directories so OxideBBS can recreate them with mode `0700`.

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

Live `status --json` also includes `audit_write_failures`, the in-memory count
of best-effort audit writes that failed while the server was running.

Any future web admin interface must add CSRF and replay protection before it is
enabled; the current admin/control surface remains local CLI plus Unix control
socket.

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
- The bundled test door is `oxide-check` (`OXIDECHK.EXE`) for validating the
  DOSEMU2 serial runtime bridge.
- Enabled configured doors are the only ones selectable by live caller menu.
- Live launch writes drop files in the node runtime directory, tracks
  `door_started`/`door_finished`/`door_timed_out` events, and returns the caller
  to the menu on completion or timeout.

Recommended smoke-test flow:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors check oxide-check
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors dropfile oxide-check --user sysop --node 1 --format DORINFO1.DEF
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors test oxide-check --user sysop --dry-run
```

Live test expectation:

- DOSEMU2 receives run-local `OXDOSEMU2.CONF` containing `$_com1 =
  "pts <runtime>/node-001/OXCOM1.PTY"`.
- DOSEMU2 receives container-safe runtime settings in the generated config.
- The door believes it is reading and writing `COM1`; it is not reading from
  DOSEMU2 console stdio.
- OxideBBS receives caller telnet bytes and forwards them to the PTY bridge. DOSEMU2
  converts those bridge bytes into COM1 UART input for the door.
- Door output follows the reverse path: COM1 output goes through the DOSEMU2 PTY,
  OxideBBS reads it from the bridge, and writes to the caller's telnet
  connection.
- On a clean run, `OXNODE.TXT` and `OXIDECHK.RPT` should be written to the node
  runtime directory and include matching node metadata.

Byte path:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOSEMU2-emulated COM1 UART
  <-> DOS door program
```

Live execution requires DOSEMU2 and the PTY bridge; it should return a clear
missing-runner or bridge-start error when unavailable.

If caller sessions should exit a running door on system restart, use `nodes
disconnect <n>` which closes an active bridge before normal disconnect cleanup.

Optional DOSEMU2 smoke script:

```bash
OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
```

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
  - requires a schema-4, schema-only target
  - validates schema and all foreign-key references before writing
  - preserves UUIDs and load ordering
  - executes in one transaction and fails atomically
- `db compact` currently returns a hard unsupported error because DecentDB has no
  safe compaction API contract in this release.
- `[audit].retention_days` defaults to `365`. Runtime audit inserts do not
  purge old rows automatically; the DecentDB repository exposes a retention
  purge helper for scheduled maintenance until a CLI wrapper is added.

## Schema migration notes

- Schema version is currently `4`.
- Existing schema `2` and `3` databases migrate automatically to `4` on first
  open.
- Databases with missing, malformed, or future markers are rejected with explicit
  operator-facing errors.
- `status`/`nodes` do not attempt to operate on incompatible databases.

## Local-only boundary

All operations here are local to the current machine:

- no remote TCP admin interface
- no remote secret/token auth model
- control socket path must be local filesystem access only
- local control socket uses Unix peer UID checks plus filesystem permissions
- any future web admin interface must include CSRF and replay protection before
  being enabled
