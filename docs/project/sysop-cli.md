# Sysop CLI

OxideBBS is sysop-command first. The same `oxidebbs-server` binary starts the
BBS, checks configuration, manages users, watches nodes, administers doors and
files, and runs network operations.

Examples:

```bash
oxidebbs-server status
oxidebbs-server nodes list
oxidebbs-server doors test lord --user sysop --dry-run
oxidebbs-server db export --format json
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
- `files areas list`
- `files list`
- `files transfers recent`
- `net status`
- `net links list`
- `net links show`
- `net areas list`
- `net queue`
- `net packets inbound`
- `net packets outbound`
- `net packets quarantine`
- `net logs`
- `net poll --dry-run`
- `net nodelist list`
- `net nodelist lookup`
- `db stats`

User security levels are documented in
[User Security Levels](./security-levels.md), including the current defaults,
message-area gates, and the distinction between numeric level `255` and the
separate `is_sysop` flag.

## Local Sysop TUI

Launch the local Ratatui sysop console with:

```bash
oxidebbs-server --config config/oxidebbs.toml sysop
```

`sysop` opens the full TUI by default. It first tries to attach to the live
control socket at `paths.runtime/oxidebbs-control.sock`. If no live socket is
reachable, it starts an embedded `serve` runtime in the same process, waits for
that runtime's control socket, and stops the embedded runtime when the TUI exits.

The legacy `--tui` flag is accepted for compatibility, and `--readonly` starts
the console with destructive actions disabled:

```bash
oxidebbs-server --config config/oxidebbs.toml sysop --readonly
```

By default, pressing `Q` in the TUI opens a confirmation dialog before quitting.
This helps prevent accidentally stopping the embedded `serve` runtime that
`sysop` may have started. The behavior is controlled by:

```toml
[sysop]
confirm_quit = true
```

To skip the routine confirmation for one launch when no callers are online, use:

```bash
oxidebbs-server --config config/oxidebbs.toml sysop --no-confirm-quit
```

If any nodes are active, the TUI always shows a stronger confirmation dialog
with `Nodes are active. Continue to shutdown?` before it exits, even when
`sysop.confirm_quit = false` or `--no-confirm-quit` is set.

Use `--connect-only` when `sysop` should never start an embedded server:

```bash
oxidebbs-server --config config/oxidebbs.toml sysop --connect-only
```

Select a TUI color theme with `--theme`:

```bash
oxidebbs-server --config config/oxidebbs.toml sysop --theme telegard
```

Available themes:

- `oxide-classic`
- `wildcat`
- `telegard`
- `vbbs`
- `mystic`
- `midnight`
- `high-contrast`

See [Sysop TUI Themes](./sysop-tui-themes.md) for theme descriptions, command
examples, and palette swatches.

The TUI uses the configured DecentDB path, `paths.logs`, `paths.screens`, and the
Unix control socket. Live node actions such as disconnect, node message,
broadcast, and reset-stale require either an existing `serve` process or the
embedded runtime started by `sysop`.

While the TUI is active, OxideBBS keeps console logging off for that process so
server log lines do not overwrite the terminal UI. Runtime logs still go to the
configured file under `paths.logs` when `[logging].file_enabled` is true.

### Doctor Screen

The sysop TUI includes a `Doctor` menu item for verbose local health checks.
Open it from the left navigation rail or the command palette (`F2`, then
`Go to Doctor`).

Doctor runs automatically when the screen opens. Press `R` or `F5` to rerun the
checks, and use `Up`/`Down` or `PageUp`/`PageDown` to scroll the report. The
summary shows total checks, passed checks, warnings, failures, the run timestamp,
and the database path.

Each check is rendered as `[PASS]`, `[WARN]`, or `[FAIL]` with a detailed result.
Warnings and failures include a `Fix:` line with the next sysop action to take.
The current TUI doctor checks:

- configured database path, database file presence, parent directory, and
  directory write permissions
- open DecentDB connection and current schema version
- required table readability and row counts for users, auth attempts, audit
  events, message areas, messages, sessions, doors, and door runs
- user repository readability, sysop-account presence, aliases, and security
  levels
- message-area availability, message visibility values, and loaded message
  references
- active and recent sessions, node-number validity, configured node range, and
  duplicate active node assignments
- door definitions, enabled doors, required door fields, door time-limit policy,
  sampled unfinished door runs, and sampled door-run references

### OxideNet Screen

Open OxideNet with `Ctrl+O`, the left navigation rail, or the command palette.
The screen includes tabs for dashboard, applications, nodes, packet queues,
subscriptions, poll logs, nodelist generation, and config-package operations.

Mutating actions use confirmation dialogs and the same OxideNet service layer as
`oxidebbs-server net oxidenet ...`. Read-only mode hides or blocks mutations
while preserving refresh, navigation, and export operations.

### Utility Workflows

The Database screen supports verify, backup, and summary export. Config supports
reload, external editor launch, and dotted `config set` style updates. ANSI
supports raw byte inspection, external editor launch, and default screen
installation. Logs and Audit support TSV export of the loaded or filtered range.
- recent audit events and auth-attempt lockout state

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
oxidebbs-server --config config/oxidebbs.toml serve --log-level debug
```

The effective precedence is `serve --log-level`, then global `-v`, then
`[logging].level`. One `-v` uses `debug`; two or more use `trace`.

At `debug` and `trace`, server logs include caller connections, session opens,
menu key selections, audit events, login outcomes, door activity, message
activity, and disconnect reasons. DecentDB audit events remain the durable
activity record; file logs are the operational troubleshooting stream.

Read logs with:

```bash
oxidebbs-server --config config/oxidebbs.toml logs recent
oxidebbs-server --config config/oxidebbs.toml logs tail --follow
oxidebbs-server --config config/oxidebbs.toml logs search login_success
```

The log commands read regular files under `paths.logs`, including nested door
runner logs under `logs/doors/`.

## Setup flow

Start with:

```bash
oxidebbs-server setup
```

Important setup behavior:

- Writes default config and creates `runtime/`, `assets/`, `doors/`, and `data/`
  directories if needed.
- Creates starter `data/oxidebbs.ddb`.
- Creates default sysop and default local message area.
- Requires `--sysop-password` for unattended setup.

After setup:

```bash
oxidebbs-server --config config/oxidebbs.toml check
oxidebbs-server --config config/oxidebbs.toml config paths
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
- drop-file format (`DOOR.SYS`, `DORINFO1.DEF`, `CHAIN.TXT`, `DOORFILE.SR`,
  `PCBOARD.SYS`, or `CALLINFO.BBS`)
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
oxidebbs-server serve
```

or:

```bash
oxidebbs-server --config config/oxidebbs.toml serve
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
oxidebbs-server nodes list
oxidebbs-server nodes show 1
oxidebbs-server nodes watch
oxidebbs-server nodes disconnect 1
oxidebbs-server nodes message 1 "System will restart in 1 minute."
oxidebbs-server nodes broadcast "Welcome to the night shift."
oxidebbs-server nodes reset-stale
```

When live, node rows include states:

- `available`, `connecting`, `login`, `main_menu`, `reading_messages`,
  `posting_message`, `in_door`, `disconnecting`, `offline`, `stale`

Each live row may include heartbeat age in seconds.

Live `status --json` also includes `audit_write_failures`, the in-memory count
of best-effort audit writes that failed while the server was running.

The `[admin_web]` monitoring surface is disabled by default and validated. When
explicitly enabled, it can serve `/status`, `/health`, `/terminal`, and
sysop-authenticated read-only JSON views on loopback or trusted LAN interfaces.
`GET /` returns a small static route index. `GET /health`, `/healthz`, and
`/healtz` run doctor-backed monitoring checks and return HTTP `200` only when
doctor has no failed checks. OxideBBS does not serve HTTPS/TLS for this
listener; put it behind a local reverse proxy such as Caddy when HTTPS is
required for WAN access. Remote mutation attempts require CSRF and
nonce/timestamp replay checks, are audited, and remain blocked by
`admin_web.read_only`; the mutating sysop/control surface remains local CLI plus
Unix control socket.

## Messages

```bash
oxidebbs-server messages search "packet"
oxidebbs-server messages search "retro" --area retro.echo
oxidebbs-server messages search "1:105/42" --network fidonet
```

`messages search` matches subject, body, author display name, area key, remote
author address, network message id, and area network id. `--area`, `--user`,
`--network`, `--limit`, and global `--json` narrow or format the result set.

## Doors and caller launch

Door management:

- `oxidebbs-server doors list`
- `oxidebbs-server doors check` (or `doors check <key>`)
- `oxidebbs-server doors test <key> --user sysop --dry-run`
- `oxidebbs-server doors dropfile <key> --user sysop --node 1 --format DORINFO1.DEF`

Meaning:

- `--dry-run` generates drop files and validates input without launching a child.
- `doors dropfile --format` supports `DOOR.SYS`, `DORINFO1.DEF`, `CHAIN.TXT`,
  `DOORFILE.SR`, `PCBOARD.SYS`, and `CALLINFO.BBS`.
- Live interactive DOS door testing requires a caller session. Start `serve`,
  connect over telnet, and launch the door from the caller `Doors` menu.
- The bundled test door is `oxide-check` (`OXIDECHK.EXE`) for validating the
  DOSEMU2 serial runtime bridge.
- Enabled configured doors are the only ones selectable by live caller menu.
- Live launch writes drop files in the node runtime directory, tracks
  `door_started`/`door_finished`/`door_timed_out` events, and returns the caller
  to the menu on completion or timeout.
- `doors add` and `doors edit` currently persist local DOS door definitions.
- `doors add` and `doors edit` also accept remote provider definitions with
  `--provider bbslink` or `--provider doorparty`, `--endpoint <host:port>`, and
  `--credential-ref <secret-ref>`. The stored credential reference is redacted
  in CLI output and audit details.

Remote provider example:

```bash
oxidebbs-server doors add bbslink-lord "BBSLink LORD" \
  --provider bbslink \
  --endpoint telnet://bbslink.example:23 \
  --credential-ref env:BBSLINK_AUTH_CODE \
  . LORD
```

The `.` argument satisfies the legacy local working-directory positional; for a
remote provider door, `--endpoint` is the endpoint stored on the door record.

Credential references must use `env:`, `file:`, `vault://`, `keyring://`,
`op://`, or `secret://`. Do not pass raw BBSLink or DoorParty secrets to door
commands.

Recommended smoke-test flow:

```bash
oxidebbs-server --config config/oxidebbs.example.toml doors check oxide-check
oxidebbs-server --config config/oxidebbs.example.toml doors dropfile oxide-check --user sysop --node 1 --format DORINFO1.DEF
oxidebbs-server --config config/oxidebbs.example.toml doors test oxide-check --user sysop --dry-run
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

## File Areas and Transfers

File-area administration is available from both the local sysop TUI and CLI.
The TUI Files screen shows areas, entries, and transfer history, and can
enable/disable areas or approve/unapprove entries with confirmation and audit
logging. Callers can use the configured `files` menu action for live ZMODEM or
negotiated XMODEM uploads and downloads when `[file_transfers].enabled = true`.

Area management:

```bash
oxidebbs-server files areas list
oxidebbs-server files areas add main --name "Main Files" --root files/main --read-level 0 --download-level 10 --upload-level 20
oxidebbs-server files areas edit main --name "General Files" --enabled true
```

File entries:

```bash
oxidebbs-server files list
oxidebbs-server files list --area main
oxidebbs-server files import main ./uploads/demo.zip --description "Demo archive"
oxidebbs-server files remove <file-id> --reason "duplicate upload"
```

Transfer history:

```bash
oxidebbs-server files transfers recent --limit 50
```

Operational notes:

- `files areas add` and `files areas edit` validate security levels in the
  `0..=255` range.
- `files import` copies the source file into the area's root directory with a
  generated storage name. The source file is preserved.
- `files remove` is a safe removal: it marks the file entry unapproved and
  requires `--reason`; it does not delete the stored file bytes.
- `--json` returns stable top-level objects for list and mutation responses.
- File-area mutations, imports, and removals write audit events.
- The TUI Files screen reuses the same DecentDB repository state and preserves
  read-only mode by blocking area and entry mutations.

## Database operations

```bash
oxidebbs-server db backup backups/oxidebbs.ddb
oxidebbs-server db export --format json > backups/oxidebbs.json
oxidebbs-server db import --format json backups/oxidebbs.json
oxidebbs-server db compact --output backups/oxidebbs-compacted.ddb
oxidebbs-server audit purge-retention --dry-run
```

- `db backup` copies the active database file.
- `db export --format json` is read-only and safe.
- `db import --format json <path>` performs a full restore only:
  - requires a schema-8, schema-only target
  - validates schema and all foreign-key references before writing
  - preserves UUIDs and load ordering
  - executes in one transaction and fails atomically
- `db compact --output <path> [--overwrite]` writes and verifies a separate
  compacted DecentDB file. It refuses the active database path; stop the server
  before manually replacing the active database with the compacted output.
- `[audit].retention_days` defaults to `365`. Runtime audit inserts do not
  purge old rows automatically; use `audit purge-retention` for scheduled
  maintenance, or `audit purge-before <timestamp>` for an explicit cutoff.

## Schema migration notes

- Schema version is currently `8`.
- Existing schema `2` through `7` databases migrate automatically to `8` on
  first open.
- Databases with missing, malformed, or future markers are rejected with explicit
  operator-facing errors.
- `status`/`nodes` do not attempt to operate on incompatible databases.

## FTN network commands

Implemented `net` commands currently cover status/list/log views, queue and
packet inspection, subscription metadata updates, poll dry-run preflight, and
full nodelist import/lookup:

```bash
oxidebbs-server net status fidonet
oxidebbs-server net toss fidonet
oxidebbs-server net scan fidonet
oxidebbs-server net links list --network fidonet
oxidebbs-server net links show fidonet-hub
oxidebbs-server net areas list --network fidonet
oxidebbs-server net areas subscribe TEST.ECHO fidonet-hub --network fidonet
oxidebbs-server net queue fidonet-hub
oxidebbs-server net packets summary --network fidonet
oxidebbs-server net packets show <packet-id>
oxidebbs-server net packets retry <packet-id>
oxidebbs-server net packets mark-quarantined <packet-id> --reason "operator review"
oxidebbs-server net packets outbound --network fidonet
oxidebbs-server net logs fidonet-hub --limit 25
oxidebbs-server net poll fidonet-hub
oxidebbs-server net poll fidonet-hub --dry-run
oxidebbs-server net areafix send fidonet-hub "+FSX_GEN" --password <password> --network fidonet
oxidebbs-server net nodelist import NODELIST.123 --network fidonet
oxidebbs-server net nodelist lookup 1:105/42 --network fidonet
```

`net toss` imports raw `.pkt` files and safe ZIP packet bundles from
`paths.runtime/network/<profile>/inbound/drop`, archives accepted inputs, and
quarantines malformed, unauthorized, unknown-area, or netmail inputs. `net scan`
writes outbound Type-2+ packets for subscribed echomail links under
`paths.runtime/network/<profile>/outbound/<link>/ready`. `net poll` transports
ready files over BinkP links, uses TLS according to each link security policy,
receives remote files into the inbound drop directory, and logs the poll.
Packet retry/quarantine commands update DecentDB packet state only;
they do not move spool files.
`net areafix send` authenticates command text against the selected link
password, applies AreaFix subscription changes, audits the activity, and prints
the reply text. It also queues the reply as outbound netmail and creates pending
rescan rows for `+AREA !`.

See [FTN CLI](../ftn/cli.md), [FTN Tosser](../ftn/tosser.md),
[FTN Scanner](../ftn/scanner.md), and [FTN Nodelists](../ftn/nodelist.md) for
the current scope and limitations.

## Local And Remote Boundaries

The CLI and TUI remain the full sysop-control surfaces:

- local CLI commands run on the host where `oxidebbs-server` can read the config
  and database paths
- the local control socket uses Unix peer UID checks plus filesystem permissions
- remote monitoring is disabled by default
- enabled remote monitoring may bind to loopback or trusted LAN interfaces
- authenticated remote API views are read-only
- WAN/public HTTPS should terminate at a local reverse proxy before forwarding
  plain HTTP to OxideBBS
- remote mutation attempts are guarded by CSRF and replay protection but remain
  blocked by read-only mode
