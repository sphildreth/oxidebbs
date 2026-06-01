# Changelog

All notable changes to OxideBBS will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added Oxide-owned DOS test door documentation and example configuration:
  `oxide-check` (`OXIDECHK.EXE`) in `config/doors.example.toml` and
  disabled by default in `config/oxidebbs.example.toml` to avoid requiring DOSEMU2
  for untouched setups.
- Added live caller door launching from the configured `Doors` menu, including
  enabled-door listing, key/number selection, selected-door validation, drop-file
  generation, `door_started`/`door_finished`/`door_timed_out` audit events, and
  `door_runs` lifecycle updates.
- Added runtime submenu menu navigation for configured `submenu` menu entries so
  callers can move into nested menus without placeholder fallback text.
- Added a server-side interactive door bridge that forwards caller bytes through
  a run-local DOSEMU2 COM1 PTY bridge, enforces timeouts, handles sysop
  disconnects, cleans node runtime directories, and reports live nodes as
  `in_door` while active.
- Added graceful server lifecycle handling in `oxidebbs-server::run`: startup and
  shutdown signals now stop listener acceptance and request active caller disconnects
  before completing shutdown.
- Added an end-to-end telnet runtime smoke test covering new-user creation,
  logoff, shutdown, and lifecycle audit records.
- Added `db import --format json <path>` as a full, schema-validated restore into
  schema-only DecentDB targets, with transactional insertion and FK-aware load
  ordering.
- Documented and enforced `db compact` explicit unsupported behavior in this
  release because DecentDB exposes no safe production compaction API.
- Added a documentation-first Oxide Door Check validation flow (`doors check`,
  `doors dropfile`, and `doors test --dry-run`) and live testing guidance via DOSEMU2
  for the caller `Doors` menu path.
- Clarified v1 door test modeling to validate `OXIDECHK.EXE` through a
  per-door DOSEMU2 `COM1` PTY bridge using `$_com1 = "pts .../OXCOM1.PTY"` and
  COM1 UART-style I/O.
- Added an opt-in `scripts/test-oxide-door-dosemu2.sh` smoke test for maintainers
  that drives `OXIDECHK.EXE` over the DOSEMU2 COM1 PTY bridge and verifies
  `OXIDECHK.RPT` creation.
- Added ADRs and a phased refactor plan documenting DOSEMU2 as the v1 DOS door
  runtime.

### Changed

- Extracted sysop CLI command handlers into `oxidebbs-server::commands`
  modules as a no-behavior-change structural refactor before live control
  socket work.
- Added DecentDB schema migration support in `oxidebbs-db` for upgrade path
  `2 -> 3`, including `message_areas.enabled` backfill, preserved messages and
  replies, schema-2 archive tables for DecentDB's self-referential table
  limitation, and marker update.
- Added a local Unix-domain control socket at `runtime/oxidebbs-control.sock`
  for live `status`, `nodes list`, `nodes show`, `nodes disconnect`,
  `nodes message`, and `nodes broadcast` operations from `oxidebbs-server`.
- Completed Phase 7 documentation/runbook coverage for setup flow, config
  validation, local control socket startup/recovery, node status/messaging, door
  dry-run/live launch behavior, backup/export/import/compact semantics, and
  schema migration workflow.
- `status` and `nodes` now prefer live runtime state from the running server and
  fall back to offline DecentDB-derived data when the control socket is
  unavailable.
- Live node disconnects, direct sysop messages, and broadcasts are now queued
  through the running server and consumed by active caller sessions.
- Added authoritative live node runtime states, heartbeat ages, stale detection,
  and live `nodes reset-stale` handling through the control socket.
- Door run finalization now persists byte counters, and door command planning
  uses the configured runner executable instead of hard-coding a runner.
- Hardened CLI automation by normalizing `--json` output for:
  `status`, `users list`, `nodes list`, `messages areas list`, `doors list`, and
  `db stats` into stable top-level objects.
- Emitted lifecycle observability events to `audit_log` for server start/stop,
  startup config load, node assignment, and database-write failures.
- Updated the GitHub Pages workflow to self-enable Pages during deployment
  bootstrap and support an optional `GITHUB_PAGES_TOKEN` for first-run setup.
- Added phase-6 hardening checks for top-level help ordering, non-interactive
  setup `--data` override behavior, config check on
  `config/oxidebbs.example.toml`, unsupported `db import` formats, and
  unsupported `db compact` behavior.
- Documented that the bundled test door fixture is a Free Pascal `i8086-msdos`
  `OXIDECHK.EXE` project fixture verified by `tools/doors/oxide-door-check/SHA256SUMS`,
  and that maintainers must bootstrap/build scripts only when changing
  `oxidechk.pas`.
- CLI `doors test <key> --user <alias>` now requires `--dry-run`; live
  interactive DOS door validation runs through a real caller session so the
  COM1 serial bridge is exercised.
- Clarified that normal Rust build and validation remain independent of DOSEMU2 and
  maintainer-only Free Pascal rebuild tooling.
- Clarified user-facing door documentation with the exact telnet-to-OxideBBS-to-
  DOSEMU2-COM1 byte path used by live DOS door sessions.
- Live DOSEMU2 door runs now receive container-safe runtime settings
  (`$_cpu_vm = "emulated"`, `$_cpu_vm_dpmi = "emulated"`, sound disabled, and
  network packet/TCP drivers disabled) in the generated per-node
  `OXDOSEMU2.CONF`.
- The optional DOSEMU2 smoke test now detects legacy `dosemu-1.x` and skips with
  a clear message instead of timing out while waiting for a COM1 PTY path that
  legacy DOSEMU cannot create.
- DOSEMU2 door command planning now stages the configured DOS executable into
  the node runtime directory and launches it by DOS filename so drop files and
  report files stay in the per-node runtime. Headless DOSEMU2 launches keep
  stdin open to avoid `-dumb` keyboard EOF shutdown.

### Removed

- Removed the legacy top-level `admin` CLI alias group during early development;
  use the direct top-level sysop command groups instead.
- Removed the temporary DOSBox runner, serial TCP bridge, smoke script, and
  headless wrapper before v1 in favor of DOSEMU2-only DOS door execution.

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
