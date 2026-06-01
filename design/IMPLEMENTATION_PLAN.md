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
| Phase 1 — Local Server Control Plane | TODO | Let sysop CLI commands control a running local server process. | Local control socket, command protocol, node command integration. |
| Phase 2 — Live Node Heartbeats And State | TODO | Make node status authoritative while the server is running. | Node registry, heartbeat timestamps, stale detection, status output. |
| Phase 3 — Live Door Launch Integration | TODO | Replace caller-facing door placeholder with controlled door execution. | Door menu launch path, run records, drop files, timeout cleanup. |
| Phase 4 — DecentDB Schema Migrations | TODO | Upgrade compatible pre-alpha databases instead of requiring recreation. | Migration runner and schema `2 -> 3` migration. |
| Phase 5 — DecentDB Restore And Compact Semantics | TODO | Make `db import` and `db compact` real, safe commands. | Restore design, import command, compaction command or documented unsupported state. |
| Phase 6 — Sysop CLI Hardening | TODO | Make CLI output, error behavior, and smoke coverage production-friendly. | CLI integration tests, stable JSON contracts, help ordering tests. |
| Phase 7 — Documentation And Runbook Completion | TODO | Bring operator docs to parity with implemented runtime behavior. | Updated docs site, runbook, changelog, and design docs. |

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
- Do not hold locks across `.await`. If shared runtime state is needed, use a
  narrow synchronous critical section and drop the guard before awaiting.
- Prefer small, typed command/request/response structs over stringly typed
  maps inside Rust code.

## Phase 0 — Current Baseline

Status: `COMPLETE`

The current baseline after the CLI-first sysop implementation includes:

- Top-level `oxidebbs-server` command groups:
  - `admin`
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
  `serve` process.
- `db import` and `db compact` are explicit command boundaries but intentionally
  blocked until restore and compaction semantics are specified.

Do not re-implement this phase unless a regression is found.

## Phase 1 — Local Server Control Plane

Status: `TODO`

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

### Module Shape

Create a new server-side module:

```text
crates/oxidebbs-server/src/control.rs
```

The module should define:

```rust
pub enum ControlRequest {
    Status,
    NodesList,
    NodeDisconnect { node_number: u16, reason: String },
    NodeMessage { node_number: u16, text: String },
    NodeBroadcast { text: String },
}

pub enum ControlResponse {
    Ok,
    Status(ControlStatus),
    Nodes(Vec<ControlNodeStatus>),
    Error { message: String },
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
{"ok":false,"error":"node 1 is not active"}
```

Implementation detail:

- Use `serde` and `serde_json`.
- Read one line per request.
- Write one line per response.
- Keep the protocol stable enough for CLI tests.

### Server Runtime Integration

In `serve`, start the control listener after config, DecentDB, menus, and the
node coordinator are initialized but before accepting callers.

The control listener must:

- Remove a stale socket file only if no server is listening on it.
- Create the parent runtime directory if missing.
- Refuse to bind outside `config.paths.runtime`.
- Log startup path.
- Shut down naturally when the process exits.

### CLI Integration

Update `crates/oxidebbs-server/src/sysop_cli.rs`:

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

## Phase 2 — Live Node Heartbeats And State

Status: `TODO`

### Objective

Create authoritative runtime node state so sysop tools can distinguish
available, connecting, login, menu, message, door, disconnecting, offline, and
stale nodes.

### Required Node States

Use the states from `design/OxideBBS_SYSOP_INTERFACE.md`:

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

Represent these as a Rust enum in server runtime code:

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

### Phase 2 Acceptance Criteria

- Live `nodes list` shows every configured node.
- Active callers move through at least `connecting`, `login`, `main_menu`, and
  `disconnecting` states.
- Stale state is detectable and visible.
- No lock is held across `.await`.
- `./scripts/dev-check.sh` passes.

## Phase 3 — Live Door Launch Integration

Status: `TODO`

### Objective

Replace the caller-facing "Doors feature placeholder" with real controlled door
execution for authenticated callers.

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

### Tests

Add tests for:

- Door selection rendering and parsing.
- Disabled door rejection.
- Missing runner or unsupported drop-file validation.
- Dry-run service path using `DryRunDoorRunner`.
- Door run record lifecycle.

Do not require DOSBox in unit tests. Use dry-run or a fake runner.

### Documentation

Update:

- `design/DOORS.md`
- `design/RUNBOOK.md`
- `docs/project/sysop-cli.md`
- `docs/about/changelog.md`

### Phase 3 Acceptance Criteria

- Caller door menu no longer returns a placeholder for enabled configured
  doors.
- Door dry-run service path is tested.
- Door run DB records are written and finished.
- Timeout behavior is tested without requiring a real DOS door.
- `./scripts/dev-check.sh` passes.

## Phase 4 — DecentDB Schema Migrations

Status: `TODO`

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

Migration `2 -> 3`:

```sql
ALTER TABLE message_areas ADD COLUMN enabled BOOL NOT NULL DEFAULT TRUE;
UPDATE system_config SET value = '3', updated_at = CURRENT_TIMESTAMP
WHERE key = 'schema_version';
```

If DecentDB does not support `ALTER TABLE ADD COLUMN` exactly as written,
choose the supported DecentDB path and document it in `design/DECENTDB_SCHEMA.md`.

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

### Documentation

Update:

- `design/DECENTDB_SCHEMA.md`
- `design/RUNBOOK.md`
- `docs/about/changelog.md`

### Phase 4 Acceptance Criteria

- A schema `2` test DB migrates to schema `3`.
- Fresh DB behavior remains unchanged.
- `./scripts/dev-check.sh` passes.

## Phase 5 — DecentDB Restore And Compact Semantics

Status: `TODO`

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

- `db import --format json <path>` imports only into an empty database.
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
- It fails on any existing user/message/session/door/audit rows.

### Compact Semantics

If DecentDB exposes a supported compaction or vacuum API, wrap that API.

If DecentDB does not expose compaction:

- Keep `db compact` returning an explicit unsupported error.
- Document that decision in `design/RUNBOOK.md` and `docs/project/sysop-cli.md`.
- Do not fake compaction by copying files unless DecentDB documents that as
  safe.

### Tests

Add tests for:

- Export/import round trip into an empty in-memory DB or temp DB.
- Import rejects non-empty DB.
- Import rejects unsupported version.
- Import rejects malformed JSON.

### Documentation

Update:

- `design/DECENTDB_SCHEMA.md`
- `design/RUNBOOK.md`
- `docs/project/sysop-cli.md`
- `docs/about/changelog.md`

### Phase 5 Acceptance Criteria

- `db import --format json` has defined, tested behavior or remains explicitly
  unsupported with documented rationale.
- `db compact` has defined, tested behavior or remains explicitly unsupported
  with documented rationale.
- `./scripts/dev-check.sh` passes.

## Phase 6 — Sysop CLI Hardening

Status: `TODO`

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
- Error messages for unsupported `db import`/`db compact`.

### Command Ordering

Top-level help should remain alphabetized, with Clap's generated `help` command
allowed at the bottom:

```text
admin
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

- Top-level JSON responses should be objects or arrays, never mixed text.
- Error JSON can be added later, but successful `--json` output must not include
  human-readable prefixes.
- IDs should remain strings.
- Booleans should remain booleans.
- Numeric counts should remain numbers.

### Documentation

Update:

- `docs/project/sysop-cli.md`
- `docs/about/changelog.md`

### Phase 6 Acceptance Criteria

- CLI help order has a test.
- Representative JSON commands have tests.
- `./scripts/dev-check.sh` passes.

## Phase 7 — Documentation And Runbook Completion

Status: `TODO`

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

### Phase 7 Acceptance Criteria

- Docs site builds.
- Runbook can be followed by a sysop from setup to status checks.
- Changelog includes every user-visible behavior change.
- `./scripts/dev-check.sh` passes.

## Recommended Immediate Next Step

Start with **Phase 1 — Local Server Control Plane**.

This is the highest-leverage next step because the CLI already presents live
node-control commands. Until the running server has a local control socket, those
commands can only record intent. The control plane should be implemented before
adding more CLI surface area so future sysop commands can use the same live
runtime path.

## Implementation Notes For Coding Agents

- Work one phase at a time.
- Keep commits scoped to the phase.
- Before editing, read the modules named in that phase.
- Prefer adding small modules over expanding `sysop_cli.rs` further.
- Do not remove compatibility aliases unless explicitly requested.
- Do not introduce a remote admin service.
- If a phase exposes a behavior as implemented in CLI help, it must either work
  or clearly return an explicit unsupported/offline message.
- When uncertain, choose the smallest local-only implementation that keeps the
  server secure by default and document the decision.
