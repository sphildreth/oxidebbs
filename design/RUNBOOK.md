# OxideBBS Runbook

## Local development startup

```bash
cargo run -p oxidebbs-server -- serve
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

`oxidebbs-server setup` and `db init` create schema `3`. When an existing
DecentDB uses supported older schema version `2`, startup runs the upgrade
before serving callers. The `2 -> 3` migration rebuilds the message-area and
message tables so every message area receives `enabled = TRUE` while preserving
messages and replies. Because DecentDB cannot drop the renamed schema-2
self-referencing `messages` table, the old tables remain as
`oxidebbs_schema2_*` archives and are not used by runtime queries. Databases with
a future marker, missing marker, or unmarked existing tables are refused with a
clear error and should be opened only with compatible OxideBBS software.

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
- the drop-file format is `DOOR.SYS` or `DORINFO1.DEF`
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

## Common door launch failures

- `dosemu` not found: fix `PATH` or `runner` path in door config.
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
oxidebbs-server db compact
```

`db import --format json` requires a schema-initialized, schema-only target:

1. Create a fresh target with `oxidebbs-server db init`. A database created by
   `oxidebbs-server setup` is seeded with a sysop account and starter data, so it
   is not an import target.
2. Verify with `oxidebbs-server db stats`.
3. Import from a full backup JSON file.

`db import --format json` is implemented as a full restore (not merge), preserving
UUIDs with full foreign-key-aware insertion order and transactionality.

`db compact` is intentionally unsupported in this release because DecentDB does
not expose a safe compaction API contract.

## DOSEMU2 smoke path

When a runtime-capable host is available, validate the bridge path end-to-end:

```bash
OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
```

Optional runtime check can be used on production-like host settings before opening
doors to callers.
