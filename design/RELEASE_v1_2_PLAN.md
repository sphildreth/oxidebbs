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
- `Partial`: some foundation, scaffolding, documentation, or narrow behavior
  exists, but the phase exit gate is not fully satisfied.
- `Planned`: ready for implementation, no coding started.
- `Active`: implementation is underway.
- `Blocked`: implementation cannot proceed until the named dependency changes.

| Phase | Title | Status | Exit Gate |
| --- | --- | --- | --- |
| P0 | Scope freeze and ADR baseline | Complete | This plan and ADRs 0018 through 0032 exist. |
| P1 | Release hygiene and stale-future sweep | Complete | All docs and examples name v1.2 scope accurately before coding starts. |
| P2 | Schema, config, and DbWriter foundation | Complete | Schema migration, shared config, DbWriter code, schema docs, release notes, and acceptance tests are current for the P2 foundation scope. |
| P3 | Caller authorization and flow polish | Complete | Runtime menu security, caller sysop submenu, logoff rendering, starter docs/assets, release notes, and acceptance tests are current. |
| P4 | Serial/modem transport and file transfers | Complete | Disabled-by-default serial and file-transfer config, real `serialport` physical serial transport, multi-device serial startup, raw serial caller sessions, ZMODEM send/receive state machines, XMODEM-CRC fallback, caller file-area upload/download workflows, transfer history persistence, path sanitization, telnet IAC escaping, and serial/file-transfer loopback acceptance tests exist. |
| P5 | Door ecosystem expansion | Complete | Mutable door CLI, DecentDB door sync, exclusive local-door run enforcement, all current drop-file writers/tests, BBSLink/DoorParty dry-run and live connectors with fake-server tests, provider secret redaction primitives, provider credential secret-reference storage (door_provider_credentials table with migration 6→7), and credential redaction across CLI/TUI/logs/backups/exports exist. |
| P6 | Database maintenance operations | Complete | Audit purge, db verify, export, import, and output-file `db compact --output <path> [--overwrite]` exist; active database replacement remains an explicit offline operator step. |
| P7 | Sysop CLI completion | Complete | Deferred user, message, ANSI, config, door, file-transfer, network state, toss, scan, plaintext BinkP poll, local AreaFix subscription execution, subscription metadata, write-audit CLI coverage, inbound AreaFix netmail processing, reply netmail generation with outbound packet creation, rescan queueing with targeted per-area per-link rescan, and nodediff CRC validation wired to CLI exist. |
| P8 | Sysop TUI completion | Complete | Local TUI includes dashboard, nodes, users, messages, files, network, OxideNet, doors, ANSI, config, database, doctor, logs, audit, and help screens; read-only guards, confirmation/audit paths, file/database/config/ANSI/log/audit workflows, and OxideNet operations are wired. |
| P9 | Shared network foundation | Complete | `oxidebbs-network` provides planned shared types/conversions; shared `network_*` tables and repository APIs exist; docs and tests cover the foundation. |
| P10 | Legacy FTN packet and message engine | Complete | Type-2 packet I/O, kludge parsing/composition, duplicate-key policy, DecentDB-backed duplicate detection, docs, and tests exist. |
| P11 | FTN toss, scan, and bundles | Complete | Network tables, packet scaffolding, raw/ZIP/ARJ bundle classification, raw pass-through, safe ZIP packet extraction, outbound ZIP bundle creation, inbound raw/ZIP/ARJ echomail tossing, outbound echomail packet scanning, netmail forwarding via NetmailRouter with .pkt file materialization, bundle creation integration, and AreaFix reply netmail composition exist. |
| P12 | FTN routing, nodelist, and AreaFix | Complete | Nodelist table, full-list parser with complete field extraction (location, sysop name, phone, speed, flags), atomic import/apply-diff with CRC validation wired to CLI, lookup, pure netmail routing decisions, AreaFix command parsing, local authenticated AreaFix subscription execution, inbound AreaFix netmail processing, reply netmail generation with outbound packet creation, rescan queueing with targeted per-area per-link rescan, and nodediff CRC validation exist. |
| P13 | BinkP transport | Complete | BinkP crate has frame constants, tested frame I/O, address/password handshake primitives, file offer/data-frame helpers, batch exchange helpers, TLS/plaintext client polling with retry execution, transport-security preflight policy, one-link-session guard primitives, inbound BinkP listener loop with per-link password/TLS policy validation and outbound file sending, TLS server-side accept, TLS integration in client polling with opportunistic fallback, and session management. |
| P14 | FTN operations, hardening, and docs | Complete | `net` status/toss/scan/poll/list/log/queue/packet/subscription/nodelist and local AreaFix commands read, import, export, transport, or update real DecentDB state, including packet summary/show/retry/quarantine state controls, packet retention cleanup with dry-run support, persistent FTN operations statistics aggregation from packets/messages/duplicates/poll logs, and stress tests validating 1000-message packets, 100-packet tosses, and 50,000-entry nodelists. |
| P15 | OxideNet implementation | Complete | OxideNet has DB-backed application/admin review, address assignment, token and credential lifecycle, config package generation/import, hub/member defaults, nodelist publication, suspended-node poll enforcement, CLI commands, TUI operations, and daily-operations docs. |
| P16 | Remote admin and status surface | Complete | Security ADR, disabled `[admin_web]` config, public-status/origin/loopback-only reverse-proxy validation, reusable read-only status payload, opt-in loopback `/status` HTTP surface, authenticated read-only API views, Argon2 sysop login, cookie-backed in-memory sessions with expiry, `HttpOnly`/`Secure`/`SameSite=Strict` cookies, CSRF binding/validation, origin checks, login and mutation rate limits, replay nonce/timestamp checks, logout session deletion, read-only mutation blocking, audit logging, and security tests exist. |
| P17 | Repository and release automation | Complete | Version metadata is aligned through `scripts/bump-version.sh`; Codeberg mirror dry-run automation, optional DOSEMU2 smoke workflow, and release dry-runs build/smoke archives, verify checksums, build docs, and build/smoke Docker. |
| P18 | Final integration and release readiness | Complete | Rust gate and docs build pass; stale wording scan completed with critical outdated references updated for serial/modem transport (ADR 0004), file transfers (ADR 0031), door providers (ADR 0030), TLS support, retry policy, and Codeberg mirror; dev-check.sh passes; remaining stale wording instances are legitimate future work or reserved address ranges. |

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
| TUI user edit | `OXIDEBBS_SYSOP_INTERFACE_AND_TUI_MASTER_SPEC.md` | P8 |
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

Coverage audit update 2026-06-05:

- Complete: P7, P8, P11, P12, P13, P14, and P15 have been revalidated and
  completed for the local sysop, BinkP transport, and network operations
  slices.
- Complete: P2 schema/config/DbWriter foundation and P3 caller authorization
  and flow polish are implemented, documented, and covered by targeted
  acceptance tests.
- Complete: P4 serial/modem transport and caller file transfers are implemented,
  documented, and covered by targeted protocol, serial, caller-flow, and
  persistence tests.
- Complete: P5 door administration, drop-file coverage, remote-provider dry-run
  and live connector adapters, provider fake-server tests, credential
  secret-reference storage, and CLI/TUI/audit/export redaction coverage are
  implemented.
- Complete: P6 audit purge, verify, JSON export/import, and output-file
  compaction are implemented and tested. `db compact --output <path>
  [--overwrite]` uses DecentDB checkpoint/save-as semantics, verifies the
  compacted output, and refuses the active database path.
- Complete: P7 sysop CLI coverage includes deferred user/message/ANSI/config/
  door/file-transfer commands, real FTN network operations, write-audit coverage,
  nodelist diff CRC validation, AreaFix reply/rescan queueing, and targeted
  rescan processing.
- Complete: P8 TUI coverage includes local file-area operations, read-only
  mutation guards, confirmation/audit paths, database/config/ANSI/log/audit
  workflows, and live network/OxideNet operational views.
- Complete: P9 shared network foundation and P10 legacy FTN packet/message
  primitives are implemented, documented, and tested.
- Complete: P11-P14 FTN toss/scan/bundle, routing/nodelist/AreaFix, BinkP
  transport, and operations workflows are implemented with DecentDB-backed
  state and targeted stress/concurrency coverage.
- Complete: P15 OxideNet has application review, address assignment, token and
  credential lifecycle, config-package generation/import, hub/member defaults,
  nodelist publication, suspended-node enforcement, CLI/TUI operations, and
  daily-operations docs.
- Complete: P16 remote admin/status coverage has disabled config validation,
  docs, an opt-in loopback read-only `/status` endpoint, authenticated read API
  views, cookie-backed sysop sessions, CSRF validation, replay nonce/timestamp
  validation for mutation attempts, rate limiting, origin checks, audit logging,
  and read-only mutation blocking.
- Complete: P17 release automation now has aligned version metadata, a bump
  script, Codeberg mirror dry-run automation, optional DOSEMU2 smoke automation,
  and packaged-binary smoke checks.
- Complete: P18 release readiness has been revalidated with the full Rust gate,
  docs build, release-plan status map, task tracker, stale wording scan, and
  updated current-release docs for FTN, AreaFix, BinkP, OxideNet, caller file
  transfers, remote admin, and release automation.

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
- Chose provider boundaries for local DOS doors and remote door services.

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

Status: Complete

Audit update 2026-06-04:

- Done: schema version is now `8`; schema migrations, shared `network_*` tables,
  OxideNet registry tables, the `[network]` config model, the deprecated `[ftn]`
  alias, and `DbWriter` tests exist.
- Done: `design/DECENTDB_SCHEMA.md`, `design/FTN_PLAN.md`, and
  `docs/about/changelog.md` reflect the current schema/version state without
  claiming later file-transfer protocol work as complete.
- Done: acceptance tests verify fresh schema tables, schema-marker rejection,
  schema `4 -> current` preservation for users, auth attempts, areas, messages,
  sessions, doors, door runs, and audit events, `oxidebbs-network` dependency
  direction, multi-profile network config, unknown network-link field
  rejection, non-legacy plaintext rejection, and DbWriter ordered execution,
  rollback, backpressure, and shutdown drain behavior.

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

Status: Complete

Audit update 2026-06-04:

- Done: configured menu items carry `min_security_level`, runtime routing rejects
  inaccessible items, door launches check door-level security, the starter config
  has a sysop submenu, and `terminal.logoff_screen` is rendered on normal logoff.
- Done: `docs/project/caller-commands.md`, `docs/project/security-levels.md`,
  `docs/project/menus.md`, `design/SPEC.md`, and `docs/about/changelog.md` now
  match the implemented caller behavior.
- Done: starter config and screen assets include a gated `S` Sysop submenu that
  does not expose remote admin mutations.
- Done: targeted tests verify normal-level callers cannot open the sysop
  submenu, level-255 callers can open it, door-level security checks, and logoff
  ANSI/plain/missing/early-disconnect behavior.

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

Status: Complete

Audit update 2026-06-05:

- Done: disabled-by-default `[serial]` config opens no device files unless
  explicitly enabled, supports multiple configured devices, and maps each device
  to a raw caller session over the shared byte-oriented `Transport` boundary.
- Done: `SerialTransport` opens physical TTY devices through the `serialport`
  crate, applies baud/data/parity/stop/flow-control settings, init strings, and
  answer strings, and reports unsupported carrier-detect line state at startup
  when required by config.
- Done: sessions persist `transport = 'serial'` through schema version 9 and
  serial loopback coverage completes login, menu input, and logoff.
- Done: `oxidebbs-transfer` implements XMODEM-CRC send/receive and owned ZMODEM
  send/receive state machines with metadata, CRC-32 data subpackets, retry via
  `ZRPOS`, cancel handling, batch loopback coverage, and telnet IAC escaping in
  the protocol adapter.
- Done: caller file-area workflows list enabled areas, gate read/download/upload
  by security level, support ZMODEM and XMODEM-CRC upload/download, sanitize
  upload names, enforce upload limits, store uploads pending sysop review, and
  persist transfer history with direction/protocol/bytes/duration/outcome.
- Done: tests cover ZMODEM send/receive, cancel, retry, batch transfer,
  XMODEM-CRC transfer, telnet IAC escaping, serial loopback transfers, serial
  login/menu/logoff, and DecentDB serial session persistence.

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

Status: Complete

Audit update 2026-06-05:

- Done: door definitions are persisted in DecentDB, setup/config sync exists,
  `doors add`, `doors edit`, and `doors dropfile --format` exist, and
  `CHAIN.TXT`, `DOORFILE.SR`, `PCBOARD.SYS`, and `CALLINFO.BBS` rendering
  functions exist.
- Done: `RemoteDoorProvider`, `ProviderRegistry`, BBSLink and DoorParty-style
  dry-run adapters, local required-config validation, and provider secret
  redaction primitives exist.
- Done: every currently supported drop-file format has exact CRLF byte-output
  coverage in `oxidebbs-door`.
- Done: live caller door validation now accepts every drop-file format that
  `oxidebbs-door` can render.
- Done: exclusive local doors now reject a second launch while an unfinished
  run exists for the same door, using persisted `door_runs` state; finished run
  history does not block a later launch.
- Complete: BBSLink and DoorParty-style TCP/telnet connectors exist with local
  fake-server tests, provider credential references are stored in
  `door_provider_credentials` through CLI and sysop service methods, and
  provider credential references are redacted in CLI JSON/plain output, TUI
  detail display, audit/log details, and JSON export/import flows.

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

Status: Complete

Audit update 2026-06-04:

- Done: `audit purge-retention`, `audit purge-before`, dry-run/JSON output,
  `db verify`, `db export --format json`, and `db import --format json` exist.
- Done: `db compact --output <path> [--overwrite]` writes a separate compacted
  DecentDB file using `checkpoint_wal`, `save_as`, and shared-WAL eviction,
  verifies the compacted output, and refuses to write to the active database
  path. Best-practice decision: OxideBBS intentionally does not perform live
  in-place replacement; operators replace the active database manually while the
  server is stopped.

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
- Implement `db compact --output <path> [--overwrite]` as verified output-file
  compaction. Do not perform live in-place replacement of the active database.
- Add packet archive retention tables/settings needed by P14.

Acceptance criteria:

- Audit purge cutoff tests cover dry-run and real delete.
- Purge action is audited.
- `db verify` fails clearly on malformed schema markers and broken references.
- `db compact --output <path> [--overwrite]` creates a verified compacted
  DecentDB output file and refuses the active database path.
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

Status: Complete

Audit update 2026-06-04:

- Done: `users delete` safe-disable behavior, `messages search` across
  subject, body, author display, area key, and network metadata, `ansi convert`,
  `config set`, and door add/edit commands exist.
- Done: file-transfer CLI surfaces (`files areas list/add/edit`,
  `files list/import/remove`, and `files transfers recent`) exist with stable
  JSON list output, safe unapprove-on-remove behavior, and audit events for
  file-area mutations, imports, and removals.
- Done: network CLI commands for P9-P14 now expose network status, link
  list/show, area list, queue, packet lists, poll logs, nodelist import,
  nodelist listing, nodelist lookup, inbound raw/ZIP echomail toss, outbound
  echomail packet scan, plaintext-legacy BinkP poll, poll dry-run preflight,
  local AreaFix subscription execution, and manual subscription metadata
  updates.
- Done: user, message, message-area, door, file-area, file-entry, nodelist, and
  manual network subscription write commands now audit successful mutations
  where DecentDB audit storage is available.
- Done: inbound AreaFix netmail processing, reply netmail outbound packet
  queueing, local and inbound AreaFix rescan queueing, and targeted
  per-area/per-link rescan processing are wired through the FTN CLI and tosser.

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

Status: Complete

Audit update 2026-06-04:

- Done: the local Ratatui sysop console exists with dashboard, nodes, users,
  messages, files, network, doors, ANSI, config, database, doctor, logs, audit,
  and help screens. User, door, file, database, and message workflows exist.
- Done: a read-only Network screen is wired into `ScreenId`, the screen module
  list, navigation, and the command palette. It summarizes profiles, links,
  areas, packets, messages, poll logs, duplicate events, packet statuses, and
  nodelist counts from DecentDB.
- Done: readonly mode now blocks Dashboard send/broadcast shortcuts and applies
  a central fail-closed guard for mutating form, confirmation, and command
  palette submissions while still allowing navigation, filters, refresh, export,
  and quit confirmation.
- Done: the OxideNet operational screen is wired into navigation and the command
  palette, with dashboard, applications, nodes, packet queues, quarantine,
  subscriptions, poll logs, nodelist, and config-package views.
- Done: database backup/verify/export status, config reload/editor/config-set,
  ANSI raw-byte/default-screen/editor workflows, log/audit export, message-area
  enable/disable, network metadata display, destructive confirmations, and audit
  paths are implemented through the local service layer.

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

Audit update 2026-06-04:

- Done: `oxidebbs-network` exists, has no dependency on higher-level crates, and
  provides FTN address, network profile/link, area mapping, netmail, local and
  network message envelope, duplicate-key, packet boundary, queue state, adapter,
  compression, and transport-security types.
- Done: shared `network_*` DecentDB tables and repository APIs exist in
  `oxidebbs-db`.
- Done: `oxidebbs-core` re-exports the shared network surface during the
  transition, and the network crate remains independent from higher-level
  OxideBBS crates.
- Done: docs and tests cover profile/link label parsing, local-to-network
  envelope conversion, dependency direction, disabled network config, and
  `legacy-ftn`/`oxidenet` profile validation.

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

Audit update 2026-06-04:

- Done: `oxidebbs-ftn` exists with packet, kludge, duplicate, and error modules.
- Done: `PacketReader` and `PacketWriter` read and write Type-2/Type-2+
  compatible packets, preserve raw/non-UTF-8 message bytes, and reject malformed
  packet types.
- Done: `EchomailKludge`, `FtnParsedMessage`, and `FtnMessageComposer` cover
  AREA, MSGID, REPLY, INTL, FMPT, TOPT, FLAGS, SEEN-BY, PATH, Via, tear, and
  origin lines.
- Done: duplicate-key construction uses ADR 0023 SHA-256 MSGID-primary hashes
  and fallback body hashes with five-minute clock-skew candidates;
  `DecentDbDuplicateDetector` checks `network_duplicate_log` and fails closed on
  database read errors.
- Done: required packet, echomail, netmail, and FTN plan docs are updated.

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

Audit update 2026-06-04:

- Done: lower-level schema and repository scaffolding for network packets,
  messages, duplicate logs, poll logs, seen-by/path, and subscriptions exists.
- Done: raw `.pkt`, ZIP arcmail, and ARJ arcmail inputs are classified by
  `oxidebbs-ftn`; raw packets pass through the extraction boundary and ZIP
  and ARJ bundles extract top-level `.pkt` entries into the requested output
  directory.
- Done: `oxidebbs-ftn` can create outbound ZIP bundles from one or more `.pkt`
  files with deterministic entry ordering and overwrite/duplicate guards.
- Done: ZIP extraction rejects nested paths, absolute/traversal-style names,
  non-packet entries, duplicate output names, corrupt archives, empty archives,
  and output-file collisions before handing packet paths to the tosser.
- Done: `Tosser` scans `paths.runtime/network/<profile>/inbound/drop`, imports
  mapped echomail from raw `.pkt` files and ZIP/ARJ bundles, validates packet
  origin/password against enabled links, records `network_packets` and
  `network_messages`, stores SEEN-BY/PATH nodes, skips duplicate messages,
  delivers local netmail, queues forwarded netmail, processes inbound AreaFix
  netmail, archives successful inputs, and quarantines malformed, unauthorized,
  unknown-area, and unroutable inputs.
- Done: `Scanner` writes Type-2+ outbound `.pkt` files for subscribed echomail
  links under `paths.runtime/network/<profile>/outbound/<link>/ready`, records
  pending outbound `network_packets`, records exported `network_messages`,
  materializes pending outbound netmail packets, avoids exporting the same local
  message to the same link twice, and bundles ready packet files into pending
  ZIP arcmail rows for links configured with ZIP compression.
- Decision: ZIP arcmail extraction accepts only top-level `.pkt` entries. This
  keeps extraction deterministic and avoids silently ignoring suspicious archive
  contents or writing outside the controlled temp directory.
- Decision: v1.2 does not add a separate `[network.paths]` schema. The tosser
  uses the profile-scoped default spool root
  `paths.runtime/network/<profile>/`, keeping manual/external-mailer operation
  deterministic without adding another path configuration surface.
- Done: focused tests cover raw packet toss, ZIP extraction/creation, ARJ
  extraction boundary handling, scanner bundle integration, netmail
  materialization, duplicate handling, wrong-password quarantine, 100-packet
  tosses, 1000-message packets, and concurrent scanner/tosser submissions
  through the supported `DbWriter` path.

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
- ZIP creation/extraction and ARJ extraction tests pass.
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

Audit update 2026-06-04:

- Done: `network_nodelist` schema, repository insert/list/find/count/replace
  operations, structured columns for name/location/sysop/phone/speed/flags,
  common full-nodelist parser, atomic full-list import, list/count output, and
  node/point lookup exist.
- Done: conservative plain-text `NODEDIFF.xxx` apply support exists for
  FTS-style `A<count>`, `C<count>`, and `D<count>` commands, and
  `net nodelist apply-diff <file> --base <full-list-file>` uses the existing
  atomic DecentDB replacement path and supports CRC validation with
  `--validate-crc`.
- Done: pure `NetmailRouter` and `RoutingDecision` coverage exists in
  `oxidebbs-ftn` for local, direct, hub-routed, crash, hold, and unknown
  destinations. Best-practice routing decision: crash and hold are explicit
  direct-link outcomes; hub routes are evaluated only after local and direct
  link checks fail.
- Done: pure AreaFix command parsing exists in `oxidebbs-ftn` for `%LIST`,
  `%QUERY`, `%HELP`, subscribe, unsubscribe, and rescan request command forms.
- Done: `net areafix send` authenticates the supplied password against the
  configured link password, executes AreaFix list/query/help/subscribe/
  unsubscribe/rescan-request command text, mutates
  `network_area_subscriptions`, updates the area subscribed aggregate, emits
  reply text, queues reply netmail, queues rescan requests, and audits
  authentication failures, subscription changes, and processed command batches.
- Done: inbound AreaFix netmail processing, reply netmail generation, rescan
  queueing, and scanner/tosser queue integration exist; routed netmail is
  delivered locally, queued directly, queued through a hub, or quarantined when
  unknown.

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

Audit update 2026-06-04:

- Done: `oxidebbs-binkp` exists and defines BinkP command constants, tested
  command/data frame parsing and writing, and client/server handshake
  primitives for `M_ADR`, optional `M_PWD`, address/password validation, and
  `M_OK`/`M_ERR` responses.
- Done: `M_FILE` offer parsing/writing, bounded data-frame send/receive
  helpers, `M_GOT` acknowledgement, `M_EOB` end-of-batch handling, and
  session-level filename validation exist.
- Done: batch send/receive helpers handle empty polls, ordered multi-file
  exchange, large file data-frame chunking/reassembly, and per-file `M_GOT`
  acknowledgements at the stream layer. A send-with-acknowledgements helper
  supports sequential send-then-receive sessions.
- Done: transport-security preflight policy exists for `tls_required`,
  `tls_opportunistic`, and `plaintext_legacy`, and `net poll --dry-run` reports
  the resulting TLS/plaintext plan and warnings.
- Done: `net poll <link>` and `net poll --all` perform TLS/plaintext BinkP
  client polling, send pending outbound packet files, receive remote files into
  the selected profile's inbound drop directory, mark acknowledged outbound
  packet rows processed, and record `network_poll_log` rows.
- Done: exponential retry backoff policy calculation and retry execution exist
  with validation, retry eligibility, capped delays, and poll-loop tests.
- Done: in-process one-active-session-per-link guard primitive exists and is
  integrated into poll and listener loops.
- Done: inbound BinkP listener accepts plaintext or TLS sockets, rejects
  plaintext for TLS-required links, validates per-link passwords, writes inbound
  files to the profile drop directory, sends pending outbound files, and marks
  DB-backed outbound packets processed after acknowledgement.
- Done: TLS succeeds with trusted certificates, fails for untrusted
  certificates, and opportunistic polling attempts TLS before plaintext
  fallback.

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

Audit update 2026-06-04:

- Done: a top-level `oxidebbs-server net` command group exists, and `status`,
  `toss`, `links list`, `links show`, `areas list`, `areas subscribe`, `areas
  unsubscribe`, `queue`, `packets summary/show/retry/mark-quarantined`,
  `packets inbound/outbound/quarantine`, `logs`, `poll --dry-run`,
  `poll`, `nodelist import`, `nodelist apply-diff`, `nodelist list`,
  `nodelist lookup`, `nodelist count`, `rescan list`, `rescan process`, and
  `rescan cancel` use real DecentDB network state.
- Done: packet retry/quarantine controls are intentionally DecentDB-state only:
  retry resets failed or quarantined rows to pending, and mark-quarantined
  records the reason without moving files. File movement happens during
  `net toss` processing, not during packet state-control commands.
- Done: `net areafix send <link-name> <command>` authenticates local command
  execution for a configured link, updates subscriptions, queues reply netmail,
  queues rescans, prints reply text, and audits activity.
- Done: packet retention cleanup with dry-run support, persistent operations
  stats aggregation from packets/messages/duplicates/poll logs, poll failure
  dashboard data, packet quarantine/list/retry controls, stress tests for
  1000-message packets, 100-packet tosses, 50,000-entry nodelists, and
  concurrent inbound/outbound scanner/tosser submissions through `DbWriter`
  exist.
- Done: FTN docs cover architecture, CLI, packet format, tosser, scanner,
  bundles, nodelist, AreaFix, BinkP, netmail routing, troubleshooting,
  configuration, testing, and performance boundaries.

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

Audit update 2026-06-04:

- Done: `oxidebbs-oxidenet` exists with default constants, PRD application
  lifecycle status variants, application/node/config-package data structs,
  OxideNet address classification/allocation helpers, and validation for the
  planned TOML config-package sections.
- Done: DecentDB schema `8` adds OxideNet application, node, and credential-hash
  registry tables with repository APIs, migration tests, backup/restore support,
  and database verification coverage.
- Done: foundation docs now cover OxideNet overview, addressing ranges,
  registry storage, and config-package validation boundaries.
- Done: DB-backed application submission/review/approval, member address
  assignment, session credential generation, invite token issue/revoke,
  password rotation, node suspension/reactivation, config package
  generation/import, hub default installation, and nodelist publication are
  implemented in `oxidebbs-oxidenet`.
- Done: `net oxidenet ...` CLI commands expose application, node, token,
  package, status, hub install, and nodelist workflows. OxideNet BinkP polling
  rejects suspended nodes and records node poll timestamps.
- Done: local sysop TUI OxideNet views expose dashboard, applications, nodes,
  packet queues, quarantine counts, subscriptions, poll logs, nodelist
  generation, and config-package operations. `docs/oxidenet/*` pages cover
  policy, areas, setup, hub administration, package import/export, registry, and
  troubleshooting.

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
- `docs/oxidenet/policy.md`
- `docs/oxidenet/SETUP_MEMBER.md`
- `docs/oxidenet/HUB_ADMIN.md`
- `docs/oxidenet/addressing.md`
- `docs/oxidenet/areas.md`
- `docs/oxidenet/CONFIG_PACKAGE.md`
- `docs/oxidenet/troubleshooting.md`
- `design/OXIDENET_PRD.md`
- `design/OXIDEBBS_SYSOP_INTERFACE_AND_TUI_MASTER_SPEC.md`

Validation:

```bash
./scripts/dev-check.sh
npm run docs:build
```

## P16: Remote Admin And Status Surface

Status: Complete

Audit update 2026-06-04:

- Done: ADR 0029 defines the required security model.
- Done: disabled-by-default `[admin_web]` config exists with validation for IP
  socket bind syntax, loopback-only enabled binds, public-status opt-in, origin
  allowlists, reverse-proxy loopback/TLS policy, read-only enforcement,
  CSRF/replay timing settings, positive rate-limit settings, example config, and
  remote-admin docs.
- Done: reusable read-only admin status JSON payload extraction exists for the
  existing CLI status command and HTTP status routing.
- Done: an `admin_web` server module starts only when `[admin_web].enabled =
  true` and serves `GET /status` only when `public_status_enabled = true`; the
  public payload omits database paths, caller addresses, secrets, and audit rows.
- Done: authenticated read API views exist for status, nodes, users, doors,
  messages, database health, log summaries, audit, FTN network state, and
  OxideNet state.
- Done: cookie-backed sysop-only sessions use Argon2 password verification,
  `HttpOnly`, `Secure`, `SameSite=Strict` cookies, session expiry, CSRF token
  runtime validation, origin checks, and login rate limiting.
- Done: remote mutations remain blocked by read-only mode; the guarded
  node-disconnect mutation stub validates auth, CSRF, replay nonce/timestamp,
  mutation rate limits, and audit logging before refusing to mutate state.
- Done: security tests cover missing token/session, reused nonce, expired
  timestamp, wrong origin, unauthenticated access, rate limits, logout session
  deletion, and read-only mutation blocking.

Objective: Implement the future remote web admin/status dashboard with the full
security model required by existing docs.

Implementation tasks:

- Apply ADR 0029.
- Add disabled-by-default `[admin_web]` config:
  - `enabled`
  - `bind`
  - `public_status_enabled`
  - `require_tls`
  - `read_only`
  - `session_timeout_seconds`
  - `csrf_token_ttl_seconds`
  - `replay_window_seconds`
  - `rate_limit_per_minute`
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
- Document TLS expectations. `[admin_web]` speaks plain HTTP only; HTTPS must
  be terminated by a loopback reverse proxy, and examples must show safe
  headers and bind-to-localhost deployment.

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

Audit update 2026-06-04:

- Done: a root `VERSION` file exists and release artifact workflow packaging
  exists.
- Complete: `VERSION`, all OxideBBS crate manifests, `Cargo.lock`,
  `package.json`, `package-lock.json`, and the release workflow manual-dispatch
  default are aligned at `1.2.0`.
- Complete: `scripts/bump-version.sh` updates release metadata and generated
  lockfile metadata from the root `VERSION` source of truth.
- Complete: Codeberg mirror automation exists as a manually dispatched workflow
  that defaults to dry-run and documents GitHub as canonical.
- Complete: optional DOSEMU2 smoke automation exists outside mandatory CI and
  skips cleanly when DOSEMU2 is unavailable.
- Complete: the release workflow smokes each packaged binary before upload.
- Complete: manual release workflow dispatch defaults to dry-run, builds and
  smokes Linux, macOS, and Windows archives without requiring an existing
  GitHub release, verifies generated checksum files, and runs release-ref docs
  and Docker image builds.
- Publication note: no tag, release, push, or mirror update was created.

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

Audit update 2026-06-05:

- Complete: `./scripts/dev-check.sh` and `npm run docs:build` pass.
- Complete: the stale wording scan was reviewed. Current release docs no longer
  describe implemented v1.2 behavior as absent or deferred; remaining hits are
  historical release-plan/ADR references, SemVer text, source examples, protocol
  vocabulary, or post-v1.2 compatibility/backlog notes.
- Complete: docs navigation includes FTN, OxideNet, file-transfer, serial, door,
  and remote-admin entry points.
- Complete: deep-dive FTN/OxideNet/file-transfer/serial docs describe the
  implemented operator workflows and release boundaries.
- Complete: the required smoke matrix is covered by the available local gates,
  focused protocol/runtime tests, release dry-run workflow checks, and docs
  build. Hosted macOS/Windows archive verification remains a publication-time
  operator check after release artifacts exist.
- Complete: `README.md`, `design/ROADMAP.md`, `design/PRD.md`, and
  `design/RUNBOOK.md` align with the v1.2 completion state.
- Complete: every phase in this status map and `design/TASKS.md` is marked
  `Complete` after the underlying phase gates passed.

Objective: Prove v1.2 is shippable as one coherent release.

Completed implementation tasks:

- Removed or rewrote stale future/deferred wording for implemented v1.2
  features in current release docs.
- Updated `design/TASKS.md` so all v1.2 phases are checked off.
- Updated `design/ROADMAP.md` with v1.2 completion and post-v1.2 work that is
  genuinely outside this release.
- Updated `design/PRD.md` so shipped former v2 candidates appear in v1.2 scope
  and remaining post-v1.2 compatibility work is tracked separately.
- Updated `README.md` boundaries.
- Verified config examples through the Rust gate.
- Verified docs navigation includes FTN, OxideNet, file transfer, serial, door,
  and remote admin pages.
- Ran the mandatory local gate and all optional smoke paths available on this
  machine.

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
- Every feature from the coverage matrix is implemented.
- Current release docs describe implemented v1.2 behavior as shipped.
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

1. Give the agent only one phase unless the phase explicitly depends on an open
   subtask in another phase.
2. Require the agent to read this plan, the linked ADRs, and the source docs
   named in the phase.
3. Require a short implementation plan before code if the phase changes schema,
   config, protocol formats, or security boundaries.
4. Require docs and tests in the same change.
5. Require `./scripts/dev-check.sh` before marking the phase complete.
6. Do not accept "stubbed", "unsupported", or "reserved" command behavior as
   complete for any feature named in the coverage matrix.
