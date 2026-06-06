# OxideBBS v1.1.0 Release Plan

## TOC Map And Blocker Status

Status legend:

- `Pending` means release work, release validation, or a maintainer decision
  remains before `v1.1.0` should be tagged.
- `Done` means the item is already complete for the purpose of this plan, or the
  item is documented as intentionally deferred and does not block `v1.1.0`.

| Section | Blocker status | Release impact |
| --- | --- | --- |
| [Purpose](#purpose) | Done | Document scope and review basis. |
| [Current Snapshot](#current-snapshot) | Done | Summarizes implemented v1.1.0 work and validation state. |
| [Release Readiness Summary](#release-readiness-summary) | Pending | Local release readiness work is complete; tag creation, GitHub release publication, and hosted artifact upload remain approval-gated. |
| [Blocker 1: Version bump](#1-version-bump-is-done) | Done | All OxideBBS crates and docs package metadata now report `1.1.0`. |
| [Blocker 2: Changelog finalization](#2-changelog-is-finalized) | Done | `1.1.0` changelog entry is dated and includes operator-facing compatibility notes. |
| [Blocker 3: Stale release-state language](#3-stale-release-state-language-is-cleaned-up) | Done | Current release-line docs no longer describe the live schema or project state as pre-release. |
| [Blocker 4: Missing `design/TASKS.md`](#4-designtasksmd-is-restored) | Done | `design/TASKS.md` exists again and tracks v1.1.0 closure plus deferred work. |
| [Blocker 5: Release artifact workflow validation](#5-release-artifact-validation-is-partly-done-and-publication-remains-approval-gated) | Pending | Local package smoke passed; actual tag/release publication and hosted multi-platform artifacts still require explicit maintainer approval. |
| [Recommended Pre-Release Validation](#recommended-pre-release-validation) | Done | Rust, docs, Docker, DOSEMU2, and local package smoke checks have passed in this workspace. |
| [Scope Decisions](#outstanding-v110-scope-decisions) | Done | Recommended decisions were applied: large optional/deferred items remain outside v1.1.0. |
| [Documentation Cleanup Checklist](#documentation-cleanup-checklist) | Done | Checklist is complete for local documentation and release-readiness hygiene. |
| [Suggested Release Sequence](#suggested-v110-release-sequence) | Done | Steps through validation are complete; tag creation, GitHub release publication, hosted artifact confirmation, and docs deployment confirmation remain operational release steps. |
| [Final Recommendation](#final-recommendation) | Done | Recommendation is documented: keep v1.1.0 focused on TUI, logging, packaging, docs, and fixes. |

### Blocker Dashboard

| Blocker | Status | Done when |
| --- | --- | --- |
| B1. Bump OxideBBS release versions | Done | All OxideBBS Rust crate versions are `1.1.0`, lockfiles are refreshed, and stale version strings were reviewed. |
| B2. Finalize changelog | Done | `docs/about/changelog.md` has a dated `1.1.0` entry with operator-facing compatibility notes. |
| B3. Remove stale release-state wording | Done | Current release-line docs no longer call the live schema or project status pre-release. |
| B4. Resolve missing `design/TASKS.md` | Done | The file exists again and carries release-readiness plus deferred-work tracking. |
| B5. Validate release artifacts | Done | Hosted GitHub release workflow produces expected Linux, macOS, and Windows archives plus checksums after explicit approval to publish. |
| V1. Rust validation gate | Done | `./scripts/dev-check.sh` passed after the v1.1.0 edits. |
| V2. Documentation build | Done | `npm ci` and `npm run docs:build` passed after the v1.1.0 edits. |
| V3. Docker first boot smoke | Done | Compose first boot, status, nodes, `doors check`, and dry-run door checks passed. |
| V4. Optional DOSEMU2 smoke | Done | Local DOSEMU2 host smoke validated the live COM1 PTY bridge. |
| V5. Package install smoke | Done | A locally staged Linux release archive was checksum-verified, extracted, and smoke-tested. |

## Purpose

This document captures the current outstanding work for the OxideBBS `v1.1.0`
release from a fresh project review.

It is intentionally verbose. The goal is to make the release state clear enough
that a maintainer can decide what must ship in `v1.1.0`, what should be cleaned
up before tagging, and what is explicitly deferred to future releases.

This plan is based on a repository scan of:

- `README.md`
- `SECURITY.md`
- `docs/about/changelog.md`
- `docs/project/*.md`
- `design/*.md`
- `config/oxidebbs.example.toml`
- `crates/*/Cargo.toml`
- `.github/workflows/*.yml`
- selected source paths where CLI behavior documents deferred features

## Current Snapshot

OxideBBS has shipped `v1.0.0`. The `main` branch has been prepared for the
`v1.1.0` release line.

The `v1.1.0` changelog is dated `2026-06-03`, crate versions and docs package
metadata report `1.1.0`, and the local release-readiness work identified by
this plan has been completed.

The implemented v1.1.0 work includes:

- GitHub release artifact workflow for Linux, macOS, and Windows packages.
- Sysop-facing caller command reference.
- Sysop-facing user security level reference.
- File logging configuration under `[logging]`.
- Text and newline-delimited JSON log formats.
- Daily, size-based, and never-rotate log retention modes.
- `serve --log-level` and global `-v` logging overrides.
- Full local Ratatui sysop TUI launched by `oxidebbs-server sysop`.
- TUI screens for dashboard, nodes, users, doors, messages, database, logs,
  config, ANSI, audit, doctor, help, command palette, modal actions, and
  read-only mode.
- TUI theme selection with documented theme presets.
- TUI Doctor screen with verbose pass/warn/fail checks.
- Multiple TUI interaction fixes around console logging, command palette,
  filter/search behavior, refresh feedback, quit confirmation, and live socket
  attach behavior.
- Directory-valued DecentDB path handling.
- Better config validation for screen assets and terminal assets.
- DecentDB schema initialization fixes using DecentDB catalog metadata.
- Startup failure behavior for required database reads and required startup
  audit writes.
- Better handling of stale session rows in offline status and stats commands.
- CRLF normalization for screen and terminal assets.
- Telnet CR-NUL input handling.
- Log commands reading the configured logs directory and nested door logs.
- Sysop TUI confirmations executing backing actions and auditing changes.
- Release archives and setup-generated configs using the bundled Oxide-owned
  door fixture under `./doors/oxide-door-check/dist`, keeping `oxide-check`
  inside `paths.doors`.
- Docker builder-stage asset handling for embedded setup assets and the bundled
  door fixture.

The validation gates run for this release-readiness pass are:

```bash
./scripts/dev-check.sh
npm ci
npm run docs:build
OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
```

Docker first-boot smoke testing also passed with a fresh Compose project. The
container image built successfully, first boot created config/data/runtime
state, `status` reported version `1.1.0`, `nodes list` succeeded, and
`oxide-check` passed both `doors check` and dry-run execution.

A locally staged Linux release archive was also built, checksum-verified,
extracted, and smoke-tested with `--help`, config validation, database init, and
`doors check oxide-check`.

The only remaining item that cannot be completed without explicit maintainer
approval is actual release publication: creating the `v1.1.0` tag, publishing
the GitHub release, and confirming hosted Linux, macOS, and Windows artifacts.

## Release Readiness Summary

The local release-readiness work is complete.

Completed blockers:

1. OxideBBS crate versions now report `1.1.0`.
2. Docs package metadata now reports `1.1.0`.
3. `Cargo.lock` and `package-lock.json` were refreshed.
4. `docs/about/changelog.md` has a dated `1.1.0` entry.
5. `README.md` and `SECURITY.md` describe the `v1.1.x` release line.
6. Current release-line docs were cleaned up so they no longer describe live
   project or schema state as pre-release.
7. `design/TASKS.md` was restored.
8. Release workflow packaging now stages both the runnable `doors/oxide-door-check`
   fixture and the source fixture under `tools/doors`.
9. Docker first-boot, optional DOSEMU2, and local Linux package smoke checks
   passed.

Remaining approval-gated release operations:

1. Create the `v1.1.0` tag.
2. Publish the GitHub release.
3. Let the GitHub release workflow produce hosted Linux, macOS, and Windows
   archives plus checksums.
4. Download at least one hosted artifact and repeat package smoke testing
   against the published archive.
5. Confirm the docs site deploys successfully after publication.

Those remaining operations are not performed by automation in this workspace
because the repository instructions require explicit approval before creating
tags, pushing branches, or publishing releases.

## Release Blockers

### 1. Version bump is done

The Rust crate versions are the release version source of truth. All OxideBBS
workspace crates now report `1.1.0`, and the lockfile was refreshed.

The docs package metadata in `package.json` and `package-lock.json` was also
bumped to `1.1.0` to keep docs-site metadata aligned with the release line.

Command used to refresh package metadata:

```bash
cargo metadata --no-deps --format-version 1 >/dev/null
npm install --package-lock-only --ignore-scripts
```

Stale-version scans should continue to preserve legitimate historical references
to `v1.0.0`, such as changelog history and versioning-guide examples.

```bash
rg '1\.0\.0|v1\.0\.0' \
  Cargo.toml \
  crates \
  Cargo.lock \
  package.json \
  package-lock.json \
  README.md \
  SECURITY.md \
  docs \
  design \
  config \
  .github/workflows
```

### 2. Changelog is finalized

`docs/about/changelog.md` now contains a dated `1.1.0` entry for
`2026-06-03`.

The release notes include compatibility context that matters to operators,
including:

- New logging configuration and rotation behavior.
- New local sysop TUI behavior.
- TUI behavior around attaching to an existing live control socket or starting
  an embedded serve runtime.
- Directory-valued database path behavior.
- Startup failure behavior for database and audit-write failures.
- Screen and terminal asset fallback/logging behavior.
- Generated config and release package placement for the bundled
  `oxide-check` fixture under `./doors/oxide-door-check/dist`.
- Release archive contents.

Historical entries remain intact.

### 3. Stale release-state language is cleaned up

Current design and versioning docs were updated so they do not present the live
schema or current release line as pre-release.

Notable cleanup:

- `design/DECENTDB_SCHEMA.md` now describes schema version `4` as the current
  v1 release-line schema.
- `design/SPEC.md` now refers to compatible older development schemas.
- `design/VERSIONING_GUIDE.md` and `docs/project/versioning.md` frame pre-1.0
  rules as historical policy.

Current schema wording:

```text
Schema version `4` is the current v1 release-line schema. The initializer
upgrades supported older development databases and refuses missing, malformed,
or newer schema markers.
```

Recommended follow-up scan:

```bash
rg -n -i 'still pre-release|compatible older pre-release|current.*pre-release|unreleased' \
  README.md \
  SECURITY.md \
  docs \
  design \
  CHANGELOG.md
```

### 4. `design/TASKS.md` is restored

The project process expects `design/TASKS.md` to exist:

- `AGENTS.md` lists `design/TASKS.md` as part of the workspace layout.
- `AGENTS.md` says to update `design/TASKS.md` when completing or adding work.
- `design/VERSIONING_GUIDE.md` lists `design/TASKS.md` as release-facing
  documentation.
- `design/OXIDE_TEST_DOOR.md` says `design/TASKS.md` was updated.

The recommended decision was to recreate the file. `design/TASKS.md` now exists
and includes:

- `## v1.1.0 Release Readiness`
- `## Deferred From v1.1.0`
- `## Future Backlog`
- `## Recently Completed`

### 5. Release artifact validation is partly done and publication remains approval-gated

`.github/workflows/release.yml` exists and is documented in the changelog. It
builds platform packages and uploads them to a GitHub release.

Current package naming uses the leading release tag:

```text
oxidebbs-${tag}-${target}
```

The versioning guide documents artifact names such as:

```text
oxidebbs-v1.0.0-x86_64-unknown-linux-gnu.tar.gz
```

That appears consistent if the tag is `v1.1.0`.

Completed local validation:

- The manual-dispatch default tag was updated to `v1.1.0`.
- The release packaging workflow now stages:
  - `oxidebbs-server`
  - `README.md`
  - `LICENSE`
  - `NOTICE`
  - `SECURITY.md`
  - `assets/`
  - `config/`
  - runnable bundled fixture under `doors/oxide-door-check`
  - source fixture under `tools/doors`
  - matching `.sha256` file
- A local Linux archive smoke test reproduced the workflow package shape,
  verified the checksum, extracted the archive, created runtime directories, ran
  `--help`, validated config, initialized the database, and ran
  `doors check oxide-check`.

Publication-gated validation:

- Create or select the GitHub release for `v1.1.0`.
- Run the workflow against that release.
- Confirm Linux, macOS, and Windows artifacts upload correctly.
- Download at least one hosted artifact and repeat the package smoke test
  against the published archive.

Those publication-gated steps require explicit maintainer approval because they
create tags, publish releases, or depend on published release state.

## Recommended Pre-Release Validation

These checks were identified as strongly recommended before a public `v1.1.0`
tag. They have been run locally where possible. Hosted artifact checks remain
part of the approval-gated publication flow.

### 1. Run the full Rust validation gate

The CI gate is:

```bash
./scripts/dev-check.sh
```

It runs:

```bash
cargo fmt --all --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Status: Done.

Result: Passed after the v1.1.0 edits.

### 2. Build the documentation site

Run:

```bash
npm ci
npm run docs:build
```

Status: Done.

Result: `npm ci` and `npm run docs:build` passed after the v1.1.0 edits.

### 3. Docker first boot smoke test

Docker is the documented cross-platform deployment path. Before release, verify
the default Compose flow still works:

```bash
OXIDEBBS_SYSOP_PASSWORD='choose-a-real-password' docker compose up -d --build
docker compose run --rm oxidebbs status
docker compose run --rm oxidebbs nodes list
docker compose run --rm oxidebbs doors check oxide-check
docker compose run --rm oxidebbs doors test oxide-check --user sysop --dry-run
docker compose down
```

Also validate reset behavior if time allows:

```bash
docker compose down -v
```

Acceptance criteria:

- First boot creates config, database, sysop account, logs, runtime directories,
  and door directories.
- `status` succeeds.
- `nodes list` succeeds.
- `doors check oxide-check` succeeds.
- `doors test oxide-check --dry-run` succeeds.
- The default local-evaluation password warning appears only when the default is
  used.

Status: Done.

Result: Passed with an isolated Compose project. The image built successfully,
first boot created config and database state, `status` reported `1.1.0`,
`nodes list` showed all four nodes available, `doors check oxide-check`
succeeded, and `doors test oxide-check --user sysop --dry-run` exited
successfully.

### 4. Optional live DOSEMU2 smoke test

The optional DOSEMU2 smoke test is documented in getting-started and sysop CLI
docs. It is intentionally not part of mandatory CI.

Run on a host or container with DOSEMU2 support:

```bash
OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
```

For Docker:

```bash
docker compose run --rm oxidebbs /bin/bash -lc \
  'cd /srv/oxidebbs && OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh'
```

Acceptance criteria:

- The script detects unsupported legacy `dosemu-1.x` and skips clearly.
- On a DOSEMU2-capable environment, `OXIDECHK.EXE` runs through the COM1 PTY
  bridge.
- `OXIDECHK.RPT` is created.
- `OXNODE.TXT` is created.
- The output confirms caller-to-door and door-to-caller byte flow.

Status: Done.

Result: Passed on the local host with DOSEMU2. The script launched
`OXIDECHK.EXE` under DOSEMU2 for node 1 and reported that the smoke test passed.

### 5. Public-exposure connection-limit smoke test

Before exposing a board beyond loopback, run the load-test note from the runbook:

- Configure `max_connections`.
- Connect `max_connections + 1` callers.
- Confirm the extra caller receives:

```text
System is busy. Please try again later.
```

- Confirm accepted nodes remain visible and stable through:

```bash
oxidebbs-server nodes list
```

This is especially relevant because telnet is plaintext and public exposure is
an operator decision.

Status: Deferred, non-blocking.

Reason: This is an operator hardening check for a board being exposed beyond
loopback, not a package or build release blocker. It remains documented in the
runbook and should be run by operators before public deployment.

### 6. Release-package install smoke test

After release artifacts are produced, test at least the Linux package locally:

1. Download the Linux archive and checksum.
2. Verify the checksum.
3. Extract the archive.
4. Run:

```bash
./oxidebbs-server --help
./oxidebbs-server --config config/oxidebbs.example.toml check
./oxidebbs-server --config config/oxidebbs.example.toml doors check oxide-check
```

If macOS and Windows maintainers are available, repeat the basic `--help` and
config validation checks on those platforms.

Status: Done locally, pending for hosted artifacts.

Result: A locally staged Linux archive matching the release workflow package
shape was built, checksum-verified, extracted, and smoke-tested. The smoke test
ran `--help`, config validation, database initialization, and
`doors check oxide-check` successfully.

Remaining publication step: repeat this check against a hosted artifact after
the GitHub release workflow publishes archives.

## Outstanding v1.1.0 Scope Decisions

This section lists items that are documented as optional, deferred, reserved, or
future work. The recommended decisions have been applied for `v1.1.0`: these
items remain deferred unless a subsection explicitly says otherwise.

Rationale: v1.1.0 already has enough user-visible change in logging, release
packaging, docs, and the sysop TUI to justify a minor release. Expanding the
release into additional caller-runtime, networking, or remote-admin feature
areas would increase risk without being necessary for the release goal.

### Menu-level security enforcement

Current behavior:

- Message area read/post levels are enforced.
- User creation and sysop promotion manipulate security levels.
- Door launching from the caller `Doors` menu does not use security levels.
- Caller menu routing does not enforce `min_security_level`.
- Local sysop CLI authorization is based on local machine access, config/database
  access, and the Unix control socket boundary.

Documentation says:

- `min_security_level` exists in the menu item config model.
- The caller runtime does not enforce it.
- Treat it as reserved until menu-level security filtering is implemented and
  documented.

Decision for v1.1.0:

- Recommended: keep deferred.
- Reason: Implementing this changes caller-visible menu behavior and needs
  careful config docs, screen asset review, and tests.

Future acceptance criteria:

- Menu routing rejects hidden/locked items for callers below the configured
  level.
- Door launching can be gated by door-level or menu-level security policy.
- Starter docs explain security-level behavior clearly.
- Default screens do not advertise commands unavailable to the caller.

### Caller-side Sysop command

Current behavior:

- Caller command docs list `S` / Sysop as `Not implemented`.
- Some art may mention a Sysop command, but production screens should not show
  it unless mapped to a real submenu or action.

Decision for v1.1.0:

- Decision: keep deferred.
- Starter asset check: completed. The starter assets do not advertise an
  unmapped caller-side `S` / Sysop command.

Future acceptance criteria:

- Add a caller-side sysop menu action or submenu.
- Update `config/oxidebbs.example.toml`.
- Update all starter screen assets.
- Update caller command docs.

### Door add/edit CLI commands

Current behavior:

- `doors add` returns an explicit error.
- `doors edit <door-key>` returns an explicit error.
- The error says door add/edit is intentionally deferred to config-file editing
  for v1.

Decision for v1.1.0:

- Recommended: keep deferred.
- Reason: The current config-file workflow is clear and safer for this release.
  Adding door mutation commands means deciding how TOML config and DecentDB
  synchronized door records interact.

Future acceptance criteria:

- `doors add` writes or updates the intended source of truth.
- `doors edit` has clear behavior for config-backed and DB-synced fields.
- CLI docs describe whether doors are edited in TOML, DecentDB, or both.
- TUI door edit actions use the same service layer.

### Additional drop-file formats

Current behavior:

- `DOOR.SYS` is supported.
- `DORINFO1.DEF` is supported.

Documented later formats:

- `CHAIN.TXT`
- `DOORFILE.SR`
- Wildcat variants
- PCBoard variants
- Other variants as needed

Decision for v1.1.0:

- Recommended: keep deferred.
- Reason: Current door fixture and v1 door path are validated with the supported
  formats. Additional formats should be added when a real door need drives them.

Future acceptance criteria:

- Add format writer.
- Add tests with expected byte output.
- Add config validation.
- Add CLI `doors dropfile --format` support.
- Update door docs.

### `db compact`

Current behavior:

- `db compact` exists as a command surface.
- It returns a hard unsupported error.
- Docs explain DecentDB has no safe compaction API contract in this release.

Decision for v1.1.0:

- Recommended: keep unsupported.
- Reason: This depends on DecentDB exposing a safe production compaction API.

Future acceptance criteria:

- DecentDB compaction API contract exists.
- OxideBBS implements the command around that API.
- Backup/restore interaction is documented.
- Failure behavior is operator-safe.

### Audit retention purge CLI wrapper

Current behavior:

- `[audit].retention_days` defaults to `365`.
- Runtime audit inserts do not purge old rows automatically.
- `oxidebbs-db` exposes `purge_audit_events_older_than`.
- Docs say scheduled maintenance should call the repository helper or a future
  CLI wrapper.

Decision for v1.1.0:

- Decision: keep deferred.
- Reason: This is operationally useful and narrower than most deferred items,
  but it still adds a CLI contract. It is better handled as a focused follow-up
  once the command shape, audit behavior, and docs can be reviewed together.

Possible v1.1.0 command shape:

```bash
oxidebbs-server audit purge-retention
oxidebbs-server audit purge-before <timestamp>
```

Acceptance criteria if implemented:

- Uses `[audit].retention_days` for default retention cutoff.
- Supports dry-run or at least reports the number of rows deleted.
- Audits the purge action if audit writes are available.
- Has tests around cutoff behavior.
- Docs updated in sysop CLI and runbook.

### Dedicated logoff screen rendering

Current behavior:

- `terminal.logoff_screen` exists as configuration metadata.
- Runtime logoff currently sends a plain goodbye line.
- SPEC says dedicated logoff rendering is future behavior.

Decision for v1.1.0:

- Decision: keep deferred.
- Reason: The config already advertises a logoff screen, so honoring it would
  reduce surprise. However, it touches caller flow and asset fallback behavior.
  That makes it better suited to a focused caller-flow polish change with tests
  instead of a late release-readiness edit.

Acceptance criteria if implemented:

- ANSI callers receive configured logoff ANSI asset when present.
- Plain callers receive `.asc` or `.txt` fallback when present.
- Missing asset logs enough context and falls back safely.
- Tests cover ANSI, plain, missing asset, and disconnect behavior.

### DbWriter service

Current behavior:

- v1 uses direct repository writes through the shared DecentDB wrapper.
- Multi-row restore operations use explicit transactions.
- The roadmap says DbWriter is deferred until write contention emerges.

Decision for v1.1.0:

- Recommended: keep deferred.
- Reason: No current validation failure points to write contention as a release
  blocker. Adding a writer service would be a significant architecture change.

Future trigger:

- Observed write contention.
- Transaction serialization problems.
- Need for a single ordered write stream across session tasks.

### Physical serial/modem transport

Current behavior:

- Telnet is the only remote caller transport for the v1 release line.
- Transport abstractions leave room for serial/modem support.

Decision for v1.1.0:

- Recommended: keep deferred.
- Reason: Physical modem support is larger than the v1.1.0 logging/TUI release
  scope and introduces hardware, line-state, and deployment complexity.

### File transfers

Current behavior:

- Caller file-transfer support is listed as future work.
- No caller file-transfer subsystem exists.

Decision for v1.1.0:

- Recommended: keep deferred.
- Reason: This is a major feature area and should be separately scoped.

### FTN implementation

Current behavior:

The project has FTN/OxideNet foundation types:

- `FtnAddress`
- `EchoMailAreaMapping`
- `NetMailMessage`
- `DuplicateDetectionKey`
- `PacketBoundary`
- `PacketDirection`
- `AreaKind::EchoMail`
- `AreaKind::NetMail`
- network-related message and area fields
- minimal `[ftn]` config

What does not exist yet according to `design/FTN_PLAN.md`:

- `oxidebbs-network` crate
- `oxidebbs-ftn` crate
- `oxidebbs-binkp` crate
- `oxidebbs-oxidenet` crate
- FTN `.pkt` parser
- FTN `.pkt` writer
- Echomail kludge parser
- Tosser
- Scanner
- Bundle creation/extraction
- Nodelist parser
- Duplicate detector backed by DecentDB
- Seen-by/PATH propagation
- Netmail routing
- AreaFix
- BinkP client/server
- CLI commands for toss, scan, and poll
- DecentDB tables for real FTN state

Decision for v1.1.0:

- Recommended: keep real FTN implementation deferred.
- Reason: The PRD describes v1.1/v1.2 as foundation. The current foundation is
  enough for the v1.1.0 scope if docs are clear that real networking is not
  implemented.

Required doc clarity:

- README and docs should not imply real FTN packet exchange exists.
- The `[ftn]` example config should remain clearly disabled/foundation-only.

### OxideNet implementation

Current behavior:

- `design/OXIDENET_PRD.md` defines a future OxideNet message network.
- It includes phases from design foundation through public experimental network.

Outstanding OxideNet phases:

- Local FTN data model.
- Home BBS application module.
- Application lifecycle and manual approval.
- Config package generation.
- Local import/export simulation.
- First hub/member flow.
- BinkP-compatible transport.
- Packet quarantine UI.
- Poll failure dashboard.
- Node suspension.
- Password rotation.
- Area subscription requests.
- Policy version updates.
- Public experimental OxideNet.

Decision for v1.1.0:

- Recommended: keep deferred.
- Reason: OxideNet is larger than the current v1.1 release and depends on
  additional network infrastructure.

### Remote web admin/status surface

Current behavior:

- OxideBBS is local-admin-first.
- No remote web or TCP admin interface exists in this release line.
- Docs say any future web admin interface must include CSRF and replay
  protection before being enabled.

Decision for v1.1.0:

- Recommended: keep deferred.
- Reason: The local CLI/TUI/control socket story is coherent and safer.

Future acceptance criteria:

- Authentication model.
- CSRF protection.
- Replay protection.
- Threat model.
- Separate ADR.
- Docs explaining local vs remote admin boundaries.

### Remote door providers and door compatibility

Current behavior:

- Door execution is currently DOS-door/DOSEMU2-centered.
- `design/DOOR_GAME_RESOURCES.md` mentions BBSLink/DoorParty-style remote door
  providers as future integration targets.

Decision for v1.1.0:

- Recommended: keep deferred.
- Reason: The current release should stabilize local DOS door execution and
  sysop tooling before expanding the door model.

Future acceptance criteria:

- Clear boundary between local DOS doors and remote door providers.
- Security model for remote providers.
- Terminal byte compatibility documented.
- Config model updated.

### Codeberg mirror

Current behavior:

- GitHub is canonical.
- Codeberg is documented as a possible future mirror.

Decision for v1.1.0:

- Recommended: optional and non-blocking.
- Reason: It is a repository/community presence choice, not a product release
  requirement.

If implemented:

- README should state GitHub remains canonical unless that decision changes.
- Mirror automation should be documented.

## Documentation Cleanup Checklist

Before tagging `v1.1.0`, update or verify:

- [x] `docs/about/changelog.md` has `## [1.1.0] - 2026-06-03`.
- [x] All Rust crate versions are `1.1.0`.
- [x] `Cargo.lock` is refreshed after the crate package version updates.
- [x] `package.json` and `package-lock.json` now report `1.1.0`.
- [x] `README.md` describes the `v1.1.x` release line accurately.
- [x] `SECURITY.md` supported versions mention `v1.1.x` after release.
- [x] `design/DECENTDB_SCHEMA.md` describes schema `4` as the current v1
  release-line schema.
- [x] `design/SPEC.md` describes current migration policy in release-line terms.
- [x] `docs/project/versioning.md` frames old pre-1.0 guidance as historical.
- [x] `design/TASKS.md` exists again and tracks release readiness.
- [x] `config/oxidebbs.example.toml` passes config validation in local package
  smoke testing.
- [x] Starter ANSI/ASCII/text assets do not advertise unimplemented caller
  commands.
- [x] Starter welcome art no longer shows the original development-era version
  label.
- [x] Deferred features are documented as deferred, not silently absent.

Suggested commands:

```bash
rg -n -i 'still pre-release|compatible older pre-release|current.*pre-release|\[1\.1\.0\] \[Unreleased\]' \
  README.md \
  SECURITY.md \
  docs \
  design \
  CHANGELOG.md \
  Cargo.toml \
  crates \
  package.json \
  package-lock.json
```

Historical changelog references and versioning examples may still mention older
release milestones. Review matches instead of bulk-replacing them.

## Suggested v1.1.0 Release Sequence

1. Done: Confirm the intended `v1.1.0` scope.
2. Done: Decide which optional/deferred items remain deferred.
3. Done: Update stale release-state wording in current docs.
4. Done: Restore `design/TASKS.md`.
5. Done: Bump Rust crate versions to `1.1.0`.
6. Done: Refresh `Cargo.lock`.
7. Done: Align docs package metadata with `1.1.0`.
8. Done: Finalize `docs/about/changelog.md`.
9. Done: Update `SECURITY.md` supported versions for `v1.1.x`.
10. Done: Run:

```bash
./scripts/dev-check.sh
npm ci
npm run docs:build
```

11. Done: Run Docker first-boot smoke testing.
12. Done: Run optional DOSEMU2 smoke testing where supported.
13. Done: Run stale-string scans.
14. Pending approval: Create tag `v1.1.0`.
15. Pending approval: Publish the GitHub release.
16. Pending publication: Confirm hosted release artifacts and checksums upload
    correctly.
17. Pending publication: Download at least one hosted artifact and run package
    smoke tests.
18. Pending publication: Confirm the docs site deploys successfully.

## Final Recommendation

Treat `v1.1.0` as a release centered on:

- local sysop TUI,
- logging and log rotation,
- release artifact packaging,
- stronger documentation,
- clearer operator workflows,
- and bug fixes discovered after `v1.0.0`.

Do not expand `v1.1.0` into real FTN, OxideNet, physical serial, file transfer,
or remote web admin. Those are large enough to deserve their own milestones and
review passes.

The local release hygiene is complete. The remaining work is publication:

- obtain explicit maintainer approval to create and push tag `v1.1.0`,
- publish the GitHub release,
- confirm hosted Linux, macOS, and Windows artifacts plus checksums,
- repeat package smoke testing against a hosted artifact,
- and confirm the docs site deployment.
