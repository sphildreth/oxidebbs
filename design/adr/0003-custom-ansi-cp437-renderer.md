# ADR 0003: Use a Custom ANSI/CP437 Caller Renderer

## Status

Accepted

## Context

A BBS caller UI is not a normal local terminal application. It must preserve CP437 art, ANSI escape behavior, telnet quirks, and classic client expectations.

## Decision

Build a custom byte-oriented ANSI/CP437 renderer for remote callers.

Do not use Ratatui for remote caller UI.

## Consequences

- Better retro fidelity.
- More control over byte streams.
- More implementation work.
- Ratatui remains available for local sysop tooling.
