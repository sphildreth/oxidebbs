# Changelog

All notable changes to OxideBBS will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Extracted sysop CLI command handlers into `oxidebbs-server::commands`
  modules as a no-behavior-change structural refactor before live control
  socket work.
- Added a local Unix-domain control socket at `runtime/oxidebbs-control.sock`
  for live `status`, `nodes list`, `nodes show`, `nodes disconnect`,
  `nodes message`, and `nodes broadcast` operations from `oxidebbs-server`.
- `status` and `nodes` now prefer live runtime state from the running server and
  fall back to offline DecentDB-derived data when the control socket is
  unavailable.
- Live node disconnects, direct sysop messages, and broadcasts are now queued
  through the running server and consumed by active caller sessions.

### Removed

- Removed the legacy top-level `admin` CLI alias group during early development;
  use the direct top-level sysop command groups instead.

## [0.2.0] - 2026-06-01

### Added

- Added top-level CLI-first sysop command groups for setup, check, status,
  users, nodes, messages, doors, ANSI screens, DecentDB maintenance, logs,
  audit, config, and the local sysop console preview.
- Added global `--data`, `--json`, `--no-color`, and repeatable `--verbose`
  options alongside the existing `--config` option.
- Added non-interactive setup flags, DecentDB initialization during setup,
  initial sysop account creation, and default local message-area seeding.
- Added DecentDB repository helpers for sysop user updates, message area
  enabled/level changes, message lookup/move/search support, door enabled
  state, door run lookup, and active-session lookup by node.
- Added a Sysop CLI documentation page covering command groups and current
  operational limits.

### Changed

- Bumped all OxideBBS Rust crate versions to `0.2.0`.
- Bumped the pre-alpha DecentDB schema marker to `3`.
- Added an `enabled` flag to message areas and per-door config definitions.
- Updated setup, getting-started, runbook, schema, and specification
  documentation for the CLI-first sysop interface.

### Compatibility Notes

- Existing development databases with schema marker `2` must be recreated
  before running `0.2.0`.
- Live node disconnect/message/broadcast commands currently record audited
  sysop intent and update ended session rows where possible; live delivery waits
  for a future local server control socket.
- `db import` and `db compact` remain blocked until DecentDB restore and
  compaction semantics are specified.

## [0.1.0] - 2026-05-31

### Added

- Added the initial Rust workspace scaffold with focused crates for server,
  core, terminal, telnet, database, door, and sysop boundaries.
- Added native DecentDB Rust dependency wiring through the released
  `sphildreth/decentdb` `v2.8.0` Git tag, without requiring a local DecentDB
  checkout.
- Added a minimal `oxidebbs-db` wrapper that opens DecentDB, initializes the
  starter schema, and records an OxideBBS schema-version marker.
- Added initial ANSI/CP437 terminal helpers with tests for box-drawing
  conversion, unrepresentable-character errors, and ANSI escape byte output.
- Added 40-column and 80-column terminal profile requirements to the product and
  technical design documents.
- Added VitePress documentation under `docs/`, including GitHub Pages deployment
  support for `https://oxidebbs.com`.
- Added GitHub Actions CI for Rust checks and documentation builds.
- Added the first configurable menu model, safe key-to-action routing, login and
  main menu config, terminal-capability screen selection, and starter
  `assets/screens/` layout.
- Added terminal screen asset loading, plain-text fallback rendering,
  width-aware menu/status/pager helpers, telnet IAC negotiation parsing,
  terminal type and NAWS events, core user/login/message flows, door drop-file
  generation and runners, DecentDB message/session/door repositories, sysop
  admin commands, and FTN/OxideNet domain models.
- Added an interactive `oxidebbs-server setup` command that writes a starter
  board config, prepares local directories, and can include a placeholder door
  definition without bundling door binaries.
- Added the first real `oxidebbs-server serve` runtime with a telnet listener,
  node-slot allocation, DecentDB session/audit records, configured screen
  rendering, starter menu routing, NAWS width updates, idle timeout handling,
  login/new-user authentication, local message reading/posting/replies, and a
  placeholder response for doors.

### Changed

- Reworked the starter DecentDB schema to use native `UUID`, `TIMESTAMPTZ`,
  `IPADDR`, and `BOOL` columns, plus foreign keys, CHECK constraints, and
  relationship indexes.
- Updated the server CLI to prefer setup-generated `config/oxidebbs.toml` when
  no `--config` path is supplied, with the example config as the clean-checkout
  fallback.
- Normalized example door definitions under `[[doors.definitions]]` so door
  settings and door entries share one config namespace without table collisions.
- Updated repository documentation to point at the `design/` document tree and
  the VitePress docs site.
- Updated development checks to use the committed lockfile with Cargo
  `--locked`.

### Fixed

- Added a starter `logoff.ans` asset so the example config references existing
  bundled ANSI files.
