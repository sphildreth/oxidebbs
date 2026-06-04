# OxideBBS Runbook

## Local development startup

```bash
cargo run -p oxidebbs-server -- serve
```

## Docker operator flow

Docker Compose is the preferred cross-platform evaluation/deployment path when
the host is Windows, macOS, or any system where the sysop does not want to
install Rust, DecentDB build headers, or DOSEMU2 natively:

```bash
OXIDEBBS_SYSOP_PASSWORD='choose-a-real-password' docker compose up -d --build
docker compose run --rm oxidebbs status
docker compose run --rm oxidebbs doors check oxide-check
```

The container uses Docker named volumes for config, data, doors, logs, and
runtime paths. Do not bind-mount runtime or data directories from Windows/macOS
for normal operation; the DOSEMU2 PTY bridge and DecentDB storage should remain
on Linux filesystems.

Reset a Docker board with:

```bash
docker compose down -v
```

## Operator flow

1. bootstrap once:

```bash
cargo run -p oxidebbs-server -- setup
cargo run -p oxidebbs-server -- check
```

2. start service:

```bash
cargo run -p oxidebbs-server -- serve
```

3. confirm runtime:

```bash
cargo run -p oxidebbs-server -- status
cargo run -p oxidebbs-server -- nodes list
```

`oxidebbs-server setup` and `db init` create schema `8`. When an existing
DecentDB uses supported older schema versions `2` through `7`, startup runs the
upgrade before serving callers. The migration chain preserves local users,
messages, replies, sessions, doors, runs, audit rows, shared network rows, file
areas, and OxideNet registry rows while adding the intervening schema
foundations. Renamed pre-upgrade tables remain as `oxidebbs_schema*_*` archives
and are not used by runtime queries. Databases with a future marker,
missing marker, or unmarked existing tables are refused with a clear error and
should be opened only with compatible OxideBBS software.

The CLI uses `config/oxidebbs.toml` when it exists, otherwise it falls back to
`config/oxidebbs.example.toml`. Pass `--config <path>` to force a file.

Running `serve` starts a local control listener on
`runtime/oxidebbs-control.sock` for sysop operations like:

- `oxidebbs-server status`
- `oxidebbs-server nodes list`
- `oxidebbs-server nodes show`
- `oxidebbs-server nodes disconnect`
- `oxidebbs-server nodes message`
- `oxidebbs-server nodes broadcast`

The socket is local-only. On Unix, startup removes a stale
`runtime/oxidebbs-control.sock` only when no process is listening on it. If
another server is still accepting connections on that socket, `serve` reports a
startup error instead of falling back to offline control behavior.
The runtime directory is created with mode `0700`, the control socket is chmoded
to `0600`, and Unix clients are accepted only when their peer UID matches the
server process UID.

Important: CLI/sysop commands must run as the same OS user that owns
`oxidebbs-server` (the UID used by the running `serve` process). Example:

```bash
sudo -u oxidebbs oxidebbs-server nodes list
```

Running control commands as a different local user (including root) will usually
be rejected with a peer-UID mismatch error even if the socket file is reachable.

The runtime path and any pre-existing `runtime/node-NNN` directories must be
writable by the server UID, and OxideBBS will try to keep node runtime
directories at mode `0700`. If you move an installation to a different OS user
or restore runtime files from backup, stop the server and remove stale
`runtime/node-*` directories before restarting:

```bash
rm -rf runtime/node-*
```

If cleanup ever remains necessary, stop the managed service or development
process first, then remove only the socket file:

```bash
systemctl stop oxidebbs
rm -f runtime/oxidebbs-control.sock
```

## Health checks

Useful commands:

```bash
oxidebbs-server check
oxidebbs-server status
oxidebbs-server db doctor
oxidebbs-server --config config/oxidebbs.example.toml check
oxidebbs-server doors check example
oxidebbs-server doors test example --user sysop --dry-run
oxidebbs-server nodes show 1
oxidebbs-server nodes watch
```

Useful local-only status checks:

```bash
oxidebbs-server nodes reset-stale
oxidebbs-server status --json
```

## Live door launch

The caller `Doors` menu lists enabled configured doors and launches the selected
door through the server-side bridge. Before launch, verify:

- the door working directory exists
- the configured runner executable, usually `dosemu`, exists on `PATH` or at
  the configured path
- the drop-file format is `DOOR.SYS`, `DORINFO1.DEF`, `CHAIN.TXT`,
  `DOORFILE.SR`, `PCBOARD.SYS`, or `CALLINFO.BBS`
- the runtime directory is writable
- the time limit is greater than zero

During a live door, the byte path is:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOSEMU2-emulated COM1 UART
  <-> DOS door program
```

The server tracks node state as `in_door` while the bridge owns the transport.
Caller disconnect or `nodes disconnect <node>` terminates the bridge and triggers
normal session cleanup. `nodes disconnect <node>` also terminates an active door
bridge and then lets the caller session follow normal disconnect cleanup.
OxideBBS never bundles door binaries; sysops provide their own licensed door
files under the configured door working directory.

Each live launch records a door run id in the caller summary and audit events.
The `door_started` audit detail includes the resolved runner command, runtime
directory, generated drop file, DOSEMU2 config path, COM1 PTY path, and per-run
runner stdout/stderr files under `logs/doors/`. If a door flashes and returns to
the menu, check `audit door <door-key>`, `doors runs show <run-id>`, and the
matching runner stderr/stdout files first.

## Common door launch failures

- `dosemu` not found: fix `PATH` or `runner` path in door config.
- `runner = "dosbox"` or another non-DOSEMU2 runner: live caller doors use the
  DOSEMU2 COM1 PTY bridge and DOSEMU2 command-line flags. Set the door runner to
  `dosemu`.
- Runner exits before COM1 exists: inspect the run id from the caller summary,
  then read the `door_finished` audit details and `logs/doors/*<run-id>*` files
  for DOSEMU or DOS program startup errors.
- PTY path never appears: verify `/dev/pts` is mounted and writable; restart
  node runtime path and check stale permissions.
- PTY permission denied: ensure `runtime/` and per-node directory permissions are
  writable by the server user.
- Door never writes COM1: check generated `OXDOSEMU2.CONF`, and validate the
test door path and command arguments.
- Caller disconnect during door: bridge should stop immediately and session state
  should return to normal disconnect flow.
- Timeout kill: confirm the `time_limit_minutes` and `door_timed_out` state in
  `door_runs`.
- Stale runtime directory cleanup: stale files under `runtime/node-XXX/` should be
  removed when the run finalizes.

## Node operations

- `nodes list` shows live node runtime state (`available`, `connecting`, `login`,
  `main_menu`, `reading_messages`, `posting_message`, `in_door`,
  `disconnecting`, `offline`, `stale`).
- `nodes show <n>` prints detail and heartbeat age.
- `nodes message <n>` and `nodes broadcast <n>` write direct text to live caller
  sessions.
- `nodes disconnect <n>` asks live runtime to disconnect through the normal path.
- `nodes reset-stale` requests disconnect of stale nodes and marks them
  `disconnecting` before cleanup.

If the control socket is unreachable for these node commands, the CLI records audit
intent and returns explicit warning text.

## Logs

Use structured logging.

Recommended env during development:

```bash
RUST_LOG=oxidebbs=debug,tower=info
```

## Backups

Use the DecentDB-aware sysop command boundary:

```bash
oxidebbs-server db backup backups/oxidebbs.ddb
oxidebbs-server db export --format json > backups/oxidebbs.json
oxidebbs-server db import --format json backups/oxidebbs.json
oxidebbs-server db compact --output backups/oxidebbs-compacted.ddb
```

`db import --format json` requires a schema-initialized, schema-only target:

1. Create a fresh target with `oxidebbs-server db init`. A database created by
   `oxidebbs-server setup` is seeded with a sysop account and starter data, so it
   is not an import target.
2. Verify with `oxidebbs-server db stats`.
3. Import from a full backup JSON file.

`db import --format json` is implemented as a full restore (not merge), preserving
UUIDs with full foreign-key-aware insertion order and transactionality.

`db compact --output <path> [--overwrite]` writes and verifies a separate
compacted DecentDB file. It refuses to write to the active database path; stop
the server before manually replacing the active database with the compacted
output.

Audit retention is configured with `[audit].retention_days` and defaults to
`365`. Runtime inserts do not auto-delete old audit rows; scheduled maintenance
should call `oxidebbs-server audit purge-retention` or
`oxidebbs-server audit purge-before <timestamp>`. Use `--dry-run` before a live
purge and `--json` when automation needs stable counts.

## Load-test note

Before exposing a board beyond a local development listener, run a connection
limit smoke test with `max_connections + 1` callers. The extra caller should
receive `System is busy. Please try again later.` while all accepted node slots
remain stable and controllable through `nodes list`.

## DOSEMU2 smoke path

When a runtime-capable host is available, validate the bridge path end-to-end:

```bash
OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
```

Optional runtime check can be used on production-like host settings before opening
doors to callers.
