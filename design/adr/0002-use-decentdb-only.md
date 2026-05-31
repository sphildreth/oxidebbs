# ADR 0002: Use DecentDB as the Only System Database

## Status

Accepted

## Context

OxideBBS is intended to dogfood DecentDB and avoid external database dependencies.

## Decision

Use DecentDB as the only system database.

Do not add SQLite, PostgreSQL, MySQL, Redis, MongoDB, or ORM layers.

## Consequences

- Simpler deployment.
- Strong alignment with the author's ecosystem.
- Repository layer must be well-designed.
- External SQL assumptions should be avoided in docs and code.
