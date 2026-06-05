# ADR 0030: Door Provider Model For Remote Door Services

## Status

Accepted

## Context

v1 door execution is centered on local DOS doors through DOSEMU2. v1.2 extends
support to BBSLink/DoorParty-style remote providers while keeping local DOS door
execution as the compatibility baseline.

## Decision

v1.2 extends door support through a provider model:

- `DosDoorProvider` for existing DOSEMU2 doors
- `RemoteDoorProvider` for BBSLink/DoorParty-style external game services

All providers share a typed launch contract containing caller identity, node,
terminal profile, time limit, security level, and audit context.

Local DOS doors remain isolated under the configured door root and runtime
directories. Remote providers use provider-specific connectors and must not
receive more caller data than their connector requires.

Door definitions are mutable DecentDB records after setup import. TOML
definitions remain seeds and examples, not the runtime source of truth once a
database is initialized.

## Consequences

- Door add/edit commands mutate DecentDB through a shared service layer.
- TUI and CLI use the same service methods.
- Provider credentials need redaction in logs, config displays, and exports.
- Existing DOSEMU2 containment rules still apply to DOS doors.
