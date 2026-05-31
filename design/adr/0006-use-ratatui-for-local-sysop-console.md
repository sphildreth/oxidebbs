# ADR 0006: Use Ratatui for Local Sysop Console

## Status

Proposed

## Context

The sysop console is a local terminal UI, unlike the remote caller UI.

## Decision

Use Ratatui with Crossterm for the local sysop/admin console.

## Consequences

- Modern local dashboard experience.
- Avoids mixing caller ANSI rendering with local admin UI.
- Ratatui dependency should stay out of remote caller rendering.
