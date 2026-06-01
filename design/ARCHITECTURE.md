# OxideBBS Architecture

## Overview

OxideBBS is a single-node Rust application with a modular internal architecture.

```text
+-------------------------+
|      oxidebbs-server    |
+------------+------------+
             |
             v
+-------------------------+
|      oxidebbs-core      |
+------------+------------+
             |
   +---------+----------+----------------+
   |                    |                |
   v                    v                v
oxidebbs-term     oxidebbs-db      oxidebbs-door
   |                    |                |
   v                    v                v
ANSI/CP437          DecentDB        DOSEMU2
```

## Why modular monolith?

A BBS is stateful and latency-sensitive but not a distributed enterprise system. A modular monolith keeps deployment simple while preserving code boundaries.

## Major boundaries

### `oxidebbs-server`

Responsible for process startup, config loading, logging, and service orchestration.

### `oxidebbs-core`

Owns BBS domain logic:

- user sessions
- nodes
- menus
- permissions
- message commands
- current activity

### `oxidebbs-term`

Owns terminal rendering:

- ANSI escape sequences
- CP437 conversion
- line input
- paging
- screen assets

### `oxidebbs-telnet`

Owns telnet-specific protocol handling.

### `oxidebbs-db`

Owns persistence using DecentDB.

### `oxidebbs-door`

Owns door execution.

### `oxidebbs-sysop`

Owns admin tooling and future Ratatui console.

## Dependency rule

Higher-level crates may depend on lower-level crates. Lower-level crates should not depend on the server binary.

Preferred direction:

```text
server -> core -> term/db/door/telnet
sysop  -> core/db
```

## Important design principle

The caller-facing terminal pipeline must stay byte-oriented and CP437-aware.
