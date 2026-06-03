# Changelog

All notable changes to OxideBBS will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - 2026-06-03

### Added

- Added a GitHub release-artifact workflow that builds and uploads Linux,
  macOS, and Windows `oxidebbs-server` packages plus SHA-256 checksums when a
  release is published, with manual dispatch support for existing releases.
- Added a sysop-facing caller command reference covering current default menu
  keys, door/message prompt commands, supported menu actions, and future/reserved
  command notes.
- Added a sysop-facing user security levels reference covering current defaults,
  enforced message-area gates, sysop promotion behavior, and currently reserved
  or unenforced security-level fields.
- Added `[logging]` config with file logging to `paths.logs`, plus
  `serve --log-level` and global `-v` overrides for sysop-controlled verbosity.
- Added configurable log file format and rotation, including newline-delimited
  JSON logs and daily or size-based archive retention.
- Added the local Ratatui sysop TUI launched by `oxidebbs-server sysop`,
  including dashboard, node, user, door, message, database, log, config, ANSI,
  audit, command-palette, modal, and read-only views backed by the same sysop
  service layer as the CLI.
- Added selectable sysop TUI themes via `sysop --theme`, including
  `oxide-classic`, `wildcat`, `telegard`, `vbbs`, `mystic`, `midnight`, and
  `high-contrast`, with sysop-facing theme documentation and visual examples.
- Added a sysop TUI `Doctor` screen with verbose pass/warn/fail checks for the
  database path, schema, required tables, users, messages, sessions, doors,
  audit events, auth-attempt state, and per-finding remediation text.

### Changed

- Removed the starter registration option from the post-login main menu and
  bundled welcome/main-menu assets so account creation is advertised only from
  the login screen.
- Removed command lists from starter welcome assets; command prompts now live on
  the active login and main menu screens only.
- Updated starter welcome art to show the `v1.1` release line instead of the
  original development-era version label.
- Updated GitHub workflow action versions for checkout and Node setup to current
  Node 24-backed majors.

### Fixed

- Sysop TUI mode now suppresses process console logging while Ratatui owns the
  terminal, preventing embedded `serve` logs from overwriting the interface.
- The sysop TUI control client now uses the server's newline-delimited JSON
  control protocol for socket probes and node actions, avoiding repeated
  `Broken pipe` control warnings from partial availability checks.
- The sysop TUI command palette now renders a visible overlay and F2 toggles it
  closed, fixing the apparent freeze after opening Help with F1 and then
  pressing F2.
- Sysop TUI F3 now opens filter/search controls on searchable screens and shows
  an explicit notice on screens without filtering; F5 now refreshes data with
  visible status-bar feedback.
- Sysop TUI `Q` now opens a quit confirmation dialog by default, supports
  `[sysop].confirm_quit = false` and `sysop --no-confirm-quit` for no-caller
  sessions, and always warns with `Nodes are active. Continue to shutdown?`
  when active nodes are present.
- Directory-valued DecentDB paths now resolve to the default `oxidebbs.ddb`
  file inside that directory, so `database.path = "/path/to/data/"` opens
  `/path/to/data/oxidebbs.ddb` instead of trying to open the directory itself.
- `config check` now reports missing configured terminal welcome/logoff assets,
  reports missing assets for each configured menu screen with menu context, and
  `serve` now logs and prints the asset plus terminal profile whenever it falls
  back because a configured terminal or screen asset cannot be loaded.
- DecentDB schema initialization now uses DecentDB catalog metadata instead of
  non-DecentDB metadata SQL, preventing valid DecentDB files from failing
  OxideBBS startup or CLI commands during schema detection.
- `serve` and `serve --dry-run` now fail before listening when required DecentDB
  startup reads fail, and live startup now requires `config_loaded` and
  `server_start` audit events to be writable instead of silently continuing.
- `db stats` and offline `status`/`nodes` fallbacks no longer report stale
  unclosed session rows as active live sessions when the server is not running.
  `db stats` now keeps those rows visible as `open_sessions` diagnostics.
- Screen and terminal assets now normalize bare line feeds to CRLF before
  sending bytes to telnet callers, preventing plain clients from rendering each
  line farther to the right.
- Starter login ANSI screen assets no longer clear the screen after the terminal
  welcome asset is sent, so SyncTerm and other ANSI callers can see the welcome
  art and login menu together.
- `logs recent`, `logs tail`, and `logs search` now read the server log and
  nested door runner logs under `paths.logs`.
- Stock telnet client CR-NUL line endings are now treated as line endings
  instead of leaking NUL bytes into aliases, passwords, or subsequent prompts.
- Sysop TUI confirmations now execute their backing actions, use the live
  `oxidebbs-control.sock` path, and audit user, door, message, broadcast, and
  live-node write actions.
- `oxidebbs-server sysop` now attaches to an existing live control socket when
  available and otherwise starts an embedded `serve` runtime for the TUI session;
  `--connect-only` preserves client-only behavior.
- Generated configs and release archives now place the bundled `oxide-check`
  door fixture under `./doors/oxide-door-check/dist`, and setup installs that
  fixture when it includes the example door definition. This keeps the example
  door inside `paths.doors` so `doors check oxide-check` satisfies the door
  containment policy.
- `setup --no-sample-ansi` no longer creates empty starter asset directories.

### Compatibility Notes

- The OxideBBS Rust workspace crate versions are aligned at `1.1.0`.
- No DecentDB schema version bump is required for this release; schema `4`
  remains the current v1 release-line schema.
- Existing `v1.0.0` board configs remain valid. The `v1.1.0` example config
  includes `[logging]` and `[sysop]` settings used by file logging, rotation,
  and the local sysop TUI.
- The local sysop TUI may start an embedded `serve` runtime when no live control
  socket is reachable. Use `sysop --connect-only` when the TUI must attach only
  to an already-running server.
- Directory-valued `database.path` values now resolve to `oxidebbs.ddb` inside
  the directory instead of attempting to open the directory path itself.
- Startup now fails before listening when required database reads or startup
  audit writes fail, making database problems visible before callers connect.
- Release archives are produced by the GitHub release workflow and should be
  accompanied by SHA-256 checksum files. Archives include both the runnable
  `doors/oxide-door-check` fixture and the source fixture under `tools/doors`.

## [1.0.0] - 2026-06-02

### Added

- Added persistent authentication abuse controls backed by DecentDB
  `auth_attempts`, with per-IP and per-alias lockouts, explicit `[auth]`
  Argon2id defaults, configurable new-user security level, and identical
  visible failed-login messaging for missing aliases and wrong passwords.
- Added audit retention configuration (`[audit].retention_days = 365`), a
  DecentDB retention purge helper, and a live `audit_write_failures` runtime
  counter exposed through control status JSON.
- Added focused security/performance review coverage for repeated login-failure
  audit bounding, post-staging door runtime cleanup, CP437 caller input
  rejection, and message visibility filtering.
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
- Added Docker and Docker Compose deployment support for a Linux runtime image
  that includes `oxidebbs-server`, DOSEMU2, bundled assets, and the checked-in
  `OXIDECHK.EXE` test door fixture.
- Added ADR 0012 documenting Docker as the cross-platform deployment path for
  Windows, macOS, and Linux hosts while keeping OxideBBS a single Linux target.
- Added Docker deployment documentation covering first boot, named volumes,
  Windows/macOS usage, sysop commands, and DOSEMU2 door smoke tests.
- Docker Compose now uses `OXIDEBBS_HOST_TELNET_PORT` for host-port publishing
  so the container listener remains fixed at `2323`.

### Changed

- Changed the default telnet bind in generated and example configs to
  `127.0.0.1:2323`; public telnet binds now produce a plaintext exposure
  warning during config checks.
- Hardened the Unix control socket with `0700` runtime directories, `0600`
  socket permissions, same-UID peer checks, and a background stale-node sweeper
  that requests disconnects with reason `stale_node_timeout`.
- Hardened door execution by enforcing runner allowlists, runner ownership/mode
  checks, working-directory containment under `paths.doors`, a `1..=240` minute
  time-limit cap, per-node runtime directory guards, `0600` log files, and
  child-process reap behavior after termination.
- Bumped the pre-alpha DecentDB schema marker to `4`, adding
  `users.alias_normalized` for case-insensitive alias uniqueness and
  persistent `auth_attempts` lockout state.
- New-user alias creation now uses the atomic
  `insert_user_if_alias_available` repository path instead of a pre-check plus
  insert sequence.
- Runtime audit inserts now generate UUIDs and timestamps inline in DecentDB
  `INSERT` statements; import uses a preserving insert path for backup fidelity.
- Caller-authored, remotely rendered text is rejected before storage when it is
  not CP437-compatible, while password input remains hidden and exempt.
- Telnet transport now buffers socket reads in 4096-byte chunks, batches telnet
  negotiation replies, completes capability negotiation early when terminal type
  and NAWS are known, and removes the fixed menu-newline drain delay.
- Message reads now use SQL-side visibility filtering, one alias map per
  displayed message list, cached enabled message areas per message-flow entry,
  and a shared `Arc<OxideConfig>` instead of per-connection deep config clones.
- Completed the 2026-06-02 security/performance re-review follow-ups, including
  same-UID control-socket documentation, runtime directory cleanup guidance,
  immediate door runner revalidation before spawn, runtime-aware audit failure
  counting, expired auth lockout cleanup, and caller-safe door launch error text.
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
- Emitted lifecycle observability events to audit events for server start/stop,
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
- Added Fedora-specific DOSEMU2 setup documentation covering the `stsp/dosemu2`
  Copr package set, legacy `dosemu-1.x` detection, the `libdj64.so.0`
  loader-cache requirement, and the Oxide Door Check smoke tests.
- Added non-interactive setup support for `--enable-example-door`, used by
  Docker first boot to enable the bundled `oxide-check` door without requiring
  ad hoc TOML editing.
- Hardened the optional DOSEMU2 smoke script so child-originated hangup signals
  do not terminate sequential multi-node checks inside Docker.
- `setup` and `ansi install-defaults` now install bundled ANSI/screen assets
  without overwriting customized files, and `check` warns when Unix runtime
  directory permissions weaken local control-socket isolation.

### Fixed

- Plain telnet callers now receive terminal-level `.asc`/`.txt` welcome assets
  when available instead of stripped CP437 ANSI art that common telnet clients
  render as mojibake.
- Default welcome and main-menu assets now show all configured starter menu
  keys, and the default main-menu goodbye/logoff action uses `[G] Goodbye`
  consistently with the login menu.

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
