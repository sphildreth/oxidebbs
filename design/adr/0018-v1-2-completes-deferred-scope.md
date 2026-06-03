# ADR 0018: Treat v1.2 As The Deferred-Scope Release

## Status

Accepted

## Context

The v1.1.0 release plan and task tracker intentionally deferred caller-flow,
door, database, transport, file-transfer, FTN, OxideNet, remote-admin, native
door, and repository automation work.

The v1.2 release is now defined as the version that includes every feature that
existing documentation describes as deferred, future, later, reserved, or a
post-v1 candidate.

## Decision

v1.2 is a consolidation release that pulls all documented deferred and future
feature work into active release scope.

The release must be implemented in phases. A phase may be marked complete only
when its behavior, tests, documentation, and validation commands pass. A feature
must not remain labeled as future after the phase implementing it is complete.

If an external dependency prevents completion, such as DecentDB lacking a safe
compaction API, that phase must be marked blocked in the release plan with the
exact dependency named. The code must not fake support or leave an unsupported
stub while claiming v1.2 completion.

## Consequences

- v1.2 is intentionally larger than a normal minor release.
- Agents implementing v1.2 must work from `design/RELEASE_v1_2_PLAN.md` and the
  phase-specific ADRs rather than re-opening scope decisions.
- Docs, examples, and changelog entries must be updated as phases complete so
  shipped documentation no longer advertises implemented work as future.
- The CI gate remains `./scripts/dev-check.sh`.
