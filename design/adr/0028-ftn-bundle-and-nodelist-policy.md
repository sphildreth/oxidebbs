# ADR 0028: FTN Bundle Compression And Nodelist Update Policy

## Status

Accepted

## Context

The FTN plan listed ZIP bundle support as required and ARJ support as deferred.
It also listed full nodelist support as required and differential nodelist
updates as deferred. v1.2 includes deferred work.

## Decision

v1.2 supports:

- raw `.pkt` processing
- ZIP bundle creation and extraction
- ARJ bundle creation and extraction
- full nodelist import
- nodelist differential update import

Compression implementations must be safe and deterministic. Prefer reviewed
Rust crates. Do not shell out to arbitrary configured commands for packet
compression or extraction.

Full nodelist import remains the base operation. Differential imports apply to
an already imported full nodelist and must fail clearly when the base nodelist
is missing or mismatched.

## Consequences

- Legacy FTN networks that still require ARJ and nodediff workflows are in
  v1.2 scope.
- Bundle and nodelist code needs fixture coverage for corrupt archives, invalid
  diffs, and idempotent rebuilds.
