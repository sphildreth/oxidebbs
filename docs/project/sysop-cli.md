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

Global options:

```bash
-c, --config <PATH>
--data <PATH>
--json
--no-color
-v, --verbose
```

The old `admin` group remains as a compatibility alias for the first local
admin commands.

## Operational Notes

`setup` writes a starter config, initializes DecentDB, creates the initial sysop
account, and creates the default local message area. Non-interactive setup
requires `--sysop-password`.

`check` validates config references, telnet bind syntax, screen assets, door
definitions, and runtime paths. Door checks warn when local DOS binaries or
runners are not installed.

`nodes disconnect`, `nodes message`, `nodes broadcast`, `nodes enable`, and
`nodes disable` record sysop intent in audit rows. Live transport control is
deferred until the server has a local control socket and node heartbeat model.

`doors test --dry-run` creates drop files without launching DOSBox. Without
`--dry-run`, the configured runner is invoked locally.

`db export --format json` is read-only. `db import` and `db compact` are present
as explicit command boundaries, but remain blocked until restore and compaction
semantics are specified for DecentDB.

## Schema Compatibility

The sysop CLI adds a message-area `enabled` flag and bumps the pre-alpha
DecentDB schema marker to `3`. Existing development databases with schema `2`
must be recreated.
