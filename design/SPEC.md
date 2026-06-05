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
- `oxidebbs-network`

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

The I/O abstraction used by callers. v1 supports telnet. v1.2 supports serial/modem through ADR 0019.

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

- `TcpTransport` for telnet callers
- `SerialTransport` for enabled serial/modem devices
- `LoopbackTransport` for tests

The `serve` runtime binds the configured telnet address, opens enabled serial
devices, opens DecentDB before accepting callers, verifies the schema marker and
core DecentDB tables, assigns node slots up to the configured node and
connection limits, records session/audit lifecycle rows, and closes sessions on
caller disconnect, logoff, or idle timeout. Startup must fail before listening
if required database reads, required startup audit writes, or configured serial
line-state requirements fail.

The telnet transport reads from the socket into an internal 4096-byte buffer and
serves caller input one byte at a time to the parser. Telnet negotiation replies
are batched and flushed before caller-visible output, before blocking for more
input when replies are pending, and before hangup.

Serial caller sessions skip telnet negotiation and parse input as raw caller
bytes. File-transfer payloads use telnet IAC escaping only on telnet transports;
serial transfers use the raw serial byte stream.

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

The named caller terminal profiles are:

| Profile | Purpose | Width x height | Charset | ANSI/control policy |
| --- | --- | --- | --- | --- |
| `ansi80` | Modern BBS/ANSI callers such as SyncTERM | 80 x 25 | CP437 | ANSI and color allowed |
| `plain` | Generic telnet clients and unknown callers | 80 x 25 | ASCII | No ANSI required |
| `c64` | C64, C64 Ultimate, and C64 terminal application callers | 40 x 25 | PETSCII-friendly ASCII fallback | No ANSI unless explicitly overridden |

The `c64` profile is a caller compatibility profile. OxideBBS remains a modern
Rust BBS server; it is not a Commodore 64 executable, does not require a
`mos-c64-none` build target, and does not introduce a C64 thin-client
architecture.

The C64 profile must keep the login flow, main menu, message list, message
reader, file list, prompts, help text, and status lines usable at 40 columns.
Menus and generated caller text should wrap or truncate at the active profile
width instead of assuming 80 columns. ANSI/CP437 art must have an ASCII,
40-column, or C64-safe fallback path for basic navigation.

PETSCII translation is not complete yet. The terminal abstraction must keep the
charset field explicit and route C64 callers through ASCII/PETSCII-friendly
fallback assets until full PETSCII encode/decode support is implemented.

Plain and C64 profiles must avoid advanced ANSI escape sequences for screen
clear, cursor movement, color, and box drawing unless a sysop deliberately
configures an ANSI-capable profile. Output line endings are normalized to CRLF
for caller output. Caller input must normalize CR, LF, CRLF, telnet CR-NUL, and
common backspace/delete bytes (`0x08` and `0x7f`).

Terminal profiles may define optional output pacing, expressed in bytes per
second. Pacing exists for slower clients and may be disabled for the default
plain/ANSI profiles.

When terminal detection is unreliable, onboarding or account settings should
offer manual terminal profile selection: ANSI / 80-column, plain ASCII, and C64
/ 40-column / PETSCII-friendly. Persisted user preference requires a user
profile/schema field and is future work unless that storage exists in the
active release.

The default caller profile may be 80x25, but 40-column callers are a supported
target, not an edge case. Screen assets should either have width-specific
variants or render through layouts that can fit within 40 columns without
truncating commands or corrupting ANSI/CP437 art.

Caller-entered text that will be echoed or rendered remotely must be
CP437-compatible before it is stored. If it is not, the caller sees exactly:
`This BBS only accepts CP437-compatible text here.` Password input is hidden and
is not subject to CP437 rejection. Server-generated fallback/diagnostic output
may replace unencodable characters with `?`; caller-authored message subjects
and bodies must not be silently rewritten.

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
- network_profiles, network_links, network_areas, network_packets,
  network_messages, network_seen_by, network_path, network_duplicate_log,
  network_poll_log, network_area_subscriptions, and network_nodelist

Implemented DecentDB tables must use DecentDB-native types where the domain is
known. Entity IDs use `UUID`, lifecycle fields use `TIMESTAMPTZ`, caller peer IP
fields use `IPADDR`, and boolean flags use `BOOL`. Relationships between users,
sessions, messages, message areas, doors, door runs, and audit events must be
enforced with DecentDB foreign keys instead of application-only conventions.
Bounded numeric and label fields should have `CHECK` constraints.

The database records an OxideBBS schema marker in `system_config`. Fresh
databases are initialized at the current schema, and compatible older development
schemas are migrated sequentially before runtime use, and missing, malformed, or
newer markers are refused with clear operator-facing errors.

Do not introduce SQLite, PostgreSQL, MySQL, Redis, or an ORM.

## 7. Write model

Use a deliberate write model to avoid chaotic concurrent writes from many sessions.

The v1.2 write foundation includes a single-writer service for session,
message, and network work that must be serialized:

```text
Session tasks
    ↓
DbCommand channel
    ↓
single DbWriter service
    ↓
DecentDB transaction
```

`DbWriter` accepts bounded queued closures, runs each closure in a DecentDB
transaction, preserves submission order, reports queue backpressure, rolls back
failed writes, and drains accepted work during shutdown. Direct repository APIs
remain available for setup, import/restore, isolated CLI commands, and focused
tests where the caller owns transaction boundaries.

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
bind = "127.0.0.1:2323"

[auth]
failed_login_threshold = 5
failed_login_window_minutes = 10
failed_login_lockout_minutes = 15
new_user_security_level = 10

[auth.argon2]
memory_cost_kib = 19456
iterations = 2
parallelism = 1

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

[[menus.main.items]]
key = "M"
label = "Messages"
action = "messages"

[[menus.main.items]]
key = "G"
label = "Goodbye"
action = "logoff"
```

Menu item actions must resolve to safe internal actions such as login, new user,
doors, messages, logoff, show screen, submenu, or no-op. Configured menu keys
are single ASCII characters and route case-insensitively. Screen assets are
selected from the best variant supported by the caller, with 40-column ANSI
preferred for ANSI callers at 40 columns or less.

Telnet callers default to plain text until terminal capability negotiation proves
otherwise. The server requests terminal type and NAWS before the first caller
screen; SyncTERM and explicit ANSI-family terminal types receive ANSI assets,
while generic telnet clients such as `xterm`/`vt100` receive ASCII or text
assets. NAWS column width can select 40-column ANSI variants for ANSI callers.

Current v1 runtime behavior supports login, new user, doors, messages, logoff,
show screen, submenu, and no-op. The starter config exposes new-user
registration from the login menu only; the post-login starter main menu exposes
Doors, Messages, and Goodbye. `submenu` actions now resolve to the configured
target menu and remain in the resulting menu context at runtime; nested submenu
navigation is supported. Guest access is not enabled by default in v1: callers
must create or use an account before reaching the main menu.

The runtime sends `terminal.welcome_screen` from the configured ANSI asset path
on connect. ANSI callers receive the configured asset; plain text callers first
probe sibling `.asc` and `.txt` assets before falling back to stripped ANSI
text. The runtime then uses `flow.login_screen`, `flow.post_login_screens`, and
`flow.main_menu` for caller screen routing. Normal caller logoff renders
`terminal.logoff_screen`: ANSI callers receive the configured ANSI asset, plain
callers first probe sibling `.asc` and `.txt` assets, and missing assets log
context before falling back to a plain goodbye line.

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
- `files` for local file-area administration, file import/safe removal, and
  transfer-history inspection
- `ansi` for screen listing, validation, preview, conversion, and inspection
- `db` for DecentDB initialization, doctor/stats, backup, verify, read-only JSON
  export, and JSON import restore
- `logs`, `audit`, `config`, and `net` for local troubleshooting and deferred
  network operations

### Logging

The board config includes:

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

Accepted levels are `error`, `warn`, `info`, `debug`, and `trace`. Accepted file
formats are `text` and newline-delimited `json`. File logs are written under
`paths.logs`. `serve --log-level <level>` overrides `[logging].level` for that
serve run. The global `-v` override maps to `debug`, and repeated `-v` maps to
`trace`.

File rotation supports `daily`, `size`, and `never`. Daily rotation is based on
UTC date boundaries for the running process. Size rotation rotates before a write
would exceed `logging.rotation.max_size_mb`. `logging.rotation.max_files` is the
number of rotated archives retained.

At `debug` and `trace`, logs should include connections, session opens, menu
selections, audit events, login outcomes, door activity, message activity, and
disconnect reasons. DecentDB audit events remain the durable activity record;
file logs are the operational troubleshooting stream. JSON log files include
standard formatter fields plus event fields such as caller address, node,
session, menu key, user id/alias, audit event type, door key/name, message area,
message id, and outcome fields when applicable.

All mutating sysop control is local in v1.2. `[admin_web]` configuration exists
and is disabled by default. When explicitly enabled, it may expose a loopback
public `/status` endpoint plus sysop-authenticated read-only JSON API views.
The `[admin_web]` listener is plain HTTP only; OxideBBS does not implement
native HTTPS/TLS for this surface. HTTPS deployments must terminate TLS in a
local reverse proxy such as Caddy, nginx, or Traefik and forward plain HTTP to
the loopback listener.
Remote mutation attempts are guarded by session authentication, CSRF, replay
nonce/timestamp checks, rate limits, origin checks, and audit logging, but are
blocked by read-only mode.

When a live control socket is unavailable, node disconnect/message/broadcast
commands preserve the previous audit intent behavior and report that live delivery
was not available. `db import --format json <path>` is now a full restore into a
schema-only database with schema checks and transactional insertion.
`db compact --output <path> [--overwrite]` writes a verified compacted DecentDB
output file and refuses to write to the active database path. Replacing the
active database is a manual offline operator step.

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

FTN/OxideNet support starts with `oxidebbs-network` protocol-neutral types for
FTN-style addresses, network profiles and links, echomail-area mappings,
netmail messages, local/network message envelopes, duplicate-detection keys,
queue states, and packet import/export boundaries. `oxidebbs-core` re-exports
those types during the v1.2 transition.

Legacy FTN packet and kludge primitives live in `oxidebbs-ftn`; BinkP frame
primitives live in `oxidebbs-binkp`; OxideNet profile data lives in
`oxidebbs-oxidenet`. ZIP packet extraction is handled inside `oxidebbs-ftn`
with a strict top-level `.pkt` policy. Toss/scan workflows, outbound bundle
compression, ARJ extraction, nodelist processing, AreaFix, BinkP sessions, and
OxideNet onboarding are implemented behind those crate boundaries and exposed
through CLI and local sysop TUI service surfaces.

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

Door audit writes are best-effort. Failed audit writes increment the live
runtime `audit_write_failures` counter, which is included in control status
responses. Audit row ids and timestamps are generated inline by DecentDB for
normal runtime inserts.

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
