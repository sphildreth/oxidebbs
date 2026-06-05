# ADR 0004: Telnet for v1, Serial/Modem for v1.2

## Status

Superseded by v1.2 implementation. Serial/modem transport is now included in v1.2 per ADR 0019.

## Context

OxideBBS v1.0 targets telnet-only to reduce initial scope. Serial/modem support was deferred to v1.2.

## Decision

Design the session layer around a transport abstraction so serial/modem support can be added in v1.2.

## Consequences

- Telnet is the only transport in v1.0.
- Serial/modem support is implemented in v1.2 per ADR 0019.
- The transport abstraction allows both telnet and serial to share session logic.
- All session logic must avoid assuming TCP-specific behavior.
