# OxideBBS Implementation Plan

This document defines the recommended next implementation phases after the
CLI-first sysop interface work. It is written for coding agents and maintainers:
each phase has enough scope, constraints, and validation detail to implement
without inventing architecture mid-task.

## Phase Map

Status values:

- `TODO`: not started on the current branch.
- `IN PROGRESS`: actively being implemented on the current branch.
- `COMPLETE`: implemented, documented, and validated according to this
  document's Definition of Done.

| Phase | Status | Goal | Primary Output |
| --- | --- | --- | --- |
| Phase 0 — Current Baseline | COMPLETE | CLI-first sysop interface, schema v3, docs, and release metadata are present. | `oxidebbs-server` exposes top-level sysop command groups. |
| Phase 0.5 — Structural Extraction Gate | COMPLETE | Reduce server monolith risk before adding live control behavior. | Command handler modules, `sysop_cli.rs` under 1000 lines, validated no-behavior-change refactor. |
| Phase 1 — Local Server Control Plane | COMPLETE | Let sysop CLI commands control a running local server process. | Local control socket, command protocol, node command integration. |
| Phase 2 — Live Node Heartbeats And State | COMPLETE | Make node status authoritative while the server is running. | Node registry, heartbeat timestamps, stale detection, status output. |
| Phase 3 — Live Door Launch Integration | COMPLETE | Replace caller-facing door placeholder with controlled door execution. | Door menu launch path, run records, drop files, timeout cleanup. |
| Phase 4 — DecentDB Schema Migrations | COMPLETE | Upgrade compatible pre-alpha databases instead of requiring recreation. | Migration runner and schema `2 -> 3` migration. |
| Phase 5 — DecentDB Restore And Compact Semantics | COMPLETE | Make `db import` and `db compact` real, safe commands. | Restore design, import command, compaction command or documented unsupported state. |
| Phase 6 — Sysop CLI Hardening | COMPLETE | Make CLI output, error behavior, and smoke coverage production-friendly. | CLI integration tests, stable JSON contracts, help ordering tests. |
| Phase 7 — Documentation And Runbook Completion | COMPLETE | Bring operator docs to parity with implemented runtime behavior. | Updated docs site, runbook, changelog, and design docs. |

## Definition Of Done

Every phase is complete only when all of the following are true:

1. Implementation is complete for every task in the phase and no TODO behavior
   remains hidden behind success messages.
2. Any product or operator behavior change is documented in the relevant
   design docs, usually `design/SPEC.md`, `design/RUNBOOK.md`,
   `design/DECENTDB_SCHEMA.md`, `design/DOORS.md`,
   `design/OxideBBS_SYSOP_INTERFACE.md`, or this file.
3. User-facing documentation is updated or created under `docs/` when a sysop,
   caller, deployer, or developer workflow changes.
4. `docs/about/changelog.md` is updated under `Unreleased` or a version heading,
   following `design/VERSIONING_GUIDE.md`.
5. Configuration examples are updated when behavior depends on config:
   `config/oxidebbs.example.toml` must remain valid.
6. Tests cover the changed behavior at the lowest practical level:
   unit tests for pure logic and repository behavior, integration-style tests
   for CLI/control-socket flows where possible.
7. The required Rust gate passes:

   ```bash
   ./scripts/dev-check.sh
   ```

8. If docs changed, the docs site builds:

   ```bash
   npm run docs:build
   ```

9. If a release version or package metadata changed, all OxideBBS Rust crate
   versions stay aligned, `Cargo.lock` is refreshed, documentation package
   metadata is refreshed when applicable, and stale version strings are scanned.
10. `git diff --check` passes.
11. Important implementation decisions are captured in docs. If a coding agent
    must make a decision because the user is unavailable, choose the simplest
    maintainable option consistent with existing architecture and document the
    decision in the relevant phase notes.

## Cross-Cutting Constraints

- Rust only, edition 2024.
- DecentDB remains the only database. Do not add SQLite, PostgreSQL, Redis, or
  an ORM.
- Telnet remains the only remote caller transport for v1.
- ANSI/CP437 caller rendering remains byte-oriented.
- Do not use Ratatui for remote caller UI. Ratatui is only for local sysop UI.
- Door execution must remain isolated from core session logic.
- Do not bundle copyrighted or abandonware DOS doors.
- Shared dependencies belong in root `[workspace.dependencies]`. Use
  `cargo add`; do not hand-edit dependency versions.
- `serde_json` is already a workspace dependency and is the required JSON
  implementation for CLI output and the local control protocol. Do not add a
  second JSON crate.
- Do not hold locks across `.await`. If shared runtime state is needed, use a
  narrow synchronous critical section and drop the guard before awaiting.
- Prefer small, typed command/request/response structs over stringly typed
  maps inside Rust code.

## Required Structural Maintenance Gates

The current `crates/oxidebbs-server/src/sysop_cli.rs` file is large
(`2694` lines at the time this plan was written). Future phases must not keep
adding substantial handler logic to that monolith. The extraction is not optional
cleanup; it is Phase 0.5 and must be completed before Phase 1 begins.

The current `crates/oxidebbs-server/src/serve.rs` file is also large
(`1869` lines at the time this plan was written). Its `handle_caller` function
owns transport setup, session lifecycle, login flow, new-user flow, main menu
routing, message flow, and disconnect cleanup inline. Phase 3 must not bolt a
door bridge directly into that shape unless the bridge can be injected through a
small tested helper. Treat session-loop extraction as the Phase 3 structural
gate.

Required gates:

- Phase 0.5: extract sysop CLI command handlers before Phase 1.
- Phase 3 pre-work: extract caller session flow seams before live door bridge
  work if the bridge cannot be tested without editing `handle_caller` directly.

## Phase 0 — Current Baseline

Status: `COMPLETE`

The current baseline after the CLI-first sysop implementation includes:

- Top-level `oxidebbs-server` command groups:
  - `ansi`
  - `audit`
  - `check`
  - `config`
  - `db`
  - `doors`
  - `logs`
  - `messages`
  - `nodes`
  - `serve`
  - `setup`
  - `status`
  - `sysop`
  - `users`
- Global options:
  - `--config`
  - `--data`
  - `--json`
  - `--no-color`
  - `--verbose`
- DecentDB schema marker `3`.
- Message areas have an `enabled` flag.
- Door definitions have an `enabled` flag in config.
- `setup` can initialize a database and create an initial sysop account.
- Node live-control commands currently record audited intent and update
  session rows where possible; they do not yet communicate with a running
  `serve` process. The current user-facing contract is explicit offline wording
  such as "live transport control requires a future control socket" and
  "recorded for delivery by a future live control channel"; Phase 1 replaces
  those fallback messages only when a live control socket is actually reached.
- `db import` and `db compact` are explicit command boundaries. Phase 5 defines
  restore semantics, enables JSON import into schema-only databases, and keeps
  compaction explicitly unsupported until DecentDB exposes a safe compaction API.

Do not re-implement this phase unless a regression is found.

## Phase 0.5 — Structural Extraction Gate

Status: `COMPLETE`

### Objective

Split CLI command execution out of `sysop_cli.rs` before any control-socket
behavior is added. This is a no-behavior-change maintenance gate, and Phase 1
must not start until it is complete.

### Required Work

Create:

```text
crates/oxidebbs-server/src/commands/
```

with at least:

```text
commands/status.rs
commands/nodes.rs
commands/doors.rs
commands/messages.rs
commands/users.rs
commands/db.rs
commands/ansi.rs
commands/logs.rs
commands/audit.rs
commands/config.rs
```

Move command handler logic out of `sysop_cli.rs`:

- Move all command handler `run_*` functions into the relevant command module,
  except for a minimal top-level dispatch function if keeping it in
  `sysop_cli.rs` is simpler.
- Move command-specific helpers with their handlers. For example, door JSON,
  door checks, and door sync helpers belong with `commands/doors.rs`.
- Keep shared CLI types, common error types, `print_json`, `emit_ok`, and small
  parser/dispatch glue in `sysop_cli.rs` only when they are genuinely shared.
- Preserve the current command names, help text, and output behavior.
- Do not add control-socket behavior in this phase.

### Line-Count Target

After extraction:

- `crates/oxidebbs-server/src/sysop_cli.rs` must be under `1000` lines.
- If Clap type definitions alone make that target impossible, move subcommand
  type definitions into command modules too and re-export them through
  `commands::`.

### Tests

This phase is a refactor, so tests should prove behavior did not change:

- Existing unit tests pass.
- Add smoke tests for representative command dispatch if practical.
- At minimum, manually verify `oxidebbs-server --help` still shows the same
  top-level command order.

### Documentation

No user-facing docs are required for a pure no-behavior-change refactor. Update
`docs/about/changelog.md` only if module extraction changes user-visible output
or behavior.

### Phase 0.5 Acceptance Criteria

- `crates/oxidebbs-server/src/commands/` exists.
- The listed command modules exist.
- Command handler logic has moved out of `sysop_cli.rs`.
- `sysop_cli.rs` is under `1000` lines.
- `./scripts/dev-check.sh` passes before Phase 1 begins.

### Phase 0.5 Completion Notes

- Command handlers and command-specific Clap types now live under
  `crates/oxidebbs-server/src/commands/`, including the required command
  modules plus `serve.rs`, `setup.rs`, and `sysop.rs`.
- `sysop_cli.rs` remains the parser/dispatch boundary and owns shared CLI
  context, error, JSON output, prompt, audit, and small database lookup helpers.
- The extraction intentionally does not add control-socket behavior. Phase 1
  remains responsible for live server control.
- Top-level help order was manually verified after extraction:
  `ansi`, `audit`, `check`, `config`, `db`, `doors`, `logs`, `messages`,
  `nodes`, `serve`, `setup`, `status`, `sysop`, `users`.

## Phase 1 — Local Server Control Plane

Status: `COMPLETE`

### Objective

Add a local-only control plane so sysop CLI commands can talk to a running
`oxidebbs-server serve` process. This phase makes `nodes disconnect`,
`nodes message`, `nodes broadcast`, and `status` operational against live
runtime state instead of only DecentDB rows.

### Required Decisions

Use a Unix domain socket for local control on Unix-like systems:

```text
runtime/oxidebbs-control.sock
```

Rationale:

- OxideBBS is currently a local sysop tool, not a remote admin service.
- A Unix socket is simpler and safer than opening a TCP admin port.
- File permissions on `runtime/` can restrict local access.

Windows support can be added later with named pipes. Do not add TCP fallback in
this phase unless the project explicitly chooses a remote admin security model.

Implementation must be platform-gated:

- Put Unix socket implementation behind `#[cfg(unix)]`.
- Provide a non-Unix stub that returns a clear "local control socket is not
  supported on this platform yet" error.
- The CLI offline fallback path must still work on every platform.
- Do not fail `cargo check --workspace` on non-Unix targets because the module
  unconditionally imports Unix-only APIs.

### Phase 0.5 Dependency

Phase 0.5 is a hard prerequisite. Do not add control-socket client calls,
runtime protocol types, or live status/node behavior until command handlers have
been extracted from `sysop_cli.rs` and the Rust gate passes. The control-socket
client fallback behavior for `status` and `nodes` belongs in
`commands/status.rs` and `commands/nodes.rs`, not in `sysop_cli.rs`.

### Module Shape

Create a new server-side module:

```text
crates/oxidebbs-server/src/control.rs
```

The module should define:

```rust
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ControlRequest {
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "nodes.list")]
    NodesList,
    #[serde(rename = "nodes.disconnect")]
    NodeDisconnect { node_number: u16, reason: String },
    #[serde(rename = "nodes.message")]
    NodeMessage { node_number: u16, text: String },
    #[serde(rename = "nodes.broadcast")]
    NodeBroadcast { text: String },
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ControlResponse {
    #[serde(rename = "ok")]
    Ok { ok: bool },
    #[serde(rename = "status")]
    Status { ok: bool, status: ControlStatus },
    #[serde(rename = "nodes")]
    Nodes { ok: bool, nodes: Vec<ControlNodeStatus> },
    #[serde(rename = "error")]
    Error { ok: bool, error: String },
}

pub struct ControlStatus {
    pub board_name: String,
    pub uptime_seconds: u64,
    pub node_count: u16,
    pub active_nodes: usize,
}

pub struct ControlNodeStatus {
    pub node_number: u16,
    pub state: String,
    pub user_alias: Option<String>,
    pub remote_address: Option<String>,
    pub connected_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
}
```

The exact field list can expand, but do not remove these fields unless the
design docs are updated in the same change.

Do not rely on `rename_all` for `ControlRequest`. The protocol uses dotted type
names, and `rename_all = "snake_case"` would serialize `NodesList` as
`nodes_list`, which is not the wire contract.

Required request mapping:

| Rust variant | Wire `type` |
| --- | --- |
| `Status` | `status` |
| `NodesList` | `nodes.list` |
| `NodeDisconnect` | `nodes.disconnect` |
| `NodeMessage` | `nodes.message` |
| `NodeBroadcast` | `nodes.broadcast` |

Required response mapping:

| Rust variant | Wire `type` |
| --- | --- |
| `Ok` | `ok` |
| `Status` | `status` |
| `Nodes` | `nodes` |
| `Error` | `error` |

### Wire Format

Use newline-delimited JSON over the Unix socket:

```json
{"type":"nodes.list"}
{"type":"nodes.disconnect","node_number":1,"reason":"sysop_disconnect"}
{"type":"nodes.message","node_number":1,"text":"System going down in 5 minutes."}
{"type":"nodes.broadcast","text":"System going down in 5 minutes."}
{"type":"status"}
```

Responses should also be newline-delimited JSON:

```json
{"ok":true,"type":"nodes","nodes":[]}
{"ok":false,"type":"error","error":"node 1 is not active"}
```

Implementation detail:

- Use `serde` and `serde_json`.
- Read one line per request.
- Write one line per response.
- Keep the protocol stable enough for CLI tests.
- The listener should spawn a task per accepted control connection.
- A control handler reads one request line, dispatches it, writes one response
  line, and closes the connection.
- Do not keep sockets open for streaming, subscriptions, or long-polling in
  Phase 1. `nodes watch` should issue repeated short request/response calls.
- Message text fields must not contain literal newline characters in Phase 1.
  Normalize `\r`, `\n`, and `\r\n` to a single ASCII space before serializing
  `NodeMessage` or `NodeBroadcast`.
- Reject any control request line larger than `64 KiB` with a protocol error.
- Do not implement multi-line JSON payloads in this phase. If future commands
  need large or multiline bodies, replace newline-delimited JSON with a
  length-prefixed frame in a separate documented protocol revision.

### Server Runtime Integration

In `serve`, start the control listener after config, DecentDB, menus, and the
node coordinator are initialized but before accepting callers.

Introduce a shared runtime object for data that both caller tasks and the
control listener need to read or update:

```rust
pub struct ServerRuntime {
    // exact fields are implementation-defined for Phase 1
}
```

This object should be owned behind `Arc<ServerRuntime>` and cloned into spawned
caller tasks and control handler tasks. Phase 1 can keep this minimal: uptime,
configured node count, and enough session/node visibility for `status` and
`nodes list` are sufficient. Phase 2 can expand or replace it with the full
node registry.

The control listener must:

- Remove a stale socket file only if no server is listening on it.
- Create the parent runtime directory if missing.
- Refuse to bind outside `config.paths.runtime`.
- Log startup path.
- Shut down naturally when the process exits.

### CLI Integration

Update the extracted command modules, with `sysop_cli.rs` remaining dispatch
glue:

- `status` should try the control socket first. If unavailable, fall back to
  offline DecentDB/config status and clearly mark uptime as unavailable.
- `nodes list` and `nodes show` should try the control socket first. If
  unavailable, fall back to DecentDB active session rows.
- `nodes disconnect`, `nodes message`, and `nodes broadcast` should fail over as
  follows:
  - If control socket is available, send the live request and return the live
    result.
  - If unavailable, record audited intent as the current implementation does,
    and print that the live server was not reachable.

Do not silently claim live delivery when the control socket is unavailable.

### Tests

Add tests for:

- Request JSON parsing.
- Response JSON serialization.
- Unknown request type rejection.
- Two simultaneous control clients do not block each other.
- Control socket client request/response round trip using a temporary runtime
  path.
- CLI fallback behavior when the socket is absent can remain covered by unit
  tests around helper functions if spawning the full binary is too heavy.

### Documentation

Update:

- `design/SPEC.md`
- `design/RUNBOOK.md`
- `docs/project/sysop-cli.md`
- `docs/about/changelog.md`

Document that the socket is local-only and located under `runtime/`.

### Not In Scope

- Remote TCP admin service.
- Authentication or authorization beyond local filesystem permissions.
- Windows named-pipe implementation.
- Length-prefixed protocol framing.
- Persistent node state tables.

### If Blocked

- If Unix socket binding is unreliable in the current Tokio version, stop and
  document the exact API limitation before choosing another transport.
- If stale socket cleanup cannot be made safe, require manual cleanup in the
  runbook and leave automatic stale removal out of the implementation.
- If command extraction becomes larger than expected, complete the extraction
  and validation first, then resume control-socket work in a follow-up change.
- If extracting shared runtime state from `serve.rs` grows beyond Phase 1,
  implement an intermediate control state backed by DecentDB active session rows
  plus a simple process uptime value. That is enough for truthful `status` and
  `nodes list` while leaving the full runtime registry to Phase 2.

### Phase 1 Acceptance Criteria

- Running `oxidebbs-server serve` creates a local control socket.
- Running `oxidebbs-server status` while the server is running shows uptime
  from live state.
- Running `oxidebbs-server nodes list` while the server is running uses live
  node state.
- Running `oxidebbs-server nodes broadcast "text"` reaches the running server
  through the socket.
- Offline fallback messages are explicit and truthful.
- `./scripts/dev-check.sh` passes.

### Completion Notes

- Implemented `crates/oxidebbs-server/src/control.rs` with newline-delimited
  JSON request/response types, explicit dotted request names, a `64 KiB`
  request-line limit, newline normalization for message text, and Unix-domain
  socket transport at `runtime/oxidebbs-control.sock`.
- `serve` binds the control socket before accepting telnet callers on Unix. A
  stale socket file is removed only when no process accepts a connection on it;
  an active socket fails startup instead of silently disabling live control.
- Live `status`, `nodes list`, and `nodes show` read process uptime and node
  snapshots from `Arc<ServerRuntime>`.
- Live `nodes disconnect`, `nodes message`, and `nodes broadcast` enqueue
  runtime commands for active caller tasks. Disconnects flow through normal
  session cleanup, while messages and broadcasts are displayed by the active
  caller loop.
- If the socket is absent or unsupported, CLI commands preserve the DecentDB
  fallback behavior and state that the live server was not reachable.
- Non-Unix builds keep a clear unsupported control-socket stub and retain
  offline CLI fallback behavior; Windows named-pipe support remains future work.

## Phase 2 — Live Node Heartbeats And State

Status: `COMPLETE`

### Objective

Create authoritative runtime node state so sysop tools can distinguish
available, connecting, login, menu, message, door, disconnecting, offline, and
stale nodes.

### Required Node States

The codebase already has `oxidebbs-core/src/node.rs::NodeStatus`:

```rust
pub enum NodeStatus {
    Idle,
    Connected,
    LoggingIn,
    InMenu,
    InDoor,
    Uploading,
    Downloading,
    Chatting,
    Voting,
    Disconnected,
}
```

Do not ignore this type. The runtime may use a server-only enum for finer
operational detail, but the plan must include explicit mapping to and from the
core domain state. The server runtime state should be:

```text
available
connecting
login
main_menu
reading_messages
posting_message
in_door
disconnecting
offline
stale
```

Represent these as a Rust enum in server runtime code only if the narrower
caller-session states are needed:

```rust
pub enum RuntimeNodeState {
    Available,
    Connecting,
    Login,
    MainMenu,
    ReadingMessages,
    PostingMessage,
    InDoor,
    Disconnecting,
    Offline,
    Stale,
}
```

Convert to stable snake_case strings only at CLI/API boundaries.

Required mapping to existing core `NodeStatus`:

| Runtime state | Core `NodeStatus` |
| --- | --- |
| `available` | `Idle` |
| `connecting` | `Connected` |
| `login` | `LoggingIn` |
| `main_menu` | `InMenu` |
| `reading_messages` | `InMenu` |
| `posting_message` | `InMenu` |
| `in_door` | `InDoor` |
| `disconnecting` | `Disconnected` |
| `offline` | `Disconnected` |
| `stale` | `Disconnected` plus stale flag in runtime/control status |

If this mapping proves awkward, update `NodeStatus` in `oxidebbs-core` in the
same phase and document the public domain change in `design/SPEC.md` and
`docs/about/changelog.md`. Do not leave two unrelated enums without tests for
their conversion.

### Runtime Model

Introduce a runtime node registry owned by `serve`:

```rust
pub struct NodeRegistry {
    nodes: Mutex<BTreeMap<u16, RuntimeNode>>,
}

pub struct RuntimeNode {
    pub node_number: u16,
    pub state: RuntimeNodeState,
    pub session_id: Option<String>,
    pub user_id: Option<String>,
    pub user_alias: Option<String>,
    pub remote_address: Option<String>,
    pub connected_at: Option<String>,
    pub last_heartbeat_at: Instant,
}
```

`serve.rs` already has a `NodeCoordinator` that owns node slot allocation using
`occupied: Mutex<Vec<bool>>` plus a `Semaphore`. The new registry must not create
an unsynchronized second source of truth for node allocation.

Required relationship:

- Prefer replacing `NodeCoordinator` with `NodeRegistry`, where the registry
  owns allocation, occupancy, runtime state, and the semaphore limit.
- If direct replacement is too risky, make `NodeRegistry` wrap
  `NodeCoordinator` so all allocation and release operations pass through one
  type.
- Do not leave `NodeCoordinator::occupied` and `NodeRegistry::nodes` as
  independent mutexes that session tasks update separately.
- If both structures temporarily exist during refactoring, document the exact
  synchronization rule and add tests for allocation, release, disconnect, and
  stale-session cleanup.

`Instant` is allowed only inside the process-local runtime registry. It must not
appear in control protocol structs or JSON output because it is not
serializable and has no meaning across processes. Control responses must expose
one or both of:

```rust
pub last_heartbeat_at: Option<String>;      // UTC RFC3339-ish timestamp
pub heartbeat_age_seconds: Option<u64>;     // computed at response time
```

Rules:

- Do not hold the mutex across `.await`.
- Session handlers update state before and after major flows.
- Node state transitions should be explicit helper methods, not scattered map
  mutation.
- Heartbeats update `last_heartbeat_at` from the session loop and while door
  execution is active.

### Stale Detection

Add a configurable or constant stale threshold. Start with:

```text
stale_after_seconds = idle_timeout_seconds + 30
```

If a node has an active session but no heartbeat newer than the threshold,
control-plane status should report `stale`. Do not automatically kill stale
nodes in this phase unless explicitly commanded.

### CLI Behavior

Update:

- `nodes list` prints all nodes and their runtime state.
- `nodes show <node>` includes session/user/remote/heartbeat details.
- `nodes watch` refreshes state from the control socket.
- `nodes reset-stale` marks stale nodes as disconnecting and asks their session
  task to terminate through the control channel.

### Tests

Add tests for:

- State transition helpers.
- Heartbeat updates.
- Stale detection.
- Control response node ordering.
- `nodes list --json` shape if practical.

### Documentation

Update:

- `design/SPEC.md`
- `design/OxideBBS_SYSOP_INTERFACE.md`
- `docs/project/sysop-cli.md`
- `docs/about/changelog.md`

### Not In Scope

- Persistent historical node-state timeline.
- Remote node control over TCP.
- Windows named-pipe control support.
- Full Ratatui dashboard implementation.
- Automatic stale-session kill unless explicitly requested by
  `nodes reset-stale`.

### If Blocked

- If existing `NodeStatus` is too coarse, add conversion tests first, then
  decide whether to extend `NodeStatus` or keep runtime-only states.
- If heartbeat updates complicate the current session loop, implement
  read-only live node state first and leave stale detection as a clearly
  documented follow-up within Phase 2.

### Phase 2 Acceptance Criteria

- Live `nodes list` shows every configured node.
- Active callers move through at least `connecting`, `login`, `main_menu`, and
  `disconnecting` states.
- Stale state is detectable and visible.
- No lock is held across `.await`.
- `./scripts/dev-check.sh` passes.

### Completion Notes

- Replaced the previous `serve.rs` `NodeCoordinator` with `ServerRuntime` as the
  single owner of node allocation, occupancy, runtime state, heartbeat data, and
  the connection semaphore.
- Added server-only `RuntimeNodeState` values for `available`, `connecting`,
  `login`, `main_menu`, `reading_messages`, `posting_message`, `in_door`,
  `disconnecting`, `offline`, and `stale`, with tests covering the explicit
  mapping to `oxidebbs_core::node::NodeStatus`.
- Control node responses now include stable snake_case state strings plus
  `last_heartbeat_at` and `heartbeat_age_seconds`.
- The stale threshold is `telnet.idle_timeout_seconds + 30`. Stale nodes remain
  visible and are not killed automatically.
- `nodes reset-stale` now uses the control socket when available, marks stale
  nodes as `disconnecting`, and asks their caller tasks to terminate through the
  runtime command channel. When the live server is unreachable, it records
  audited intent instead of claiming live reset.

## Phase 3 — Live Door Launch Integration

Status: `COMPLETE`

### Objective

Replace the caller-facing "Doors feature placeholder" with real controlled door
execution for authenticated callers.

### Required Pre-Design And Structural Work

Before implementing live door launch, inspect `serve.rs` and decide whether the
bridge can be injected without expanding `handle_caller` directly. If not,
extract session-flow helpers first.

Required session-loop seam:

- Keep transport setup and final disconnect cleanup in the caller session owner.
- Move login/new-user flow routing behind small helper functions or a
  `session/` module.
- Move main-menu action dispatch behind a helper that can call a door service
  without embedding process I/O logic in `handle_caller`.
- Keep message flow separate from door flow.
- Add focused tests around the extracted helpers before adding the door bridge.

Required transport design task:

- Document the chosen byte-bridge contract in `design/DOORS.md` or
  `design/TELNET.md` before coding the bridge.
- The design must explain how the normal session read loop is paused while a
  door owns caller I/O.
- The design must explain how raw caller bytes are forwarded without
  line-based menu parsing.
- The design must explain how timeout or sysop disconnect can shut down the
  bridge from outside the caller read path.
- Update `LoopbackTransport` and any other test doubles in the same change as
  any `Transport` trait extension.

Preferred transport shape if the existing `Transport` trait is insufficient:

- Add a narrow bridge-specific split or adapter rather than broad terminal
  semantics to every transport.
- The adapter should provide independent read and write halves or equivalent
  concurrency so child output can be sent while waiting for caller input.
- The bridge must return control to the normal session loop after the child
  exits, so the implementation must either recompose the transport or expose a
  bridge API that borrows and restores it safely.

### Required Behavior

When an authenticated caller chooses the configured door menu action:

1. List enabled doors.
2. Let the caller select a door by key or number.
3. Validate the selected door:
   - enabled
   - working directory exists
   - runner exists
   - drop-file format supported
   - runtime directory writable
   - time limit greater than zero
4. Generate the drop file in the node runtime directory.
5. Record a `door_started` audit event.
6. Insert a `door_runs` row.
7. Run the door with the configured runner.
8. Enforce timeout.
9. Record `door_finished` or `door_timed_out`.
10. Update the `door_runs` row.
11. Clean up node runtime state as appropriate.
12. Return the caller to the main menu.

### Isolation Rules

- Door execution must stay in `oxidebbs-door` or a server adapter around it.
- Core session logic should call a small door service API; it should not know
  process details.
- Drop-file rendering must remain tested in `oxidebbs-door`.
- Do not bundle any real DOS door binaries.

### Service Shape

Add a server-side helper, for example:

```rust
struct DoorService<'a> {
    db: &'a OxideDb,
    config: &'a OxideConfig,
}
```

Responsibilities:

- Resolve configured and DB-backed door state.
- Build `DoorRunRequest`.
- Insert and finish `DoorRunRecord`.
- Insert audit events.
- Return user-facing success/failure text.

### Door I/O Bridge

The hard part of live doors is not drop-file generation; it is bridging telnet
caller bytes to the door process and door output back to the caller. The current
`oxidebbs-door::DoorRunner` API is synchronous and process-oriented:

```rust
pub trait DoorRunner {
    fn run(&self, request: &DoorRunRequest) -> Result<DoorRunResult, DoorError>;
}
```

That API can prepare and launch a door, but it does not expose stdin/stdout for
interactive caller I/O. Phase 3 must either extend the door runner API or add a
server-side interactive runner adapter.

Recommended design:

1. Keep `oxidebbs-door` responsible for:
   - validating door definitions
   - preparing node runtime directories
   - rendering drop files
   - building a `DoorRunPlan`
2. Add an interactive server adapter in `oxidebbs-server`, for example
   `door_session.rs`, that:
   - writes a run-local DOSEMU2 config mapping `COM1` with
     `$_com1 = "pts <node runtime dir>/OXCOM1.PTY"`
   - waits for the run-local PTY path to appear
   - spawns the configured process without forwarding DOSEMU2 stdout/stderr to
     the caller
   - pauses normal menu input handling while the door is active
   - forwards bytes between the caller `Transport` and the COM1 PTY
   - watches for caller disconnect, child exit, timeout, and sysop disconnect
   - terminates the child on timeout or caller disconnect
3. Keep the current dry-run path for tests and sysop troubleshooting.

Transport interaction rules:

- The bridge should work against the existing `Transport` trait where possible.
- If the trait lacks the methods needed for bidirectional streaming and
  shutdown, complete the required transport design task before changing code,
  then extend the trait narrowly and update existing telnet tests.
- The session loop must not process menu commands while the bridge owns the
  caller transport.
- The node registry should report `in_door` while the bridge is active if
  Phase 2 has landed. If Phase 2 has not landed, door launch can still proceed
  and state reporting can be added later.

Ordering note: Phase 3 can be partially implemented before Phase 2 if the door
bridge does not depend on heartbeat-aware node state. In that case, defer only
`in_door` state reporting to Phase 2 and document the temporary limitation.

### Tests

Add tests for:

- Door selection rendering and parsing.
- Disabled door rejection.
- Missing runner or unsupported drop-file validation.
- Dry-run service path using `DryRunDoorRunner`.
- Door run record lifecycle.
- Interactive bridge behavior using a fake child process or test helper command
  that echoes stdin to stdout.
- Timeout cleanup of a fake long-running child process.

Do not require DOSEMU2 in unit tests. Use dry-run or a fake runner.

### Documentation

Update:

- `design/DOORS.md`
- `design/RUNBOOK.md`
- `docs/project/sysop-cli.md`
- `docs/about/changelog.md`

### Not In Scope

- Bundling real door binaries.
- Full DOS terminal emulation beyond byte forwarding.
- File-transfer protocol implementation.
- Door editor UI.
- Multi-node exclusive-door locking beyond rejecting obviously invalid
  definitions unless a locking design is added in the same phase.

### If Blocked

- If the existing `Transport` trait cannot support bidirectional bridge
  semantics cleanly, stop and update `design/DOORS.md` with a focused transport
  bridge design before coding further.
- If reliable child-process I/O is not feasible cross-platform in this phase,
  implement Unix support behind `#[cfg(unix)]`, keep dry-run available
  everywhere, and document the platform limit.

### Phase 3 Acceptance Criteria

- Caller door menu no longer returns a placeholder for enabled configured
  doors.
- Caller bytes are bridged to the child process and child output is bridged back
  to the caller in at least one tested interactive path.
- Door dry-run service path is tested.
- Door run DB records are written and finished.
- Timeout behavior is tested without requiring a real DOS door.
- `./scripts/dev-check.sh` passes.

### Completion Notes

- Added a server-side `door_session.rs` adapter with `DoorService`, caller door
  selection rendering/parsing, selected-door validation, run record lifecycle,
  audit events, runtime cleanup, and a run-local DOSEMU2 COM1 PTY byte bridge.
- Kept the existing `Transport` trait unchanged. The bridge temporarily borrows
  the caller transport, pauses normal menu/line parsing, and forwards bytes
  between the caller and the DOSEMU2 COM1 PTY.
- Live door execution marks the node `in_door`, watches for child exit, caller
  disconnect, timeout, and sysop disconnect, then returns normal completions and
  timeouts to the main menu.
- Updated `door_runs` finalization to persist byte counts, and updated
  `oxidebbs-door` command planning to use the configured runner executable.
- Added tests for door menu selection, disabled/missing/invalid validation,
  `DryRunDoorRunner` service lifecycle, finished door run records, interactive
  bridge echo behavior, and timeout cleanup without requiring DOSEMU2.

## Phase 4 — DecentDB Schema Migrations

Status: `COMPLETE`

### Objective

Implement explicit schema migrations so compatible pre-alpha databases can be
upgraded rather than rejected and recreated.

### Current Problem

The initializer rejects schema marker mismatches. This is safe, but the `2 -> 3`
change only adds `message_areas.enabled`, so it should be migratable.

### Required Design

Add migration support inside `oxidebbs-db`:

```text
crates/oxidebbs-db/src/migrations.rs
```

Expose:

```rust
pub fn migrate_to_current(db: &Db) -> decentdb::Result<()>;
```

Rules:

- Migrations are sequential.
- Each migration checks the current schema marker.
- Each migration updates the schema marker only after successful DDL/data
  changes.
- Failed migrations leave a clear error.
- Do not silently skip unknown future versions.

### Required Migration

First task: verify DecentDB DDL support with a focused test or local probe.
Do not assume `ALTER TABLE ADD COLUMN` works until a test demonstrates it
against the pinned DecentDB dependency.

Initial desired migration `2 -> 3`:

```sql
ALTER TABLE message_areas ADD COLUMN enabled BOOL NOT NULL DEFAULT TRUE;
UPDATE system_config SET value = '3', updated_at = CURRENT_TIMESTAMP
WHERE key = 'schema_version';
```

If DecentDB does not support `ALTER TABLE ADD COLUMN` exactly as written, do
not improvise silently. Stop Phase 4 implementation and document one of these
paths:

1. DecentDB-supported table rebuild:
   - create a replacement table with the new schema
   - copy rows with `enabled = TRUE`
   - recreate indexes/constraints
   - swap tables if DecentDB supports that safely
2. Recreate-only pre-alpha policy:
   - keep rejecting schema `2`
   - document that `2 -> 3` is not migrated
   - leave migration framework for future supported migrations

The selected path must be documented in `design/DECENTDB_SCHEMA.md`.

Selected implementation: DecentDB rejects direct `ALTER TABLE ADD COLUMN` on
checked tables, so Phase 4 uses the DecentDB-supported table-rebuild path. It
renames schema-2 `message_areas` and `messages` to `oxidebbs_schema2_*` archive
tables, creates replacement v3 tables, copies message-area and message rows,
restores reply links, recreates message indexes, renames the v3 tables into the
canonical names, and updates `system_config.schema_version` to `3` only after
the rebuild succeeds.

### Init Flow

Change open/init behavior:

1. If no schema exists, create latest schema.
2. If schema exists and is older than current, run migrations.
3. If schema exists and is current, continue.
4. If schema exists and is newer than current, refuse to open.

### Tests

Add tests for:

- Fresh DB initializes to current schema.
- Synthetic schema `2` DB migrates to `3`.
- Newer schema marker is rejected.
- Missing schema marker behavior is clear.
- DDL capability test proving the chosen `2 -> 3` strategy works with the
  pinned DecentDB dependency.

### Documentation

Update:

- `design/DECENTDB_SCHEMA.md`
- `design/RUNBOOK.md`
- `docs/about/changelog.md`

### Not In Scope

- General-purpose migration DSL.
- Downgrade migrations.
- Online migration while `serve` is accepting callers.
- Cross-database import/export.

### If Blocked

- If DecentDB lacks required DDL support, stop and write an ADR or schema note
  choosing table rebuild versus recreate-only policy before continuing.
- If table rebuild cannot preserve constraints safely, do not ship a partial
  migration. Keep recreate-only behavior and document it.

### Phase 4 Acceptance Criteria

- A schema `2` test DB migrates to schema `3`.
- Fresh DB behavior remains unchanged.
- `./scripts/dev-check.sh` passes.

### Phase 4 Completion Notes

- Added `crates/oxidebbs-db/src/migrations.rs` with `migrate_to_current` and a
  sequential migration runner.
- Confirmed with a focused test that the pinned DecentDB rejects direct
  `ALTER TABLE ... ADD COLUMN` on checked tables, so the selected `2 -> 3` path
  rebuilds `message_areas` and `messages` through replacement tables.
- Preserved schema-2 message areas, messages, message replies, and foreign-key
  relationships in the canonical schema-3 tables. Because DecentDB cannot drop
  the renamed schema-2 self-referencing `messages` table, the migration keeps
  the old tables under `oxidebbs_schema2_*` archive names outside runtime query
  paths.
- Updated schema open/init flow to create current schema when absent, migrate
  older schemas, refuse missing/unmarked existing tables, and refuse newer
  marker versions.

## Phase 5 — DecentDB Restore And Compact Semantics

Status: `COMPLETE`

Implementation completed in this phase:

- `db import --format json` now performs a schema-validated, transactional full restore into a schema-only DecentDB target.
- `db compact` returns an explicit, documented unsupported error because DecentDB exposes no safe compaction API in this release.

### Objective

Complete the currently blocked `db import` and `db compact` commands.

### Restore Semantics

Before coding import, document the restore model:

- Is import allowed only into an empty database?
- Does import preserve UUIDs?
- How are foreign-key ordering and constraints handled?
- Is import transactional?
- What happens if the import file schema version differs?
- Does import replace the whole database or merge records?

Recommended decision:

- `db import --format json <path>` is a whole-database restore, not a merge.
- It imports only into a schema-initialized, data-empty database.
- The only pre-existing rows allowed are internal schema/config rows required
  to open the database, such as `system_config.schema_version`.
- At the time this plan was written, `DbCommand::Init` calls `open_database`
  and does not seed a sysop user, message areas, doors, sessions, or audit
  events. Phase 5 must lock that behavior with a test and may use existing
  `db init` as the import-ready target creator if the test proves it remains
  schema-only.
- Add `db init --empty` before enabling import only if `db init` grows starter
  data behavior or shares code with `setup`. `db init --empty` must create the
  current schema without seeding a sysop user, message areas, doors, sessions,
  or audit events.
- A database created by the starter `setup` flow is not an import target if it
  already contains a sysop user or audit events. In that case, import must fail
  with a message that tells the sysop to create a schema-only target with
  `db init --empty`.
- It preserves UUIDs.
- It validates schema version first.
- It loads tables in dependency order:
  1. users
  2. message_areas
  3. messages
  4. sessions
  5. doors
  6. door_runs
  7. audit_events
- It fails on any existing row in the exported data tables.
- It performs all validation before writing rows.
- If DecentDB exposes transactions that cover the needed writes, import must run
  in one transaction. If DecentDB does not expose that support, keep import
  unsupported unless the implementation can prove partial writes cannot occur
  after validation succeeds.

### Compact Semantics

If DecentDB exposes a supported compaction or vacuum API, wrap that API.

If DecentDB does not expose compaction:

- Keep `db compact` returning an explicit unsupported error.
- Document that decision in `design/RUNBOOK.md` and `docs/project/sysop-cli.md`.
- Do not fake compaction by copying files unless DecentDB documents that as
  safe.

### Tests

Add tests for:

- Existing `db init` creates a schema-only import-ready target, or
  `db init --empty` does if that command is added.
- Export/import round trip into an empty in-memory DB or temp DB.
- Import accepts a schema-only target with only allowed internal rows.
- Import rejects a starter `setup` database that already has sysop or audit
  rows.
- Import rejects unsupported version.
- Import rejects malformed JSON.
- Import leaves the target unchanged after validation failure.

### Documentation

Update:

- `design/DECENTDB_SCHEMA.md`
- `design/RUNBOOK.md`
- `docs/project/sysop-cli.md`
- `docs/about/changelog.md`

### Not In Scope

- Merging users, messages, doors, or audit events into a populated database.
- Selective table import.
- Conflict resolution for duplicate aliases, message IDs, or door keys.
- Cross-version data transformation beyond rejecting unsupported export schema
  versions.
- File-level database replacement unless DecentDB documents that as a safe
  restore mechanism.

### If Blocked

- If DecentDB cannot provide transactional restore semantics and partial writes
  cannot be ruled out, keep `db import` unsupported and document the exact
  reason.
- If an empty import target cannot be created without the starter sysop account,
  add `db init --empty` before continuing with import.
- If export JSON is missing fields required to preserve IDs or relationships,
  update export first and treat that as part of Phase 5.

### Phase 5 Acceptance Criteria

- `db import --format json` restores into a schema-only target with defined,
  tested behavior or remains explicitly unsupported with documented rationale.
- `db compact` has defined, tested behavior or remains explicitly unsupported
  with documented rationale.
- `./scripts/dev-check.sh` passes.

## Phase 6 — Sysop CLI Hardening

Status: `COMPLETE`

### Objective

Make the CLI stable enough for repeated sysop use and future automation.

### Required Work

Add tests or smoke coverage for:

- Top-level help command order.
- `--json` output shape for:
  - `status`
  - `users list`
  - `nodes list`
  - `messages areas list`
  - `doors list`
  - `db stats`
- Non-interactive setup with `--data`.
- Config check on `config/oxidebbs.example.toml`.
- Error messages for unsupported `db import` formats and unsupported
  `db compact`.

### Command Ordering

Top-level help should remain alphabetized, with Clap's generated `help` command
allowed at the bottom:

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

Nested command groups should use one of these sane orderings:

- alphabetical if all commands are peer operations
- operational lifecycle order if one exists, such as `init`, `doctor`, `stats`,
  `backup`, `export`, `import`, `compact`, `verify`

Document non-alphabetical order with a short comment near the enum.

### JSON Contract

For stable automation:

- Top-level JSON responses for the commands listed in this phase must be
  objects. Existing pre-alpha array responses should be normalized in this
  phase and documented in the changelog.
- Error JSON can be added later, but successful `--json` output must not include
  human-readable prefixes.
- IDs should remain strings.
- Booleans should remain booleans.
- Numeric counts should remain numbers.

Required successful response shapes:

```json
{
  "board": "Example BBS",
  "version": "0.2.0",
  "database": "data/oxidebbs.db",
  "telnet": "127.0.0.1:2323",
  "nodes": { "total": 4, "active": 0 },
  "doors": { "enabled": 1, "total": 1 },
  "messages": { "areas": 1 }
}
```

`users list --json`:

```json
{
  "users": [
    {
      "id": "uuid",
      "alias": "sysop",
      "real_name": "System Operator",
      "email": null,
      "security_level": 100,
      "is_sysop": true,
      "created_at": "2026-05-31T00:00:00Z",
      "last_login_at": null,
      "total_calls": 0,
      "time_bank_minutes": 0,
      "status": "active"
    }
  ]
}
```

`nodes list --json`:

```json
{
  "nodes": [
    {
      "node_number": 1,
      "state": "available",
      "user_alias": null,
      "session": null,
      "last_heartbeat_at": null,
      "heartbeat_age_seconds": null
    }
  ]
}
```

When a node has an active session, `session` uses the same object shape as
existing session JSON and `user_alias` is the resolved alias when available.

If Phase 1 or Phase 2 live state is unavailable when Phase 6 is implemented,
keep this same object shape:

- Include every configured node number.
- For a configured node with no active DB session row, use
  `state: "available"`.
- For a node with an active DB session row but no live runtime heartbeat, use
  `state: "offline"` and include the session row so operators can see the
  stale/offline evidence.
- Set `last_heartbeat_at` and `heartbeat_age_seconds` to `null`.
- Resolve `user_alias` from the DB when practical; otherwise keep it `null`.

`messages areas list --json`:

```json
{
  "areas": [
    {
      "id": "uuid",
      "key": "general",
      "name": "General",
      "description": "General discussion",
      "kind": "local",
      "network_id": null,
      "read_security_level": 0,
      "post_security_level": 10,
      "moderated": false,
      "enabled": true
    }
  ]
}
```

`doors list --json`:

```json
{
  "doors": [
    {
      "id": "uuid",
      "key": "lord",
      "name": "Legend of the Red Dragon",
      "runner": "dry-run",
      "working_dir": "doors/lord",
      "command": "lord.exe",
      "drop_file": "door.sys",
      "exclusive": false,
      "time_limit_minutes": 30,
      "enabled": true
    }
  ]
}
```

`db stats --json`:

```json
{
  "schema_version": 3,
  "users": 1,
  "message_areas": 1,
  "messages": 0,
  "sessions": 0,
  "active_sessions": 0,
  "doors": 1,
  "door_runs": 0,
  "audit_events": 0
}
```

### Documentation

Update:

- `docs/project/sysop-cli.md`
- `docs/about/changelog.md`

### Not In Scope

- Stable JSON error schema.
- Backward compatibility with pre-Phase 6 top-level array outputs.
- Remote admin API contract.
- Shell completion generation.
- Internationalized CLI output.

### If Blocked

- If changing top-level arrays to objects breaks too much existing test
  coverage, keep the object contract and update tests/docs in the same phase;
  this is still pre-alpha hardening.
- If live node data is unavailable because Phase 1 or Phase 2 is not complete,
  retain the same `nodes list --json` object shape with offline/session-derived
  values and null heartbeat fields.

### Phase 6 Acceptance Criteria

- CLI help order has a test.
- Representative JSON commands have tests.
- `./scripts/dev-check.sh` passes.

### Completion Notes

- Top-level command order is covered by a Clap command-factory test.
- Stable successful JSON object contracts are covered for `status`,
  `users list`, `nodes list`, `messages areas list`, `doors list`, and
  `db stats`.
- `users list`, `nodes list`, `messages areas list`, and `doors list` no
  longer emit top-level arrays under `--json`; this is an intentional pre-alpha
  contract hardening decision.
- Non-interactive setup with a global `--data` override and config checking for
  `config/oxidebbs.example.toml` are covered by command-level tests.
- Because Phase 5 implemented JSON restore, Phase 6 validates unsupported
  import formats instead of treating `db import` itself as unsupported. `db
  compact` remains explicitly unsupported until DecentDB exposes a safe
  compaction API.

## Phase 7 — Documentation And Runbook Completion

Status: `COMPLETE`

### Objective

Make the documentation site and design docs accurately describe the implemented
server operations.

### Required Docs

Create or update:

- `docs/project/sysop-cli.md`
- `docs/project/deployment.md`
- `docs/project/getting-started.md`
- `docs/project/setup.md`
- `design/RUNBOOK.md`
- `design/SPEC.md`
- `design/OxideBBS_SYSOP_INTERFACE.md`
- `design/DECENTDB_SCHEMA.md`
- `design/TASKS.md`
- `docs/about/changelog.md`

`docs/project/sysop-cli.md` is referenced by earlier phases. If it does not
exist when a phase first needs it, create it in that phase with the content
relevant to that phase, then complete it in Phase 7.

### Required Content

Docs must cover:

- Setup flow.
- Config validation.
- Starting `serve`.
- Using the local control socket.
- Reading node status.
- Disconnecting or messaging nodes.
- Door dry-run testing.
- Door live launch behavior.
- Database backup/export/import/compact semantics.
- Schema migration behavior.
- How to recover from stale runtime socket files.
- Which operations are local-only.

### Validation

Run:

```bash
npm run docs:build
./scripts/dev-check.sh
```

### Not In Scope

- Marketing copy or public launch material.
- Hosted documentation deployment changes.
- Screenshots or video walkthroughs.
- Remote administration documentation beyond explicitly local-only behavior and
  the reason remote admin is not implemented.

### If Blocked

- If VitePress navigation does not include a new required page, update the docs
  site navigation in the same phase rather than leaving an orphaned file.
- If implemented behavior differs from `design/OxideBBS_SYSOP_INTERFACE.md`,
  update that design document to match the shipped local-only behavior and call
  out deferred work in `design/TASKS.md`.
- If a prior phase intentionally left `db import`, `db compact`, live control,
  or door launch unsupported, document the unsupported state as explicit product
  behavior instead of omitting it.

### Phase 7 Acceptance Criteria

- Docs site builds.
- Runbook can be followed by a sysop from setup to status checks.
- Changelog includes every user-visible behavior change.
- `./scripts/dev-check.sh` passes.

### Phase 7 Completion Notes

- Completed required documentation updates in:
  - `docs/project/sysop-cli.md`
  - `docs/project/deployment.md`
  - `docs/project/getting-started.md`
  - `docs/project/setup.md`
  - `design/RUNBOOK.md`
  - `design/SPEC.md`
  - `design/OxideBBS_SYSOP_INTERFACE.md`
  - `design/DECENTDB_SCHEMA.md`
  - `design/TASKS.md`
  - `docs/about/changelog.md`
- Added coverage for setup, config validation, serve startup, control socket use
  and stale-socket recovery, node status/disconnect/message/broadcast behavior,
  door test/live launch notes, database backup/export/import/compact semantics,
  schema migration, and local-only operations.

## Recommended Immediate Next Step

All implementation phases documented in this file are complete as written.

## Implementation Notes For Coding Agents

- Work one phase at a time.
- Keep commits scoped to the phase.
- Before editing, read the modules named in that phase.
- Do not add substantial command handler logic back to `sysop_cli.rs`; keep it
  as parser/dispatch glue and shared CLI helpers.
- Do not remove active command names unless explicitly requested.
- Do not introduce a remote admin service.
- If a phase exposes a behavior as implemented in CLI help, it must either work
  or clearly return an explicit unsupported/offline message.
- When uncertain, choose the smallest local-only implementation that keeps the
  server secure by default and document the decision.
