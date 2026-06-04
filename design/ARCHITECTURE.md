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

Owns local admin tooling and the Ratatui sysop console.

### `oxidebbs-network`

Owns protocol-neutral network types such as FTN-style addresses, profiles,
links, packet boundaries, queue state, duplicate keys, and local/network message
envelopes.

### `oxidebbs-ftn`

Owns legacy FTN packet and message primitives: Type-2 packet I/O, echomail
kludges, and duplicate detection policy.

### `oxidebbs-binkp`

Owns BinkP network-mail transport framing and client/server session primitives.

### `oxidebbs-oxidenet`

Owns OxideNet-specific profile data, addressing defaults, applications, node
registry, and config package structures.

## Dependency rule

Higher-level crates may depend on lower-level crates. Lower-level crates should not depend on the server binary.

Preferred direction:

```text
server -> core -> term/db/door/telnet
sysop  -> core/db
```

## Important design principle

The caller-facing terminal pipeline must stay byte-oriented and CP437-aware.
Terminal compatibility is profile-based. C64 and C64 Ultimate support means
remote callers using C64 terminal applications can select or be detected as a
40-column, ASCII/PETSCII-friendly profile; OxideBBS itself remains the same
modern Rust server.
