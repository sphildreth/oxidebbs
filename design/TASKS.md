# OxideBBS Initial Task List

This is intended for a local coding agent to work from.

## Phase 1 — Make the skeleton real

- [x] Confirm workspace crate names.
- [x] Add real package metadata to each crate.
- [x] Add common dependencies.
- [x] Add CI workflow.
- [x] Add VitePress documentation site.
- [x] Add `cargo fmt`, `cargo clippy`, and `cargo test` commands to scripts.
- [x] Add a minimal server binary that loads config and logs startup.

## Phase 2 — Configuration

- [x] Define `OxideConfig`.
- [x] Parse TOML config.
- [x] Validate paths.
- [x] Validate node count.
- [x] Validate telnet bind address.
- [x] Add `oxidebbs-server --config config/oxidebbs.example.toml check`.
- [x] Add interactive setup command for new systems.

## Phase 3 — Terminal

- [x] Add ANSI writer helper.
- [x] Add CP437 byte/string helper.
- [x] Add screen asset loader.
- [x] Add 40-column screen asset selection.
- [x] Add plain text fallback renderer.
- [x] Add width-aware menu/status/pager layout tests.
- [x] Add tests for CP437 round-trip cases.
- [x] Add tests for ANSI escape generation.
- [x] Add ANSI escape sequence parser.

## Phase 4 — Telnet

- [x] Define `Transport` trait.
- [x] Add telnet listener and `serve` runtime.
- [x] Add telnet parser for IAC sequences.
- [x] Support basic WILL/WONT/DO/DONT.
- [x] Add terminal type negotiation later.
- [x] Add NAWS/window-size support later.
- [x] Add node assignment and idle timeout handling.
- [x] Add clean disconnect handling.
- [x] Add session lifecycle logging.
- [x] Add integration test with loopback transport.

## Phase 5 — DecentDB

- [x] Add DecentDB dependency/path.
- [x] Add `oxidebbs-db` repository traits.
- [x] Add database open/init routine.
- [x] Add schema version record.
- [x] Add sequential schema migration runner.
- [x] Add schema `2 -> 3` migration for `message_areas.enabled`.
- [x] Add user repository.
- [x] Add audit event repository.
- [x] Add test database fixture.
- [x] Use DecentDB-native UUID, TIMESTAMPTZ, IPADDR, BOOL, foreign-key, and
  CHECK constraints in the starter schema.

## Phase 6 — Users and menus

- [x] Add user model.
- [x] Add password hashing decision ADR.
- [x] Add new user flow.
- [x] Add login flow.
- [x] Add menu model.
- [x] Add menu command router.
- [x] Add basic main menu.

## Phase 7 — Doors

- [x] Add door definition model.
- [x] Parse `doors.toml`.
- [x] Add node runtime directory handling.
- [x] Add DOOR.SYS generation.
- [x] Add DORINFO1.DEF generation.
- [x] Add dry-run door test.
- [x] Add DOSEMU2 runner.
- [x] Add timeout and disconnect cleanup.
- [x] Record door runs in DecentDB.

## Phase 8 — Messages

- [x] Add message area model.
- [x] Add message model.
- [x] Add post message command.
- [x] Add read message command.
- [x] Add reply command.
- [x] Add private mail foundation.
- [x] Add local-only moderation.

## Phase 9 — Sysop CLI/TUI

- [x] Add local sysop command foundation.
- [x] Add top-level CLI-first sysop command groups.
- [x] Add global config, data-path, JSON, no-color, and verbosity options.
- [x] Add setup, check, serve dry-run, and status commands.
- [x] List users.
- [x] Show/add/rename/disable/promote/demote users.
- [x] Reset password.
- [x] List nodes.
- [x] Show node state and record audited node control intents.
- [x] Add message area and moderation commands.
- [x] Add door list/show/check/test/dropfile/run-history commands.
- [x] Add ANSI screen list/show/validate/preview/convert/inspect commands.
- [x] Add DecentDB init/doctor/stats/backup/verify/export commands.
- [x] Add logs, audit, and config inspection commands.
- [x] Show recent calls.
- [x] Test door config.
- [x] Prototype Ratatui sysop console.
- [x] Extract sysop CLI command handlers into `oxidebbs-server::commands`
  modules before adding live server control behavior.
- [x] Add live server control socket for node disconnect/message/broadcast delivery.
- [x] Add authoritative live node states, heartbeat ages, stale detection, and
  live stale-node reset.
- [x] Wire the caller `Doors` menu to enabled door selection and live
  child-process bridging.
- [x] Record live door launch lifecycle, byte counts, timeout cleanup, and
  `in_door` node state.
- [x] Specify DecentDB restore and compaction semantics.
- [x] Enable schema-validated JSON restore for `db import --format json`.
- [x] Keep `db compact` explicitly unsupported until DecentDB exposes a safe compaction API.
- [x] Stabilize successful `--json` object contracts for `status`,
  `users list`, `nodes list`, `messages areas list`, `doors list`, and
  `db stats`.
- [x] Add CLI hardening tests for top-level help order, non-interactive
  setup with `--data`, example-config checking, unsupported import formats, and
  unsupported compaction.

## Phase 10 — FTN/OxideNet design

- [x] Add ADR for FTN abstraction.
- [x] Define network address model.
- [x] Define echomail area mapping.
- [x] Define netmail model.
- [x] Define duplicate detection approach.
- [x] Define packet import/export boundary.

## Oxide Door Check

- [x] Add Oxide-owned `OXIDECHK.EXE` test fixture metadata references to docs and
  design docs.
- [x] Add Free Pascal `src/oxidechk.pas` source and checked-in
  `dist/OXIDECHK.EXE` conformance fixture.
- [x] Add `SHA256SUMS` verification for the checked-in executable fixture.
- [x] Add maintainer-only `scripts/bootstrap-fpc-i8086-msdos.sh` and
  `scripts/build-oxidechk-door.sh` rebuild flow.
- [x] Reference `oxide-check` in example, main, and setup-generated
  configuration files.
- [x] Document `doors check/dropfile/test` smoke-test workflow in setup,
  getting-started, deployment, and sysop CLI docs.
- [x] Document DOSEMU2 runtime directory execution, COM1 PTY mapping, and
  `OXNODE.TXT` diagnostics.
- [x] Convert `OXIDECHK.EXE` and live caller launch to validate COM1 serial I/O
  through a run-local DOSEMU2 COM1 PTY bridge instead of DOS console/stdout.
- [x] Add container-safe DOSEMU2 runtime config for live DOS door execution.
- [x] Update changelog with the user-facing test-door documentation and config
  behavior.
- [x] Add optional DOSEMU2 smoke script that is skipped unless explicitly run
  interactively.
- [x] Keep normal Cargo build/test independent of Free Pascal, DOSEMU2, and the
  staged i8086/MS-DOS cross toolchain.

## DOSEMU2 Door Runtime Refactor

- [x] Add `design/DOSBOX_TO_DOSEMU2_REFACTOR_PLAN.md` to define the phased
  replacement of the temporary DOS runtime with DOSEMU2.
- [x] Add ADR 0010 selecting DOSEMU2 and its COM1 PTY backend as the long-term
  v1 DOS door runtime.
- [x] Add ADR 0011 documenting that the temporary runner should be removed
  before v1 instead of maintained as a parallel runner.
- [x] Document the Debian 13 LXC validation decision and keep live DOSEMU2
  validation opt-in until a target LXC host is available.
- [x] Replace temporary command planning and serial bridge code with DOSEMU2
  command planning and COM1 PTY bridging.
- [x] Convert Oxide Door Check documentation and optional smoke testing to
  DOSEMU2.
- [x] Remove temporary runner scripts, config defaults, and user-facing
  documentation.

## Implementation Plan Phase 7 — Documentation And Runbook Completion

- [x] Update docs for setup/validation/startup and local control flow in
  `docs/project/setup.md`, `docs/project/getting-started.md`, and
  `docs/project/deployment.md`.
- [x] Update sysop command documentation for local control socket, node control,
  door dry-run/live behavior in `docs/project/sysop-cli.md` and
  `design/OxideBBS_SYSOP_INTERFACE.md`.
- [x] Update operational runbook and schema semantics in `design/RUNBOOK.md` and
  `design/DECENTDB_SCHEMA.md`.
- [x] Update `design/SPEC.md` with local-only control-surface constraints.
- [x] Update changelog with phase-7 documentation completion and operational
  behavior notes in `docs/about/changelog.md`.

## V1 Release-Candidate Hardening

- [x] Fix `.github/workflows/pages.yml` to self-enable Pages when disabled using
  `actions/configure-pages@v5` with `enablement: true`; verify this in the
  target run `https://github.com/sphildreth/oxidebbs/actions/runs/26764478252`
  by eliminating the `enablement: false` bootstrap failure.
  - Configure an optional `GITHUB_PAGES_TOKEN` secret (PAT with repo/pages/admin
    scope) for first-run enablement on repositories with no existing Pages site.
  - Fall back to `github.token` for already provisioned repos.

- [x] Implement V1 runtime hardening for graceful shutdown and lifecycle
  observability:
  - Add startup/shutdown signal handling in `oxidebbs-server::serve`.
  - Emit `config_loaded`, `server_start`, `server_stop`, `node_assigned`,
    `db_write_failed` audit events where feasible.
  - Ensure active node tasks receive disconnect requests before shutdown exits.
- [x] Implement configured `submenu` runtime behavior and remove the visible
  caller placeholder.
- [x] Add end-to-end telnet/runtime smoke coverage for connect, new-user
  creation, logoff, shutdown, and lifecycle audit records.
- [x] Reconcile roadmap/spec v1 readiness items: line input is implemented,
  direct repository writes are documented as the v1 write model, and dedicated
  welcome/logoff screen rendering is deferred beyond v1.

## Docker Cross-Platform Deployment

- [x] Add a Linux Docker image that builds `oxidebbs-server` and includes
  bundled assets, config examples, scripts, the checked-in `OXIDECHK.EXE`
  fixture, and DOSEMU2.
- [x] Add Docker Compose with named volumes for config, DecentDB data, doors,
  logs, and runtime files.
- [x] Add a first-boot container entrypoint that runs non-interactive setup,
  seeds the sysop account, validates config, and enables the bundled
  `oxide-check` door for Docker evaluation.
- [x] Add ADR 0012 documenting Docker as the cross-platform deployment path
  while keeping OxideBBS a single Linux runtime target.
- [x] Add user-facing Docker deployment documentation covering first boot,
  Windows/macOS usage, volumes, resets, sysop commands, and door smoke tests.
