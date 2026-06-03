# ADR 0029: Remote Admin Security Model

## Status

Accepted

## Context

OxideBBS has been local-admin-first through CLI, TUI, and a Unix control socket.
Docs warned that any future web admin interface must include CSRF and replay
protection before it is enabled.

v1.2 includes the remote admin/status surface.

## Decision

Remote admin is disabled by default and uses a separate bind address from
telnet.

The remote surface must provide:

- authenticated sessions
- Argon2id-hashed admin credentials or a reviewed equivalent secret store
- secure, HttpOnly, SameSite cookies
- per-form or per-request CSRF tokens for browser-originating mutations
- replay protection for API mutations using nonce and timestamp validation
- request audit logging
- rate limiting for login and mutation attempts
- explicit read-only mode
- clear separation between public status, authenticated status, and mutations

The first implementation may use reverse-proxy TLS termination, but docs must
state that credentials and session cookies must not be sent over plaintext
networks.

## Consequences

- Remote admin cannot be accidentally enabled by installing v1.2.
- Security behavior is part of acceptance testing, not optional polish.
- The local Unix control socket remains available for local CLI/TUI workflows.
