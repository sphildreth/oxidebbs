# ADR 0005: Isolate Door Runner Subsystem

## Status

Accepted

## Context

DOS door execution will be one of the hardest parts of OxideBBS. It involves drop files, runtime directories, DOSBox/DOSEMU behavior, time limits, I/O bridging, and cleanup.

## Decision

Implement door execution in a dedicated `oxidebbs-door` crate.

## Consequences

- Door complexity is contained.
- Multiple runners can be supported over time.
- Core session logic remains cleaner.
- Door tests can be developed independently.
