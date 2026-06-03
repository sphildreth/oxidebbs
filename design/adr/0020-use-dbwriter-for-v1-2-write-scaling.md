# ADR 0020: Use DbWriter For v1.2 Write Scaling

## Status

Accepted

## Context

The v1 release line uses direct repository writes through the shared DecentDB
wrapper. That was sufficient for local messages, users, doors, audit events, and
the v1.1 TUI.

v1.2 adds heavier write paths: file transfers, network toss/scan, BinkP poll
logs, packet queues, OxideNet application state, remote admin actions, and
possibly serial sessions. The previous roadmap deferred a writer service until
write contention emerged. v1.2 creates enough write contention risk to justify
the service before it becomes a production failure.

## Decision

Introduce a `DbWriter` service that serializes high-contention writes through a
bounded queue.

The service must:

- own an `OxideDb` handle or repository bundle
- accept typed write jobs rather than raw SQL strings
- return typed results or typed errors to callers
- use explicit transactions for multi-row operations
- expose backpressure through bounded queue errors
- never hold a lock across `.await`
- preserve direct repository APIs for one-shot CLI commands and tests

Session tasks, live door execution, audit writes, file-transfer records,
network toss/scan, and BinkP poll logging should use `DbWriter` when running
inside the server runtime. CLI commands may use direct repositories unless they
need ordering with a live server.

## Consequences

- The server runtime gets one ordered write path for high-volume operations.
- Direct repository code remains useful for setup, migrations, imports, and
  isolated tests.
- Tests must cover queue backpressure, transaction failure rollback, and
  shutdown draining.
