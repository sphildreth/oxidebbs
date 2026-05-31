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

Do not introduce SQLite, PostgreSQL, MySQL, Redis, or an ORM.

## 7. Write model

Use a deliberate write model to avoid chaotic concurrent writes from many sessions.

Preferred pattern:

```text
Session tasks
    ↓
DbCommand channel
    ↓
single DbWriter service
    ↓
DecentDB transaction
```

Reads may be direct where safe, but writes should initially be centralized.

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

Drop files to support early:

- `DOOR.SYS`
- `DORINFO1.DEF`
- `DOORFILE.SR`
- `CHAIN.TXT`

## 9. Configuration

Configuration should be TOML.

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
```

## 10. Observability

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

## 11. Error handling

- Avoid panics in long-running server paths.
- Use domain-specific errors.
- Log enough context for sysop troubleshooting.
- Preserve caller-friendly messages.
- Never expose sensitive config or password hashes to callers.

## 12. Testing strategy

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
