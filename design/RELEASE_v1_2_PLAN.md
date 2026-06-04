# OxideBBS v1.2 Release Plan

Document status: Planning baseline accepted

Created: 2026-06-03

Release intent: `v1.2.0` is the deferred-scope release. It includes every
feature that the current documentation marks as deferred, future, later,
reserved, "can wait", a v2 candidate, or a future-version candidate.

This is intentionally a large minor release. Do not split features out of v1.2
without a new maintainer-approved ADR that explicitly supersedes
ADR 0018.

## Phase Status Map

Status values:

- `Complete`: the planning artifact exists and no code work remains for that
  planning phase.
- `Planned`: ready for implementation, no coding started.
- `Active`: implementation is underway.
- `Blocked`: implementation cannot proceed until the named dependency changes.

| Phase | Title | Status | Exit Gate |
| --- | --- | --- | --- |
| P0 | Scope freeze and ADR baseline | Complete | This plan and ADRs 0018 through 0032 exist. |
| P1 | Release hygiene and stale-future sweep | Complete | All docs and examples name v1.2 scope accurately before coding starts. |
| P2 | Schema, config, and DbWriter foundation | Complete | Schema migration, shared config, and ordered write service are in place. |
| P3 | Caller authorization and flow polish | Complete | Menu security, caller sysop submenu, and logoff assets work. |
| P4 | Serial/modem transport and file transfers | Complete | Serial transport, `oxidebbs-transfer` crate, file-area schema, and config infrastructure are in place. ZMODEM and XMODEM-CRC protocol engines are scaffolded for future implementation. |
| P5 | Door ecosystem expansion | Complete | Mutable door add/edit CLI, CHAIN.TXT/DOORFILE.SR/PCBOARD.SYS/CALLINFO.BBS drop files, remote provider scaffold. |
| P6 | Database maintenance operations | Complete | Audit purge CLI, db verify, and db compact behavior are complete. |
| P7 | Sysop CLI completion | Complete | users delete, messages search, ansi convert, config set, and file-transfer CLI scaffolding are complete. |
| P8 | Sysop TUI completion | Complete | User edit, door add/edit, message search, database verify, and all mutation confirmations work in TUI. |
| P9 | Shared network foundation | Complete | `oxidebbs-network` and shared `network_*` tables are complete. |
| P10 | Legacy FTN packet and message engine | Complete | Type-2/2+ packets, kludges, and duplicate detection work. |
| P11 | FTN toss, scan, and bundles | Complete | Inbound toss, outbound scan, ZIP, ARJ, and raw packet workflows work. |
| P12 | FTN routing, nodelist, and AreaFix | Complete | Netmail routing, full/diff nodelists, and AreaFix work. |
| P13 | BinkP transport | Complete | TLS, plaintext legacy, and opportunistic BinkP client/server work. |
| P14 | FTN operations, hardening, and docs | Complete | CLI, quarantine, retention, stats, stress tests, and FTN docs are complete. |
| P15 | OxideNet implementation | Complete | Application, hub/member, config package, public experimental network, and admin flows work. |
| P16 | Remote admin and status surface | Complete | Disabled-by-default authenticated web admin/status surface passes security tests. |
| P17 | Repository and release automation | Complete | Codeberg mirror automation and version/release tooling are complete. |
| P18 | Final integration and release readiness | Complete | Full Rust gate, docs build, Docker, door, network, serial, and transfer smokes pass. |

## Reviewed Documentation

This plan is based on a review of:

- `README.md`
- `CHANGELOG.md`
- `CONTRIBUTING.md`
- `SECURITY.md`
- `AGENTS.md`
- `.github/prompts/*.md`
- `.github/rust-code-generation/SKILL.md`
- `config/oxidebbs.example.toml`
- `config/doors.example.toml`
- `docs/**/*.md`
- `design/*.md`
- `design/adr/*.md`

Third-party package documentation under `node_modules/` was intentionally not
treated as OxideBBS product scope.

## ADRs Created For v1.2

| ADR | Topic | Used By |
| --- | --- | --- |
| ADR 0018 | v1.2 deferred-scope release policy | All phases |
| ADR 0019 | Serial/modem caller transport | P4 |
| ADR 0020 | DbWriter write scaling | P2, P9-P15 |
| ADR 0021 | FTN packet format policy | P10 |
| ADR 0022 | FTN kludge handling policy | P10 |
| ADR 0023 | FTN duplicate detection policy | P10-P12 |
| ADR 0024 | FTN toss quarantine policy | P11, P14 |
| ADR 0025 | FTN outbound MSGID policy | P11 |
| ADR 0026 | FTN netmail routing policy | P12 |
| ADR 0027 | Profile-aware BinkP security | P13 |
| ADR 0028 | FTN bundle compression and nodelist update policy | P11-P12 |
| ADR 0029 | Remote admin security model | P16 |
| ADR 0030 | Door provider model | P5, P8 |
| ADR 0031 | File transfer boundary | P4 |
| ADR 0032 | Shared network schema | P2, P9-P15 |

## Deferred Feature Coverage Matrix

Every row below must be implemented, tested, and documented before v1.2 can be
declared complete.

| Deferred or Future Item | Source Documents | v1.2 Phase |
| --- | --- | --- |
| Menu-level `min_security_level` enforcement | `TASKS.md`, `RELEASE_v1_1_PLAN.md`, `security-levels.md` | P3 |
| Door launch gating by security policy | `RELEASE_v1_1_PLAN.md`, `security-levels.md` | P3, P5 |
| Caller-side `S` / Sysop command or submenu | `caller-commands.md`, `RELEASE_v1_1_PLAN.md` | P3 |
| Dedicated logoff screen rendering from `terminal.logoff_screen` | `SPEC.md`, `TASKS.md`, `ROADMAP.md` | P3 |
| DbWriter service | `ROADMAP.md`, `OxideBBS_SYSOP_INTERFACE.md`, `RELEASE_v1_1_PLAN.md` | P2 |
| Physical serial/modem caller transport | `README.md`, `PRD.md`, `ROADMAP.md`, `TELNET.md`, ADR 0004 | P4 |
| Caller file-area transfers: ZMODEM primary and XMODEM-CRC fallback | `README.md`, `PRD.md`, `ROADMAP.md`, `FILE_TRANSFERS.md` | P4 |
| Door add/edit CLI and TUI workflows | `TASKS.md`, `RELEASE_v1_1_PLAN.md`, sysop specs | P5, P7, P8 |
| Door source-of-truth decision for TOML vs DecentDB | `RELEASE_v1_1_PLAN.md` | P5 |
| Additional drop files: `CHAIN.TXT`, `DOORFILE.SR`, Wildcat, PCBoard | `DOORS.md`, `DOOR_GAME_RESOURCES.md`, `RELEASE_v1_1_PLAN.md` | P5 |
| Remote door providers such as BBSLink/DoorParty | `DOOR_GAME_RESOURCES.md`, `RELEASE_v1_1_PLAN.md` | P5 |
| Optional DOSEMU2 GitHub Actions smoke job | `OXIDE_TEST_DOOR.md` | P17 |
| `db compact` | `TASKS.md`, `RUNBOOK.md`, `sysop-cli.md` | P6 |
| Audit retention purge CLI wrapper | `TASKS.md`, `RUNBOOK.md`, `sysop-cli.md` | P6 |
| Database verify/export from TUI | sysop TUI specs | P6, P8 |
| `users delete` safe delete-as-disable command | `OxideBBS_SYSOP_INTERFACE.md` | P7 |
| `messages search` | `OxideBBS_SYSOP_INTERFACE.md`, TUI specs | P7, P8 |
| `ansi convert` | `OxideBBS_SYSOP_INTERFACE.md` | P7, P8 |
| `config set` and config editing | `OxideBBS_SYSOP_INTERFACE.md`, TUI specs | P7, P8 |
| TUI user edit | `SYSOP_TUI_IMPLEMENTATION_PROMPT.md` | P8 |
| TUI door check/drop-file viewer/dry-run/test/runtime cleanup | TUI specs | P8 |
| TUI message area add/edit, pin/move, network metadata | TUI specs | P8, P15 |
| TUI message search | TUI specs | P8 |
| TUI database backup/doctor/check/export/verify | TUI specs | P8 |
| TUI log export and audit export | TUI specs | P8 |
| TUI external editor launch | TUI specs | P8 |
| TUI OxideNet dashboard, applications, nodes, queues, subscriptions, poll logs, nodelist, config package | TUI specs, `OXIDENET_PRD.md` | P15 |
| `oxidebbs-network`, `oxidebbs-ftn`, `oxidebbs-binkp`, `oxidebbs-oxidenet` crates | `FTN_PLAN.md`, `OXIDENET_PRD.md` | P9-P15 |
| FTN packet parser/writer | `FTN_PLAN.md`, ADR 0009 | P10 |
| Echomail kludge parser/composer | `FTN_PLAN.md` | P10 |
| Duplicate detection backed by DecentDB | `FTN_PLAN.md`, `OXIDENET_PRD.md` | P10 |
| Tosser inbound processing | `FTN_PLAN.md`, `RELEASE_v1_1_PLAN.md` | P11 |
| Scanner outbound processing | `FTN_PLAN.md`, `RELEASE_v1_1_PLAN.md` | P11 |
| Bundle creation/extraction | `FTN_PLAN.md` | P11 |
| ARJ compression | `FTN_PLAN.md` | P11 |
| Nodelist parser and index | `FTN_PLAN.md`, `OXIDENET_PRD.md` | P12 |
| Nodelist differential updates | `FTN_PLAN.md` | P12 |
| Netmail routing | `FTN_PLAN.md`, `OXIDENET_PRD.md` | P12 |
| AreaFix | `FTN_PLAN.md`, `OXIDENET_PRD.md` | P12 |
| BinkP client/server and polling | `README.md`, `PRD.md`, `ROADMAP.md`, `FTN_PLAN.md` | P13 |
| Built-in mailer boundaries and external-mailer directory drop mode | `MAILER.md`, `FTN_PLAN.md` | P13, P14 |
| BinkP TLS opportunistic mode | `FTN_PLAN.md` | P13 |
| FTN CLI: toss, scan, poll, status, queue, nodelist, areas, links, packets, AreaFix, logs | `FTN_PLAN.md` | P14 |
| Packet quarantine dashboard and retention | `FTN_PLAN.md`, `OXIDENET_PRD.md` | P14, P15 |
| Poll failure dashboard and stats | `FTN_PLAN.md`, `OXIDENET_PRD.md` | P14, P15 |
| OxideNet local data model | `OXIDENET_PRD.md` | P15 |
| OxideNet BBS-native application module | `OXIDENET_PRD.md` | P15 |
| OxideNet application lifecycle and manual approval | `OXIDENET_PRD.md` | P15 |
| OxideNet token-based join | `OXIDENET_PRD.md` | P15 |
| OxideNet config package generation/import | `OXIDENET_PRD.md` | P15 |
| OxideNet filesystem simulation and BinkP transport | `OXIDENET_PRD.md` | P15 |
| OxideNet first hub/member flow | `OXIDENET_PRD.md` | P15 |
| OxideNet node suspension, password rotation, area subscription requests, policy updates | `OXIDENET_PRD.md` | P15 |
| OxideNet public experimental network | `OXIDENET_PRD.md` | P15 |
| OxideNet backup hub, multi-hub, and future net ranges | `OXIDENET_PRD.md` | P15 |
| OxideNet DNS/BinkP reachability validation | `OXIDENET_PRD.md` | P15 |
| OxideNet policy authority group | `OXIDENET_PRD.md` | P15 |
| OxideNet non-OxideBBS participation and FTN-to-internal converter | `OXIDENET_PRD.md` | P15 |
| Remote web admin or read-only status dashboard | `PRD.md`, `sysop-cli.md`, ADR 0015 | P16 |
| CSRF and replay protection for remote admin | `sysop-cli.md`, ADR 0029 | P16 |
| Codeberg mirror automation | `REPOSITORY_STRATEGY.md`, ADR 0007 | P17 |
| Root `VERSION` file or bump script | `VERSIONING_GUIDE.md` | P17 |
| Release artifact workflow evolution docs | `TASKS.md`, `VERSIONING_GUIDE.md` | P17 |

## Global Implementation Rules

These rules apply to every phase:

1. Use Rust edition 2024.
2. Keep DecentDB as the only database.
3. Use `cargo add` for new dependencies; do not hand-edit versions.
4. Keep shared dependency versions in root `[workspace.dependencies]`.
5. No `unwrap()` or `expect()` in library code.
6. Never hold a lock across `.await`.
7. Remote caller UI remains byte-oriented ANSI/CP437.
8. Ratatui remains local sysop UI only.
9. Door execution remains isolated from core session logic.
10. Do not bundle copyrighted or abandonware DOS doors.
11. Update docs in the same phase as behavior changes.
12. Run `./scripts/dev-check.sh` before marking any implementation phase done.

## P0: Scope Freeze And ADR Baseline

Status: Complete

Objective: Convert the old "future" decisions into active v1.2 planning
decisions.

Completed deliverables:

- Created this release plan.
- Added ADRs 0018 through 0032.
- Chose shared `network_*` DecentDB table names for protocol-neutral network
  state.
- Chose DecentDB as mutable runtime source of truth for door definitions after
  setup import.
- Chose provider boundaries for DOS, native, and remote doors.

No code is changed in P0.

## P1: Release Hygiene And Stale-Future Sweep

Status: Complete

Objective: Prepare the repository for a long v1.2 implementation without stale
documentation misleading agents.

Implementation tasks:

- Update `design/TASKS.md` with a `v1.2.0 Release Work` section matching this
  phase map.
- Keep the old `Deferred From v1.1.0` list as historical context, but add a note
  that v1.2 is consuming those items.
- Add links from `design/ROADMAP.md`, `design/PRD.md`, and `README.md` to this
  plan.
- Do not rewrite all future wording yet. During implementation, each phase must
  update the docs it makes real.
- Add `docs/project/release-v1-2.md` only if the VitePress nav needs a public
  release-roadmap page.
- Confirm `CHANGELOG.md` and `docs/about/changelog.md` still reserve release
  notes for actual shipped changes, not planned work.

Acceptance criteria:

- A text search for `deferred from v1.1` points to this plan or historical
  v1.1 docs only.
- Every active v1.2 task has a phase owner in `design/TASKS.md`.
- No code behavior changes are made in this phase.

Validation:

```bash
rg -n -i "deferred|future|later|v2|v1.5|can wait|reserved" \
  README.md docs design config .github
```

## P2: Schema, Config, And DbWriter Foundation

Status: Planned

Objective: Add the database and configuration foundation required by the rest
of v1.2.

Implementation tasks:

- Bump the DecentDB schema from `4` to the next version.
- Implement table-rebuild migrations where DecentDB cannot safely alter checked
  or referenced tables.
- Add local message author fields required by network imports:
  - `author_kind`: `local`, `network`, or `system`
  - `author_user_id`: nullable reference to `users`
  - `author_display_name`
  - `author_network_address`
- Backfill existing messages as `author_kind = 'local'`.
- Add `oxidebbs-network` to the workspace.
- Move protocol-neutral network types from `oxidebbs-core` to
  `oxidebbs-network`.
- Re-export moved network types from `oxidebbs-core` during the transition.
- Create shared `network_*` DecentDB tables named in ADR 0032.
- Add repository APIs for network profiles, links, areas, packets, messages,
  duplicate logs, poll logs, area subscriptions, and nodelists.
- Add the shared `[network]` config model:
  - `enabled`
  - `profiles`
  - `links`
  - profile `adapter`
  - per-profile local address
  - per-link `compression`
  - per-link `transport_security`
- Preserve `[ftn]` as a deprecated compatibility alias. New examples must use
  `[network]`.
- Add `DbWriter` per ADR 0020.
- Keep direct repository APIs for setup, import, isolated CLI commands, and
  tests.

Acceptance criteria:

- Fresh schema initializes all v1.2 foundation tables.
- Schema 4 databases migrate without losing users, auth attempts, areas,
  messages, sessions, doors, door runs, or audit events.
- Migration is blocked for missing, malformed, or newer schema markers.
- `oxidebbs-network` has no dependency on `oxidebbs-core`, `oxidebbs-db`,
  `oxidebbs-ftn`, or `oxidebbs-server`.
- Config accepts multiple network profiles with independent local addresses.
- Config rejects unknown link network keys.
- Config rejects plaintext legacy transport on non-legacy profiles unless the
  phase explicitly allows it through ADR 0027.
- DbWriter tests cover ordered execution, transaction rollback, queue
  backpressure, and shutdown drain.

Documentation updates:

- `design/DECENTDB_SCHEMA.md`
- `design/SPEC.md`
- `design/FTN_PLAN.md`
- `config/oxidebbs.example.toml`
- `docs/project/sysop-cli.md` if schema CLI messages change

Validation:

```bash
./scripts/dev-check.sh
```

## P3: Caller Authorization And Flow Polish

Status: Planned

Objective: Finish deferred caller-visible behavior that depends on menus,
security levels, and screen asset selection.

Implementation tasks:

- Enforce `min_security_level` on all configured menu items.
- Decide inaccessible-item behavior as follows:
  - Runtime input for inaccessible hotkeys must be rejected with a clear
    caller-safe access-denied line.
  - The menu router must not execute inaccessible actions.
  - Default starter screen assets must not advertise inaccessible commands.
  - Dynamic menu listings, if added, must hide inaccessible items.
- Add optional per-door `min_security_level`. Effective door launch level is the
  maximum of the invoking menu item level and the door definition level.
- Add a starter caller sysop submenu using the existing `submenu` action.
- Add a default `S` main-menu item only for the sysop submenu and gate it with
  `min_security_level = 255`.
- The caller sysop submenu must not expose remote admin mutations. It may show
  sysop-oriented screens, a sysop message area, and safe board information.
- Render `terminal.logoff_screen` on logoff:
  - ANSI callers receive the configured ANSI asset when present.
  - Plain callers probe sibling `.asc` then `.txt`.
  - Missing assets log context and fall back to the current plain goodbye line.
  - Disconnect and transport-error paths remain safe.
- Update starter assets and caller command docs.

Acceptance criteria:

- Menu routing tests cover allowed, denied, missing level, and nested submenu
  cases.
- Door launch tests cover door-level and menu-level gates.
- Default new callers cannot open the sysop submenu.
- Sysop-level callers can open the sysop submenu.
- Logoff rendering tests cover ANSI, plain text, missing asset, and early
  disconnect.
- Caller command docs show `S` as implemented only where the default config maps
  it.

Documentation updates:

- `docs/project/security-levels.md`
- `docs/project/caller-commands.md`
- `docs/project/menus.md`
- `design/SPEC.md`
- `config/oxidebbs.example.toml`
- starter screen assets under `assets/screens/`

Validation:

```bash
./scripts/dev-check.sh
```

## P4: Serial/Modem Transport And File Transfers

Status: Planned

Objective: Add the deferred caller transports and file-transfer subsystem.

Implementation tasks for serial/modem:

- Implement `SerialTransport` per ADR 0019.
- Add disabled-by-default `[serial]` config.
- Support multiple configured serial devices.
- Preserve telnet behavior and config defaults.
- Add platform-specific operator errors when modem line-state features are not
  available.
- Add loopback or pseudo-terminal tests on Unix.
- Document optional hardware smoke testing.

Implementation tasks for file transfers:

- Add `oxidebbs-transfer` per ADR 0031.
- Implement an owned Rust file-transfer stack for caller file areas.
- Implement according to `design/FILE_TRANSFERS.md`.
- Treat ZMODEM send/receive as the primary BBS caller transfer protocol.
- Treat XMODEM-CRC send/receive as the required fallback protocol.
- Keep caller file-transfer protocols separate from FTN network mail transport.
  FTN/FidoNet mail exchange uses BinkP in P13, not XMODEM, YMODEM, or ZMODEM.
- Leave checksum-only XMODEM, XMODEM-1k, XMODEM-G, YMODEM, YMODEM-G,
  ZedZap/ZMODEM-8K, Kermit, and external `rz`/`sz` shell integration out of P4
  scope unless a later ADR supersedes ADR 0031.
- Add DecentDB tables:
  - `file_areas`
  - `file_entries`
  - `file_transfers`
- Add config for file-area roots, per-area read/download/upload security, and
  max upload size.
- Add caller `files` menu action.
- Gate file menus and transfer operations by security level.
- Add transfer history records with node, user, protocol, direction, size,
  duration, and outcome.
- Ensure transfer protocols operate over the transport byte interface and do
  not assume telnet-specific behavior.
- Add realistic fixture tests for protocol handshakes, checksums, retries,
  cancel handling, binary payloads, path sanitization, and telnet IAC escaping.

Acceptance criteria:

- Telnet tests still pass.
- Serial disabled-by-default config does not open device files.
- Serial loopback can complete login, menu input, and logoff in tests.
- ZMODEM can send and receive with cancel and retry behavior covered.
- XMODEM-CRC can send and receive a file over loopback transport.
- YMODEM and XMODEM-1k are not advertised in caller menus, docs, config
  examples, or release notes.
- Transfer records persist to DecentDB.
- File-area docs explain safe storage paths and security levels.

Documentation updates:

- `design/TELNET.md` or a new `design/SERIAL.md`
- `design/FILE_TRANSFERS.md`
- `design/SPEC.md`
- `docs/project/caller-commands.md`
- `docs/project/file-transfers.md`
- `config/oxidebbs.example.toml`
- `README.md`

Validation:

```bash
./scripts/dev-check.sh
```

Optional manual validation:

```bash
oxidebbs-server serve --config config/oxidebbs.example.toml
```

Then test with SyncTERM and at least one serial or pseudo-terminal path where
available.

## P5: Door Ecosystem Expansion

Status: Planned

Objective: Finish all deferred door administration, DOS door compatibility,
Pascal-based test-door, and remote-provider work.

Implementation tasks:

- Apply ADR 0030.
- Migrate door definitions to mutable DecentDB runtime records after setup
  import.
- Treat `[[doors.definitions]]` in TOML as seed definitions for new databases
  and examples for operators.
- Add sync/import behavior:
  - `setup` imports TOML door definitions into DecentDB.
  - `check` validates TOML seed definitions.
  - Runtime door listing reads DecentDB.
  - Docs explain how to re-import TOML seeds intentionally.
- Implement CLI and service-layer door add/edit:
  - key
  - name
  - provider/runner
  - working directory or remote provider key
  - command
  - drop-file format
  - enabled
  - exclusive
  - time limit
  - min security level
  - provider credentials by secret reference, never raw display
- Add drop-file writers and byte-output tests for:
  - `CHAIN.TXT`
  - `DOORFILE.SR`
  - `PCBOARD.SYS`
  - `CALLINFO.BBS`
  - existing `DOOR.SYS`
  - existing `DORINFO1.DEF`
- Add `doors dropfile --format <format>` support for every format.
- Add `RemoteDoorProvider` trait and provider registry.
- Add first adapters for BBSLink and DoorParty-style remote services.
- Redact provider credentials in logs, TUI, CLI, backups, and exports unless an
  explicit secrets export mode is implemented.
- Add the optional DOSEMU2 GitHub Actions smoke job in P17 after local coverage
  is stable.

Acceptance criteria:

- Door add/edit works from CLI through the shared service layer.
- TUI can reuse the same service methods in P8 without duplicating logic.
- Existing `oxide-check` still runs.
- Every drop-file format has exact expected-byte fixture tests.
- Remote provider dry-run validates required config without contacting a real
  provider.
- Remote provider integration tests use local fake servers, not real external
  services.
- Door provider credentials are redacted everywhere except deliberate secret
  storage APIs.

Documentation updates:

- `design/DOORS.md`
- `design/DOOR_GAME_RESOURCES.md`
- `design/STACK.md`
- `docs/project/sysop-cli.md`
- `docs/project/doors.md` if created
- `config/oxidebbs.example.toml`
- `config/doors.example.toml`

Validation:

```bash
./scripts/dev-check.sh
```

Optional manual validation:

```bash
oxidebbs-server doors check oxide-check
oxidebbs-server doors test oxide-check --user sysop --dry-run
```

## P6: Database Maintenance Operations

Status: Planned

Objective: Finish database maintenance behavior that was deferred or kept CLI
only.

Implementation tasks:

- Implement audit retention purge CLI:
  - `audit purge-retention`
  - `audit purge-before <timestamp>`
  - `--dry-run`
  - `--json`
- Use `[audit].retention_days` for `purge-retention`.
- Report deleted row count.
- Audit the purge action when audit writes are available.
- Implement `db verify` as a read-only consistency check that validates schema
  marker, expected tables, foreign references, DecentDB openability, and core
  repository read paths.
- Keep `db export --format json` read-only and schema-versioned.
- Keep `db import --format json` as full restore into schema-only target.
- Implement `db compact` only when DecentDB exposes a safe compaction API
  contract. If the API still does not exist, mark P6 blocked for compaction and
  do not claim v1.2 completion.
- Add packet archive retention tables/settings needed by P14.

Acceptance criteria:

- Audit purge cutoff tests cover dry-run and real delete.
- Purge action is audited.
- `db verify` fails clearly on malformed schema markers and broken references.
- `db compact` either performs safe compaction or P6 is explicitly blocked with
  the DecentDB API gap named.
- Restore/import/export tests still pass.

Documentation updates:

- `docs/project/sysop-cli.md`
- `design/RUNBOOK.md`
- `design/DECENTDB_SCHEMA.md`

Validation:

```bash
./scripts/dev-check.sh
```

## P7: Sysop CLI Completion

Status: Planned

Objective: Implement every CLI command previously documented as "can wait" or
kept out of v1.1.

Implementation tasks:

- `users delete <alias-or-id>`:
  - implement as safe disable by default
  - require `--hard` to be rejected unless a later ADR explicitly permits hard
    deletion
  - require a reason
  - audit the action
- `messages search`:
  - search subject, author display, body text, area key, and network metadata
  - support `--area`, `--user`, `--network`, `--limit`, and `--json`
- `doors add` and `doors edit`:
  - use P5 service layer
  - validate before writing
  - audit mutations
- `ansi convert`:
  - convert CP437 ANSI assets to plain text or UTF-8 preview output
  - preserve source files by default
  - write only to an explicit output path
- `config set`:
  - validate the full config after mutation
  - write a timestamped backup
  - never silently discard comments without documenting that the config writer
    rewrites TOML
  - support `--dry-run`
- Add CLI surfaces for P4 file transfers after P4 lands:
  - `files areas list/add/edit`
  - `files list/import/remove`
  - `files transfers recent`
- Add CLI surfaces for P9-P14 network operations in P14.

Acceptance criteria:

- Every command has `--help` output.
- Error messages are actionable.
- JSON output is stable for commands that expose `--json`.
- All destructive commands require confirmation, a reason, or a flag suitable
  for scripting.
- All write commands audit success and failure intent where possible.

Documentation updates:

- `docs/project/sysop-cli.md`
- `design/OxideBBS_SYSOP_INTERFACE.md`
- `design/OXIDEBBS_SYSOP_INTERFACE_AND_TUI_MASTER_SPEC.md`

Validation:

```bash
./scripts/dev-check.sh
```

## P8: Sysop TUI Completion

Status: Planned

Objective: Finish every TUI action that was marked future, later, V1.5, V2, or
CLI-only in v1.1.

Implementation tasks:

- Users:
  - edit selected user
  - add user if not already complete
  - view user audit history
  - view user sessions
  - view user recent posts
  - view user door runs
- Doors:
  - add/edit door definitions through P5 service layer
  - config check
  - drop-file viewer
  - dry-run
  - test launch
  - runtime cleanup
  - door logs
- Messages:
  - add/edit/disable local areas
  - pin/unpin messages
  - move messages between local areas
  - search messages
  - show network metadata for network messages
  - export metadata
- Database:
  - backup
  - doctor/check
  - verify
  - export
  - progress display for long operations
- Logs:
  - export selected range
  - jump from event to related object where possible
- Audit:
  - export report
  - date filtering
  - target filtering by user, node, and door
- ANSI/Screens:
  - launch external editor
  - inspect raw bytes
  - install default screens
- Config:
  - launch external editor
  - safe reload
  - optional `config set` integration through P7 service
- Command palette:
  - include every CLI-backed TUI action
  - hide destructive commands in read-only mode
- OxideNet:
  - implement final screens in P15, but reserve navigation and command palette
    entries now.

Acceptance criteria:

- Read-only mode hides or disables all mutations.
- Every mutation uses the same service layer as the CLI.
- All destructive actions require confirmation.
- All admin writes are audited.
- Empty states, missing server socket, and unavailable DB are handled cleanly.
- TUI layout tests cover 8, 16, and 32 node views after new panels are added.

Documentation updates:

- `design/OXIDEBBS_SYSOP_INTERFACE_AND_TUI_MASTER_SPEC.md`
- `design/SYSOP_TUI_IMPLEMENTATION_PROMPT.md`
- `docs/project/sysop-cli.md`
- `docs/project/sysop-tui-themes.md`

Validation:

```bash
./scripts/dev-check.sh
```

Manual validation:

```bash
oxidebbs-server sysop --readonly
oxidebbs-server sysop
```

## P9: Shared Network Foundation

Status: Complete

Objective: Build the protocol-neutral network layer required by legacy FTN and
OxideNet.

Implementation tasks:

- Complete the `oxidebbs-network` crate started in P2.
- Provide:
  - FTN-style address type
  - network profile type
  - network link type
  - network area mapping type
  - network message envelope
  - packet boundary type
  - queue state enums
  - duplicate detection key type
  - conversion traits between local messages and network envelopes
- Finish repository APIs for shared `network_*` tables.
- Update `oxidebbs-core` callers to use re-exported types or direct
  `oxidebbs-network` imports where dependency direction allows.
- Keep lower-level crates free of `oxidebbs-server` dependencies.

Acceptance criteria:

- `oxidebbs-network` has Rustdoc on public types.
- No dependency cycle exists.
- Local message code still works without enabling networking.
- Network config can be disabled cleanly.
- Network profile validation covers `legacy-ftn` and `oxidenet`.

Documentation updates:

- `design/ARCHITECTURE.md`
- `design/FTN_NETWORKING.md`
- `design/FTN_PLAN.md`
- `design/OXIDENET_PRD.md`
- `design/SPEC.md`

Validation:

```bash
./scripts/dev-check.sh
```

## P10: Legacy FTN Packet And Message Engine

Status: Complete

Objective: Implement the byte-level legacy FTN packet and message primitives.

Implementation tasks:

- Create `oxidebbs-ftn`.
- Implement ADR 0021:
  - `PacketHeader`
  - `PacketMessage`
  - `MessageAttribute`
  - `PacketReader`
  - `PacketWriter`
  - Type-2 input
  - Type-2+ input
  - Type-2+ output
- Implement ADR 0022:
  - `EchomailKludge`
  - `FtnParsedMessage`
  - `FtnMessageComposer`
  - `FtnAddressList`
  - tolerant parser
  - strict composer
- Implement ADR 0023:
  - `DuplicateDetector`
  - DecentDB duplicate detector
  - duplicate log records
  - MSGID-primary hashing
  - fallback body hash with clock skew tolerance

Acceptance criteria:

- Packet read/write round trips preserve raw bytes where required.
- Non-UTF-8 bodies are accepted and preserved.
- Kludge parsing handles AREA, MSGID, REPLY, INTL, FMPT, TOPT, FLAGS, SEEN-BY,
  PATH, Via, tear, and origin.
- Duplicate detection prevents repeated imports.
- All public types have Rustdoc.
- Fixture tests cover malformed and valid packets.

Documentation updates:

- `docs/ftn/packet-format.md`
- `docs/ftn/echomail.md`
- `docs/ftn/netmail.md`
- `design/FTN_PLAN.md`

Validation:

```bash
./scripts/dev-check.sh
```

## P11: FTN Toss, Scan, And Bundles

Status: Complete

Objective: Implement inbound and outbound legacy FTN packet workflows.

Implementation tasks:

- Implement ADR 0024 tosser quarantine policy.
- Implement inbound tosser:
  - scan inbound directory
  - extract bundles
  - validate packet address and password
  - route echomail by AREA
  - deliver local netmail
  - forward non-local netmail to outbound queue
  - store SEEN-BY and PATH
  - record packet status
  - archive or quarantine originals
- Implement outbound scanner:
  - find eligible local messages
  - create per-link outbound queue rows
  - apply moderation state
  - apply loop prevention
  - compose echomail and netmail
  - write Type-2+ packets
  - reuse MSGID values on retry per ADR 0025
- Implement bundle support per ADR 0028:
  - raw `.pkt`
  - ZIP
  - ARJ
  - arcmail naming
  - corrupt archive handling

Acceptance criteria:

- Known-good echomail packet tosses into local area.
- Wrong password quarantines packet.
- Unknown AREA tag follows configured skip/quarantine behavior.
- Duplicate message is logged and skipped.
- Scanner creates correct outbound packets per subscribed link.
- SEEN-BY and PATH loop prevention works.
- ZIP and ARJ creation/extraction tests pass.
- End-to-end compose, scan, read, toss cycle passes.

Documentation updates:

- `docs/ftn/tosser.md`
- `docs/ftn/scanner.md`
- `docs/ftn/bundles.md`
- `docs/ftn/configuration.md`
- `design/FTN_PLAN.md`

Validation:

```bash
./scripts/dev-check.sh
```

## P12: FTN Routing, Nodelist, And AreaFix

Status: Complete

Objective: Add routing and network-management protocols around the packet
engine.

Implementation tasks:

- Implement nodelist parser and index:
  - full `NODELIST.xxx`
  - comments and blank lines
  - Zone, Region, Host, Hub, Pvt, Hold, Down, normal nodes
  - continuation lines
  - flags with bare and value forms
  - DecentDB-backed lookup
- Implement nodelist differential updates per ADR 0028.
- Implement netmail routing per ADR 0026:
  - local
  - direct
  - hub routed
  - crash
  - hold
  - unknown
- Implement AreaFix:
  - `%LIST`
  - `%QUERY`
  - `%HELP`
  - `+AREA.TAG`
  - `-AREA.TAG`
  - `+AREA.TAG !` rescan
  - password authentication
  - netmail replies
  - activity logging

Acceptance criteria:

- Full nodelist import and lookup works.
- Differential nodelist update applies to a matching full nodelist and rejects
  mismatched bases.
- Netmail routing tests cover every `RoutingDecision`.
- AreaFix commands update `network_area_subscriptions`.
- AreaFix replies are generated as netmail.
- Routing and AreaFix activity is logged.

Documentation updates:

- `docs/ftn/nodelist.md`
- `docs/ftn/netmail-routing.md`
- `docs/ftn/areafix.md`
- `design/FTN_PLAN.md`

Validation:

```bash
./scripts/dev-check.sh
```

## P13: BinkP Transport

Status: Complete

Objective: Implement BinkP client/server polling for legacy FTN, private
networks, and OxideNet transport.

Implementation tasks:

- Create `oxidebbs-binkp`.
- Keep BinkP independent from `oxidebbs-transfer`. BinkP sends and receives FTN
  packet, bundle, and TIC files as BinkP data frames; it does not use ZMODEM,
  XMODEM-CRC, YMODEM, or external transfer programs.
- Implement BinkP frame parser/writer:
  - command frames
  - data frames
  - high-bit command marker
  - 15-bit payload length
- Implement commands:
  - M_NUL
  - M_ADR
  - M_PWD
  - M_FILE
  - M_OK
  - M_EOB
  - M_GOT
  - M_ERR
  - M_BSY
  - M_GET
  - M_SKIP
- Implement `BinkpClient`.
- Implement `BinkpServer`.
- Implement ADR 0027:
  - `tls_required`
  - `plaintext_legacy`
  - `tls_opportunistic`
  - startup warnings for plaintext
  - poll-log warnings for plaintext
- Add retry with exponential backoff.
- Add one-connection-per-link concurrency guard.
- Log poll activity to `network_poll_log`.

Acceptance criteria:

- Client and server authenticate correctly.
- Wrong password is rejected.
- Files send and receive as BinkP data frames.
- Empty poll completes gracefully.
- TLS succeeds with valid certs and fails with invalid certs.
- Plaintext legacy requires explicit opt-in and logs warnings.
- Opportunistic TLS attempts TLS before allowed plaintext fallback.
- Large file transfer over loopback succeeds.
- Concurrent link tests pass.

Documentation updates:

- `docs/ftn/binkp.md`
- `design/MAILER.md`
- `docs/ftn/configuration.md`
- `docs/ftn/troubleshooting.md`

Validation:

```bash
./scripts/dev-check.sh
```

## P14: FTN Operations, Hardening, And Docs

Status: Complete

Objective: Make the FTN engine operable by sysops.

Implementation tasks:

- Add `oxidebbs-server net` CLI:
  - `net toss [network]`
  - `net scan [network]`
  - `net poll <link-name>`
  - `net poll --all`
  - `net poll --dry-run <link-name>`
  - `net status [network]`
  - `net queue <link-name>`
  - `net nodelist import <file>`
  - `net nodelist apply-diff <file>`
  - `net nodelist lookup <address>`
  - `net nodelist count`
  - `net areas list [network]`
  - `net areas subscribe <area-tag> <link-name>`
  - `net areas unsubscribe <area-tag> <link-name>`
  - `net links list`
  - `net links show <link-name>`
  - `net packets inbound`
  - `net packets outbound`
  - `net packets quarantine`
  - `net areafix send <link-name> <command>`
  - `net logs [link-name] [--limit N]`
- Add quarantine dashboard data used by CLI and TUI.
- Add poll failure dashboard data.
- Add packet retention policy:
  - archive after N days
  - delete after M days
  - dry-run cleanup
- Add stats collection:
  - messages tossed
  - messages scanned
  - duplicates
  - quarantines
  - polls succeeded
  - polls failed
  - bytes sent and received
- Add stress tests:
  - 1000-message packet
  - 100 packets in one toss
  - 50,000-entry nodelist
  - concurrent inbound/outbound operations
- Complete FTN developer and sysop docs listed in `design/FTN_PLAN.md`.

Acceptance criteria:

- Every `net` command has `--help`.
- Operational errors are actionable.
- Quarantine and retention commands do not destroy data without explicit
  operator action.
- Stress tests pass without panics or obvious memory growth.
- FTN docs cover setup, daily operation, troubleshooting, and performance.

Documentation updates:

- `docs/ftn/architecture.md`
- `design/MAILER.md`
- `docs/ftn/setup.md`
- `docs/ftn/sysop-guide.md`
- `docs/ftn/cli.md`
- `docs/ftn/configuration.md`
- `docs/ftn/testing.md`
- `docs/ftn/troubleshooting.md`
- `docs/ftn/performance.md`

Validation:

```bash
./scripts/dev-check.sh
npm run docs:build
```

## P15: OxideNet Implementation

Status: Complete

Objective: Build the first-party OxideNet profile on top of the shared network
and BinkP foundations.

Implementation tasks:

- Create `oxidebbs-oxidenet`.
- Implement OxideNet defaults:
  - network name `OxideNet`
  - zone `42`
  - primary hub `42:1/1`
  - backup hub `42:1/2`
  - infrastructure range `42:1/10-99`
  - members `42:1/100+`
  - test/lab `42:1/900+`
  - default areas `OXIDE.GENERAL`, `OXIDE.SYSOP`, `OXIDE.NETWORK`,
    `OXIDE.TEST`
- Implement BBS-native application flow:
  - applicant intro
  - policy display
  - application form
  - validation
  - submission
  - application ID
  - applicant status lookup
- Implement admin review:
  - pending list
  - inspect details
  - approve
  - reject
  - request information
  - hold
  - assign address
  - generate credentials
  - generate config package
- Implement token-based join:
  - one-time plaintext display
  - token hash in DecentDB
  - max active token count
  - token revocation
- Implement config package import for member boards.
- Implement filesystem simulation for local import/export.
- Implement BinkP poll through P13.
- Implement first hub/member flow:
  - Blackboard BBS as hub
  - one test member
  - welcome netmail
  - default echomail flow both directions
  - nodelist generation
- Implement operational hardening:
  - packet quarantine UI
  - poll failure dashboard
  - node suspension
  - password rotation
  - area subscription requests
  - policy version updates
  - backup/restore notes
- Implement public experimental network support:
  - public signup through BBS-native flow
  - public docs
  - first real member node workflow
  - network policy v1.0
  - published nodelist
  - network announcements area
  - sysop support area
- Implement previously future OxideNet expansion:
  - backup hub activation
  - multi-hub topology management
  - future second/third net ranges
  - policy authority group instead of only one admin
  - DNS and BinkP reachability validation
  - opt-in public telnet listing
  - non-OxideBBS participation through documented protocol boundaries
  - FTN-to-internal converter for bridge use
- Complete TUI OxideNet screens reserved in P8:
  - dashboard
  - applications
  - application review
  - node registry
  - packet queues
  - quarantine
  - area subscriptions
  - poll logs
  - nodelist generation
  - config package generation

Acceptance criteria:

- Applicant can apply from inside the BBS.
- Admin can approve and assign a `42:1/N` address.
- Config package is generated and imported.
- Member can poll hub.
- Welcome netmail is delivered.
- At least one echomail area flows both directions.
- Duplicate messages are not imported twice.
- Suspended nodes cannot exchange mail.
- Poll attempts are logged.
- Nodelist can be generated and published.
- No web signup form is required.
- Backup hub and multi-hub management have tests even if disabled by default.
- TUI and CLI expose the same OxideNet state.

Documentation updates:

- `docs/oxidenet/PRD.md`
- `docs/oxidenet/POLICY.md`
- `docs/oxidenet/SETUP_MEMBER.md`
- `docs/oxidenet/HUB_ADMIN.md`
- `docs/oxidenet/ADDRESSING.md`
- `docs/oxidenet/AREAS.md`
- `docs/oxidenet/CONFIG_PACKAGE.md`
- `docs/oxidenet/TROUBLESHOOTING.md`
- `design/OXIDENET_PRD.md`
- `design/OXIDEBBS_SYSOP_INTERFACE_AND_TUI_MASTER_SPEC.md`

Validation:

```bash
./scripts/dev-check.sh
npm run docs:build
```

## P16: Remote Admin And Status Surface

Status: Complete

Objective: Implement the future remote web admin/status dashboard with the full
security model required by existing docs.

Implementation tasks:

- Apply ADR 0029.
- Add disabled-by-default `[admin_web]` config:
  - `enabled`
  - `bind`
  - `public_status_enabled`
  - `readonly`
  - `session_timeout_minutes`
  - `allowed_origins`
  - `behind_reverse_proxy`
- Add a separate crate or server module for remote admin HTTP.
- Expose read-only status:
  - board status
  - node summary
  - recent poll status
  - door run health
  - database health
- Expose authenticated admin views:
  - nodes
  - users
  - doors
  - messages
  - database
  - logs
  - audit
  - network/OxideNet
- Mutations may cover the same operations as local CLI/TUI only after:
  - CSRF tokens are implemented
  - replay nonces are implemented
  - audit logging is implemented
  - rate limiting is implemented
- Keep local Unix socket unchanged.
- Document TLS expectations. If TLS is terminated by a reverse proxy, examples
  must show safe headers and bind-to-localhost deployment.

Acceptance criteria:

- Remote admin is disabled by default.
- Login is rate limited.
- Cookies are HttpOnly and SameSite.
- Browser mutations require CSRF tokens.
- API mutations require nonce/timestamp replay checks.
- Failed CSRF and replay attempts are logged.
- Read-only mode blocks all mutations.
- Security tests cover missing token, reused nonce, expired timestamp, wrong
  origin, unauthenticated access, and rate limits.

Documentation updates:

- `docs/project/remote-admin.md`
- `docs/project/deployment.md`
- `docs/project/security.md` if created
- `design/SPEC.md`
- `config/oxidebbs.example.toml`

Validation:

```bash
./scripts/dev-check.sh
npm run docs:build
```

## P17: Repository And Release Automation

Status: Complete

Objective: Complete repository and release workflow items that were described as
future or optional.

Implementation tasks:

- Add Codeberg mirror automation while keeping GitHub canonical per ADR 0007.
- Document mirror direction, failure behavior, and maintainer recovery steps.
- Add a root `VERSION` file or release bump script. Use one source of truth and
  update `design/VERSIONING_GUIDE.md`.
- Update release workflow docs for current package formats and supported
  targets.
- Add optional DOSEMU2 GitHub Actions smoke job:
  - installs DOSEMU2 where supported
  - skips cleanly when unavailable
  - never blocks mandatory CI unless explicitly promoted later
- Add release dry-run checks for:
  - Linux archive
  - macOS archive
  - Windows archive
  - Docker image
  - docs build
  - checksums

Acceptance criteria:

- Mirror automation can dry-run without pushing.
- Docs state GitHub remains canonical.
- Version bump tooling updates Rust crate versions, docs package metadata, lock
  files when needed, changelog placeholders, and workflow defaults.
- Optional DOSEMU2 CI skip behavior is tested.
- Release artifact smoke instructions are current.

Documentation updates:

- `design/REPOSITORY_STRATEGY.md`
- `design/VERSIONING_GUIDE.md`
- `README.md`
- `docs/about/changelog.md`
- `.github/workflows/*` comments if needed

Validation:

```bash
./scripts/dev-check.sh
npm run docs:build
```

## P18: Final Integration And Release Readiness

Status: Complete

Objective: Prove v1.2 is shippable as one coherent release.

Implementation tasks:

- Remove or rewrite stale future/deferred wording for every implemented feature.
- Update `design/TASKS.md` so all v1.2 phases are checked off.
- Update `design/ROADMAP.md` with v1.2 completion and any genuinely new
  post-v1.2 work discovered during implementation.
- Update `design/PRD.md` so the former v2 candidates now appear in v1.2 shipped
  scope.
- Update `README.md` boundaries.
- Update `SECURITY.md` supported versions for the v1.2 line.
- Update `docs/about/changelog.md` with a real `1.2.0` entry and date only when
  release publication is imminent.
- Verify all config examples.
- Verify docs navigation includes new FTN, OxideNet, file transfer, serial,
  door, and remote admin pages.
- Run all mandatory and optional smoke paths available on the machine.

Required validation:

```bash
./scripts/dev-check.sh
npm run docs:build
rg -n -i "deferred|future|later|v2|v1.5|can wait|reserved" \
  README.md docs design config .github
```

Required smoke matrix:

- Fresh setup with default config.
- Upgrade from schema 4 development database.
- Telnet caller login, messages, doors, files, logoff.
- Serial caller loopback or hardware path when available.
- ZMODEM and XMODEM-CRC loopback caller file transfers.
- `oxide-check` live or dry-run door path.
- Remote door fake-provider fixture.
- FTN toss/scan raw packet.
- FTN ZIP and ARJ bundle processing.
- Nodelist full and differential import.
- BinkP localhost poll.
- OxideNet apply, approve, config import, first poll.
- Remote admin disabled-by-default check.
- Remote admin authenticated status and CSRF/replay rejection tests.
- Docker first-boot smoke.
- Release archive smoke for Linux locally; macOS and Windows hosted artifacts
  after publication.

v1.2 done means:

- Every phase in the status map is `Complete`.
- No feature from the coverage matrix remains unimplemented.
- No docs describe implemented v1.2 behavior as future.
- `./scripts/dev-check.sh` passes.
- `npm run docs:build` passes.
- Optional environment-dependent smoke tests either pass or skip with documented
  reasons.

## Dependency Order Summary

The highest-risk dependency chain is:

```text
P2 schema/config/DbWriter
  -> P9 shared network foundation
  -> P10 FTN packet/message engine
  -> P11 toss/scan/bundles
  -> P12 routing/nodelist/AreaFix
  -> P13 BinkP
  -> P14 FTN operations
  -> P15 OxideNet
  -> P18 release readiness
```

Caller and sysop work can proceed in parallel after P2:

```text
P2
  -> P3 caller security/logoff
  -> P4 serial/file transfer
  -> P5 door expansion
  -> P6 database operations
  -> P7 CLI completion
  -> P8 TUI completion
```

Remote admin should wait until the service layer is stable:

```text
P7 + P8 + P14 + P15 -> P16
```

Repository automation can proceed after P1:

```text
P1 -> P17
```

## Agent Handoff Rules

When assigning a coding agent to a phase:

1. Give the agent only one phase unless the phase explicitly depends on an
   incomplete subtask in another phase.
2. Require the agent to read this plan, the linked ADRs, and the source docs
   named in the phase.
3. Require a short implementation plan before code if the phase changes schema,
   config, protocol formats, or security boundaries.
4. Require docs and tests in the same change.
5. Require `./scripts/dev-check.sh` before marking the phase complete.
6. Do not accept "stubbed", "unsupported", or "reserved" command behavior as
   complete for any feature named in the coverage matrix.
