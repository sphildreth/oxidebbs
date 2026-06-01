# Sysop CLI

OxideBBS administration is CLI-first. The binary exposes top-level command
groups instead of requiring sysops to use a web panel:

```bash
cargo run -p oxidebbs-server -- check
cargo run -p oxidebbs-server -- status
cargo run -p oxidebbs-server -- users list
cargo run -p oxidebbs-server -- nodes list
cargo run -p oxidebbs-server -- messages areas list
cargo run -p oxidebbs-server -- doors list
cargo run -p oxidebbs-server -- ansi list
cargo run -p oxidebbs-server -- db doctor
cargo run -p oxidebbs-server -- logs recent
cargo run -p oxidebbs-server -- audit recent
```

The automation contracts below always emit JSON objects with stable top-level
keys when `--json` is present:

- `status`:
  `{"board","version","database","telnet","nodes","doors","messages"}`
- `users list`:
  `{"users": [...]}`
- `nodes list`:
  `{"nodes": [...]}`
- `messages areas list`:
  `{"areas": [...]}`
- `doors list`:
  `{"doors": [...]}`
- `db stats`:
  `{"schema_version","users","message_areas","messages","sessions",...}`

Global options:

```bash
-c, --config <PATH>
--data <PATH>
--json
--no-color
-v, --verbose
```

## Operational Notes

`setup` writes a starter config, initializes DecentDB, creates the initial sysop
account, and creates the default local message area. Non-interactive setup
requires `--sysop-password`.

`check` validates config references, telnet bind syntax, screen assets, door
definitions, and runtime paths. Door checks warn when local DOS binaries or
runners are not installed.

`status`, `nodes list`, and `nodes show` prefer live runtime state from the
local server control socket at `runtime/oxidebbs-control.sock`. `nodes
disconnect`, `nodes message`, and `nodes broadcast` use the same socket to queue
commands for active caller sessions; when the socket is unreachable they still
record explicit sysop intent in audit rows.

Live node output reports runtime states such as `connecting`, `login`,
`main_menu`, `reading_messages`, `posting_message`, `disconnecting`, and
`stale`, along with heartbeat age when available. `nodes reset-stale` asks the
running server to disconnect stale sessions through the live control channel;
when the socket is unreachable it records audited intent instead. `nodes enable`
and `nodes disable` remain audited command boundaries until persistent node
state is modeled.

`doors test --dry-run` creates drop files without launching DOSBox. Without
`--dry-run`, the configured runner is invoked locally. The live caller `Doors`
menu uses the same configured door state, validates the selected door, records
door run history, and returns normal exits or timeouts to the main menu. During
an active door, live node status reports `in_door`; `nodes disconnect <node>`
terminates the bridge and then disconnects the caller through normal session
cleanup.

`db export --format json` is read-only. `db import --format json <path>` is now
implemented as a full restore into a schema-initialized, data-empty database:

- it validates schema compatibility and export reference integrity before writing;
- it preserves UUIDs and import ordering for message/user/door relationships;
- it runs as one transaction and rejects partially-written restores; and
- it returns an explicit failure if the target database has data outside schema
  metadata.

`db compact` is explicitly unsupported in this release because DecentDB does not
expose a production-safe compaction API. The command returns a clear error
rather than faking compaction.

Top-level help follows the canonical order:

```text
ansi
audit
check
config
db
doors
logs
messages
nodes
serve
setup
status
sysop
users
help
```

## Schema Compatibility

The sysop CLI adds a message-area `enabled` flag and bumps the pre-alpha
DecentDB schema marker to `3`. Current builds migrate supported schema `2`
development databases to `3` before opening them, and refuse missing, malformed,
or future markers with a clear error.
