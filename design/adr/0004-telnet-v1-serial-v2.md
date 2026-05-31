# ADR 0004: Telnet for v1, Serial/Modem Later

## Status

Accepted

## Context

Telnet support is enough to create a useful v1. Physical modem support is interesting but adds serial, modem, line-state, and hardware complexity.

## Decision

v1 supports telnet only.

Design the session layer around a transport abstraction so serial/modem support can be added later.

## Consequences

- v1 remains achievable.
- Future serial support is not blocked.
- All session logic must avoid assuming TCP-specific behavior.
