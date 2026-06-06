# ADR 0001: Use Rust for OxideBBS

## Status

Accepted

## Context

OxideBBS is a systems-heavy application: telnet sessions, byte-oriented terminal rendering, process orchestration, node management, serial/modem support, and embedded database usage.

## Decision

Use Rust as the primary implementation language.

## Consequences

- Strong memory-safety guarantees.
- Excellent fit for byte-oriented protocol work.
- Good fit for a single-binary server.
- Natural fit for DecentDB.
- Requires careful async design.
