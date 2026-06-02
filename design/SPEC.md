# OxideBBS Technical Specification

## 1. Architecture style

OxideBBS is a Rust modular monolith.

It should ship as one primary server binary with internal crates for domain boundaries:

- `oxidebbs-server`
- `oxidebbs-core`
- `oxidebbs-term`
- `oxidebbs-telnet`
- `oxidebbs-db`
- `oxidebbs-door`
- `oxidebbs-sysop`

This keeps the system easy to run while keeping the codebase clean.

## 2. Runtime model

```text
Telnet Listener
    ↓
Transport
    ↓
Session
    ↓
Menu / Command Router
    ↓
Core Services
    ↓
DecentDB Repository Layer
```

Door execution follows a separate path:

```text
Session
    ↓
Door Service
    ↓
Node Manager
    ↓
Drop File Writer
    ↓
Door Runner
    ↓
DOS Runtime
```

## 3. Core concepts

### Board

The configured BBS instance.

### Node

A logical caller slot. Nodes are assigned to active sessions and may map to per-node door directories.

### Session

A live caller interaction. A session has a transport, user context, terminal state, node assignment, and current activity.

### Transport

The I/O abstraction used by callers. v1 supports telnet. v2 may support serial/modem.

### Menu

A configurable command surface made of display assets and command mappings.

### Door

An external program launched by OxideBBS, usually a DOS door using a drop file.

### Message area

A local or network-backed discussion area.

## 4. Transport design

Transport should be byte-oriented.

```rust
pub trait Transport {
    async fn read_byte(&mut self) -> Result<Option<u8>>;
    async fn write_all(&mut self, bytes: &[u8]) -> Result<()>;
    async fn hangup(&mut self) -> Result<()>;
}
```

Implementations:

- `TelnetTransport` for v1
- `SerialTransport` later
- `LoopbackTransport` for tests

The `serve` runtime binds the configured telnet address, opens DecentDB before
accepting callers, assigns node slots up to the configured node and connection
limits, records session/audit lifecycle rows, and closes sessions on caller
disconnect, logoff, or idle timeout.

## 5. ANSI/CP437 design

OxideBBS must not treat remote caller output as normal Unicode UI.

The terminal layer should support:

- Raw ANSI bytes
- CP437 character conversion where needed
- Box drawing characters
- 16-color ANSI palette
- Screen clear, cursor positioning, and basic SGR sequences
- 40-column and 80-column terminal profiles
- Width-aware menus, prompts, status bars, line wrapping, and paging
- Safe line editor for caller input
- Output paging

The default caller profile may be 80x25, but 40-column callers are a supported
target, not an edge case. Screen assets should either have width-specific
variants or render through layouts that can fit within 40 columns without
truncating commands or corrupting ANSI/CP437 art.

Avoid using Ratatui for remote caller UI. Ratatui is appropriate for local sysop/admin TUI only.

## 6. Database design

DecentDB is the only system database.

The repository layer should hide persistence details from the BBS core.

Initial storage domains:

- users
- sessions
- nodes
- messages
- message_areas
- doors
- door_runs
- audit_events
- system_config
- network_config

Implemented DecentDB tables must use DecentDB-native types where the domain is
known. Entity IDs use `UUID`, lifecycle fields use `TIMESTAMPTZ`, caller peer IP
fields use `IPADDR`, and boolean flags use `BOOL`. Relationships between users,
sessions, messages, message areas, doors, door runs, and audit events must be
enforced with DecentDB foreign keys instead of application-only conventions.
Bounded numeric and label fields should have `CHECK` constraints.

The database records an OxideBBS schema marker in `system_config`. Fresh
databases are initialized at the current schema, compatible older pre-alpha
schemas are migrated sequentially before runtime use, and missing, malformed, or
newer markers are refused with clear operator-facing errors.

Do not introduce SQLite, PostgreSQL, MySQL, Redis, or an ORM.

## 7. Write model

Use a deliberate write model to avoid chaotic concurrent writes from many sessions.

Longer term, the preferred pattern is:

```text
Session tasks
    ↓
DbCommand channel
    ↓
single DbWriter service
    ↓
DecentDB transaction
```

The current v1 implementation uses direct repository writes through the shared
DecentDB wrapper and keeps multi-row restore operations inside explicit
transactions. This is acceptable for the current local telnet scope because the
repository layer owns the SQL boundary and the CI suite exercises the active
write paths. A single `DbWriter` service remains the next scaling step if
write contention or transaction serialization becomes a practical issue.

## 8. Door runner design

Door support is a core capability, not an afterthought.

Door runner responsibilities:

- Resolve door definition
- Allocate node execution context
- Create per-run/per-node working directory
- Generate drop files
- Launch DOS runtime
- Bridge I/O between caller and process
- Enforce time limits
- Kill/cleanup on disconnect
- Persist door run result

V1 includes a redistributable door fixture:

- `Oxide Door Check` (`key = "oxide-check"`) is an Oxide-owned test executable,
  implemented in Free Pascal and committed as
  `tools/doors/oxide-door-check/dist/OXIDECHK.EXE`.
- The checked-in fixture is validated by `tools/doors/oxide-door-check/SHA256SUMS`.
- The build target is `i8086-msdos`.
- The fixture is multi-node aware and reports the active node number for diagnostics.
- The fixture does not grant third-party door redistribution rights.

Drop files to support early:

- `DOOR.SYS`
- `DORINFO1.DEF`
- `DOORFILE.SR`
- `CHAIN.TXT`

The initial implementation includes `DOOR.SYS` and `DORINFO1.DEF` generation,
per-node runtime directory helpers, dry-run execution, DOSEMU2 command planning,
live caller launch from the configured `Doors` menu, byte bridging between the
caller transport and child process, timeout/sysop-disconnect cleanup, DecentDB
door-run records with byte counters, per-run DOSEMU stdout/stderr capture under
the configured logs directory, early-exit-before-COM1 diagnostics, and `in_door`
live node state. Additional drop-file formats remain compatible with this
boundary.

Generated drop files use the active board configuration for board and sysop
identity. `DORINFO1.DEF` maps the caller's user-profile real name into its
first-name and last-name fields because that format has no separate alias field.
`DOOR.SYS` includes both alias and real name.

The live caller door bridge is DOSEMU2-specific. Door runner values must resolve
to a DOSEMU2-compatible binary such as `dosemu`; DOSBox/DOSBox-Staging is not a
supported runner for the v1 COM1 PTY bridge.

## 9. Configuration

Configuration should be TOML.

The server binary should provide an interactive `setup` command that can create
a starter config before any config file exists. The command should prompt for
board identity, sysop identity, timezone, telnet bind, node count, database
path, and whether to include a placeholder door definition. It must create the
starter directories and refuse to overwrite an existing config unless explicitly
forced.

Example top-level sections:

```toml
[board]
name = "OxideBBS"

[telnet]
bind = "0.0.0.0:2323"

[database]
path = "./data/oxidebbs.ddb"

[paths]
ansi = "./assets/ansi"
doors = "./doors"
runtime = "./runtime"

[nodes]
count = 4

[flow]
login_screen = "login"
login_menu = "login"
post_login_screens = ["screen1", "screen2"]
main_menu = "main"

[screens.login]
ansi = "login/login.ans"
ansi_40 = "login/login-40.ans"
ascii = "login/login.asc"
text = "login/login.txt"

[menus.main]
screen = "main_menu"
prompt = "Command? "

[[menus.main.items]]
key = "D"
label = "Doors"
action = "doors"
```

Menu item actions must resolve to safe internal actions such as login, new user,
doors, messages, logoff, show screen, submenu, or no-op. Configured menu keys
are single ASCII characters and route case-insensitively. Screen assets are
selected from the best variant supported by the caller, with 40-column ANSI
preferred for ANSI callers at 40 columns or less.

Current v1 runtime behavior supports login, new user, doors, messages, logoff,
show screen, submenu, and no-op. `submenu` actions now resolve to the configured
target menu and remain in the resulting menu context at runtime; nested submenu
navigation is supported. Guest access is not enabled by default in v1: callers
must create or use an account before reaching the main menu.

The runtime sends `terminal.welcome_screen` from the configured ANSI asset path
on connect, then uses `flow.login_screen`, `flow.post_login_screens`, and
`flow.main_menu` for caller screen routing. The `terminal.logoff_screen` field
remains configuration metadata for future dedicated logoff rendering; logoff
currently sends a plain goodbye line.

## 10. Users and authentication

New-user and login flows are modeled in `oxidebbs-core`. User registration
normalizes profile fields, applies starter security defaults, and stores a
precomputed password hash created by the server/auth adapter. Login verifies
aliases case-insensitively, rejects inactive or locked accounts, and updates
call counters after a successful password verification.

Password hashes must use Argon2id PHC strings. Core code accepts a verifier
boundary so cryptographic verification can live in the server/auth adapter
without weakening domain tests.

## 11. Local messages

Message commands cover posting, reading visible messages, replies, private mail
recipient targeting, and local moderation state changes. Security levels gate
read and post operations. Moderated areas create pending messages until a sysop
approves or deletes them.

The telnet message menu must provide at least local area selection, visible
message listing, message reading, multi-line posting, and replies for
authenticated callers.

## 12. Sysop tooling

The server exposes CLI-first local sysop command groups:

- `setup`, `check`, `serve`, and `status`
- `users` for user listing, creation, status changes, security levels, password
  resets, sysop promotion, audits, and safe delete-as-disable behavior
- `nodes` for session listing plus live `disconnect`, `message`, and `broadcast`
  against a local control socket when the server is running
- `messages` for local area administration and message moderation
- `doors` for configured door inspection, checks, dry-run testing, drop-file
  generation, run history, and runtime cleanup
- `ansi` for screen listing, validation, preview, conversion, and inspection
- `db` for DecentDB initialization, doctor/stats, backup, verify, read-only JSON
  export, and JSON import restore
- `logs`, `audit`, and `config` for local troubleshooting

All sysop control is local in v1. There is no remote admin API or remote
interactive interface in this phase.

When a live control socket is unavailable, node disconnect/message/broadcast
commands preserve the previous audit intent behavior and report that live delivery
was not available. `db import --format json <path>` is now a full restore into a
schema-only database with schema checks and transactional insertion.
`db compact` is explicitly unsupported in this release because DecentDB does not
expose a safe compaction API; the command returns a clear error.

The local control socket is Unix-domain only in this phase and lives at
`runtime/oxidebbs-control.sock`. The protocol uses one newline-delimited JSON
request and one newline-delimited JSON response per connection. Live node
disconnect, message, and broadcast requests enqueue runtime commands consumed by
active caller tasks; disconnects use normal session cleanup, and messages are
rendered through the caller telnet transport.

The socket path is configurable through the config runtime directory and is always
local filesystem access. Stale socket recovery removes `oxidebbs-control.sock`
when no process is listening; active sockets block startup and return an explicit
start-time error.

While the server is running, node state is process-local and authoritative from
the runtime registry. Live node responses use stable snake_case states:
`available`, `connecting`, `login`, `main_menu`, `reading_messages`,
`posting_message`, `in_door`, `disconnecting`, `offline`, and `stale`. Stale
detection is based on heartbeat age with a threshold of telnet idle timeout plus
30 seconds. `nodes reset-stale` requests live disconnects for stale nodes when
the control socket is reachable.

Remote callers must never see Ratatui output; Ratatui remains local sysop/admin
UI only.

## 13. FTN/OxideNet boundary

FTN/OxideNet support starts with core domain types for FTN addresses,
echomail-area mappings, netmail messages, duplicate-detection keys, and packet
import/export boundaries. Packet parsing, bundling, compression, and transport
remain future infrastructure behind this boundary.

## 14. Observability

Use structured logging.

Required events:

- server_start
- server_stop
- caller_connected
- caller_disconnected
- login_success
- login_failure
- node_assigned
- door_started
- door_finished
- door_timed_out
- db_write_failed
- config_loaded

Door launch audit details must include the run id, resolved runner program and
arguments, runtime directory, generated drop file, DOSEMU2 COM1 PTY path, and
per-run runner stdout/stderr log paths when available. A door runner that exits
before the COM1 PTY appears must be distinguishable from a bridged door session
that exits normally.

## 15. Error handling

- Avoid panics in long-running server paths.
- Use domain-specific errors.
- Log enough context for sysop troubleshooting.
- Preserve caller-friendly messages.
- Never expose sensitive config or password hashes to callers.

## 16. Testing strategy

Required test categories:

- CP437 conversion tests
- ANSI renderer tests
- Telnet negotiation parser tests
- Menu routing tests
- User auth tests
- Drop-file generation snapshot tests
- Door runner dry-run tests
- DecentDB repository integration tests
- Session disconnect cleanup tests
