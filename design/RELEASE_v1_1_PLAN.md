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
| [Current Snapshot](#current-snapshot) | Done | Summarizes implemented v1.1.0 work and known validation state. |
| [Release Readiness Summary](#release-readiness-summary) | Pending | High-level release readiness remains blocked by versioning, changelog, stale docs, missing task doc, and artifact validation. |
| [Blocker 1: Version bump](#1-version-bump-is-not-done) | Pending | Required before tagging because Rust crate versions still report `1.0.0`. |
| [Blocker 2: Changelog finalization](#2-changelog-is-still-unreleased) | Pending | Required before tagging because `1.1.0` is still marked `Unreleased`. |
| [Blocker 3: Stale pre-release language](#3-stale-pre-release-language-remains-in-release-facing-docs) | Pending | Required cleanup so current release-line docs do not describe the project/schema as pre-release. |
| [Blocker 4: Missing `design/TASKS.md`](#4-designtasksmd-is-referenced-but-missing) | Pending | Required process-doc decision: recreate the file or remove/update references. |
| [Blocker 5: Release artifact workflow validation](#5-release-artifact-workflow-needs-real-release-validation) | Pending | Required release confidence check for published platform packages and checksums. |
| [Recommended Pre-Release Validation](#recommended-pre-release-validation) | Pending | Rust gate is done, docs build was done during review, but Docker/package/optional runtime smoke checks remain pending. |
| [Scope Decisions](#outstanding-v110-scope-decisions) | Pending | Maintainer should confirm deferred items remain out of v1.1.0. |
| [Documentation Cleanup Checklist](#documentation-cleanup-checklist) | Pending | Checklist remains open until release docs and stale-string scans are complete. |
| [Suggested Release Sequence](#suggested-v110-release-sequence) | Pending | Operational sequence remains to be executed for the actual release. |
| [Final Recommendation](#final-recommendation) | Done | Recommendation is documented: keep v1.1.0 focused on TUI, logging, packaging, docs, and fixes. |

### Blocker Dashboard

| Blocker | Status | Done when |
| --- | --- | --- |
| B1. Bump OxideBBS release versions | Pending | All OxideBBS Rust crate versions are `1.1.0`, lockfiles are refreshed if needed, and stale version strings are reviewed. |
| B2. Finalize changelog | Pending | `docs/about/changelog.md` has a dated `1.1.0` entry with operator-facing compatibility notes. |
| B3. Remove stale pre-release wording | Pending | Current release-line docs no longer call schema/project state pre-alpha, alpha, or beta. |
| B4. Resolve missing `design/TASKS.md` | Pending | The file exists again, or all process references to it are intentionally updated. |
| B5. Validate release artifacts | Pending | GitHub release workflow produces expected Linux, macOS, and Windows archives plus checksums. |
| V1. Rust validation gate | Done | `./scripts/dev-check.sh` passed during this review. |
| V2. Documentation build | Done | `npm run docs:build` passed during this review. |
| V3. Docker first boot smoke | Pending | Compose first boot, status, nodes, and dry-run door checks pass. |
| V4. Optional DOSEMU2 smoke | Pending | Capable host/container validates live COM1 PTY bridge or skips unsupported runtime clearly. |
| V5. Package install smoke | Pending | At least one release archive is downloaded, checksum-verified, extracted, and smoke-tested. |

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

OxideBBS has shipped `v1.0.0`. The `main` branch is now tracking vnext work for
`v1.1.0`.

The current repository already contains a substantial `1.1.0 [Unreleased]`
changelog section. The implemented v1.1.0 work appears to include:

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

The Rust validation gate and docs build passed during review:

```bash
./scripts/dev-check.sh
npm run docs:build
```

Those validation results mean there is no currently observed build, test, fmt,
or clippy blocker. The remaining work is primarily release hygiene, stale
documentation cleanup, release artifact validation, and explicit scope decisions.

## Release Readiness Summary

The project is close to a `v1.1.0` release, but it is not ready to tag yet.

Outstanding release blockers:

1. The crate versions still report `1.0.0`.
2. The changelog section is still marked `[1.1.0] [Unreleased]`.
3. Release-facing docs still contain stale pre-release language.
4. `design/TASKS.md` is referenced by project process docs but does not exist.
5. The GitHub release artifact workflow should be exercised for `v1.1.0` before
   treating package publication as proven.

Recommended release hardening:

1. Run Docker first boot and basic Docker sysop commands.
2. Run the optional DOSEMU2 live smoke test on a capable host.
3. Run a connection-limit smoke test before public exposure.
4. Re-scan for stale `1.0.0`, `Unreleased`, `alpha`, `beta`, and "pre-alpha"
   references in release-facing docs.
5. Decide whether the outstanding deferred items remain deferred or become
   v1.1.0 work.

## Release Blockers

### 1. Version bump is not done

The Rust crate versions are the release version source of truth according to
`design/VERSIONING_GUIDE.md`.

Current observed state:

```text
crates/oxidebbs-core/Cargo.toml    version = "1.0.0"
crates/oxidebbs-db/Cargo.toml      version = "1.0.0"
crates/oxidebbs-door/Cargo.toml    version = "1.0.0"
crates/oxidebbs-server/Cargo.toml  version = "1.0.0"
crates/oxidebbs-sysop/Cargo.toml   version = "1.0.0"
crates/oxidebbs-telnet/Cargo.toml  version = "1.0.0"
crates/oxidebbs-term/Cargo.toml    version = "1.0.0"
```

The docs package metadata also currently reports `1.0.0`:

```text
package.json       "version": "1.0.0"
package-lock.json  "version": "1.0.0"
```

The versioning guide says not all non-Rust metadata must be bumped just because
OxideBBS is released, but the package metadata should be checked deliberately.
If it is kept at `1.0.0`, that should be an intentional decision and not drift.

Required decision:

- Bump all OxideBBS Rust crates to `1.1.0`.
- Refresh `Cargo.lock` if package metadata changes are reflected there.
- Decide whether `package.json` and `package-lock.json` should remain `1.0.0`
  as docs-site metadata or move to `1.1.0` for release alignment.

Suggested command after edits:

```bash
cargo metadata --no-deps --format-version 1 >/dev/null
```

Suggested stale-version scan:

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

Keep historical changelog entries and versioning examples where appropriate.
Do not blindly replace all historical references.

### 2. Changelog is still unreleased

`docs/about/changelog.md` currently contains:

```text
## [1.1.0] [Unreleased]
```

Before tagging, this should become a dated release entry:

```text
## [1.1.0] - YYYY-MM-DD
```

The release notes should also include compatibility context that matters to
operators. Based on current v1.1.0 content, that likely includes:

- New logging configuration and rotation behavior.
- New local sysop TUI behavior.
- TUI behavior around attaching to an existing live control socket or starting
  an embedded serve runtime.
- Directory-valued database path behavior.
- Startup failure behavior for database and audit-write failures.
- Screen and terminal asset fallback/logging behavior.
- Any expected config example changes.
- Any release artifact packaging notes.

The changelog currently contains a strong list of additions, changes, and fixes.
The work is not to invent release notes from scratch; it is to finalize the
entry and ensure compatibility notes are clear.

### 3. Stale pre-release language remains in release-facing docs

README and SECURITY were cleaned up previously, but a broader repository scan
still found stale pre-release language in design and docs.

Observed examples:

- `design/DECENTDB_SCHEMA.md` says schema version `4` is still pre-alpha.
- `design/SPEC.md` says compatible older pre-alpha schemas are migrated.
- `docs/about/changelog.md` historical entries refer to pre-alpha schema marker
  bumps.
- `docs/project/versioning.md` still contains pre-1.0 policy text.
- `design/VERSIONING_GUIDE.md` still contains pre-1.0 policy and examples.

Required decision:

- Historical changelog entries for `0.1.0`, `0.2.0`, and `1.0.0` may retain
  historical wording if it accurately describes the past.
- Current design/spec docs should not present the live schema or current release
  line as pre-alpha now that `v1.0.0` shipped.
- Versioning policy docs can keep explanatory "before v1.0.0" rules if they are
  clearly historical policy, but they should not imply the project is still
  before `v1.0.0`.

Recommended edits:

- Change `design/DECENTDB_SCHEMA.md` current schema text from "still pre-alpha"
  to release-line wording, such as:

```text
Schema version `4` is the current v1 release-line schema. The initializer
upgrades supported older development databases and refuses missing, malformed,
or newer schema markers.
```

- Change `design/SPEC.md` from "compatible older pre-alpha schemas" to
  "compatible older development schemas" or "compatible older supported schemas."

Recommended scan:

```bash
rg -n -i 'alpha|beta|pre-alpha|prealpha|pre-1\.0|before `v1\.0\.0`|before v1\.0\.0' \
  README.md \
  SECURITY.md \
  docs \
  design \
  CHANGELOG.md
```

### 4. `design/TASKS.md` is referenced but missing

The project process expects `design/TASKS.md` to exist:

- `AGENTS.md` lists `design/TASKS.md` as part of the workspace layout.
- `AGENTS.md` says to update `design/TASKS.md` when completing or adding work.
- `design/VERSIONING_GUIDE.md` lists `design/TASKS.md` as release-facing
  documentation.
- `design/OXIDE_TEST_DOOR.md` says `design/TASKS.md` was updated.

Current observed state:

```text
design/TASKS.md does not exist.
```

Required decision:

- Recreate `design/TASKS.md` as a living task tracker, or
- Remove/update references to it if the project no longer wants this file.

Recommendation:

Recreate the file. A lightweight task tracker is useful for v1.1.0 release
closure and future v1.2 planning. It should not duplicate the full roadmap or
changelog, but it can hold short release readiness checklists.

Suggested sections:

- `## v1.1.0 Release Readiness`
- `## Deferred Items`
- `## Future Backlog`
- `## Recently Completed`

### 5. Release artifact workflow needs real release validation

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

Outstanding validation:

- Run the workflow through `workflow_dispatch` for a draft or existing release,
  or validate it during the actual `v1.1.0` release publication.
- Confirm Linux, macOS, and Windows artifacts upload correctly.
- Confirm each archive includes:
  - `oxidebbs-server`
  - `README.md`
  - `LICENSE`
  - `NOTICE`
  - `SECURITY.md`
  - `assets/`
  - `config/`
  - matching `.sha256` file
- Confirm the release notes reference the artifact names that actually upload.

## Recommended Pre-Release Validation

These are not necessarily blockers if the maintainer explicitly accepts the
risk, but they are strongly recommended before a public `v1.1.0` tag.

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

This passed during the review that created this plan.

### 2. Build the documentation site

Run:

```bash
npm ci
npm run docs:build
```

`npm run docs:build` passed during the review that created this plan. If the
release process starts from a clean checkout or CI environment, run `npm ci`
first.

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

## Outstanding v1.1.0 Scope Decisions

This section lists items that are documented as optional, deferred, reserved, or
future work. The release owner should decide whether each item remains deferred
for `v1.1.0` or becomes required before release.

The recommendation is to keep most of these deferred unless they are small
documentation cleanups. v1.1.0 already has enough user-visible change in logging
and the sysop TUI to justify a minor release.

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

- Recommended: keep deferred unless starter assets accidentally advertise it.
- Required pre-release check: verify starter assets do not advertise an
  unmapped Sysop command.

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

- Recommended: consider implementing if small, otherwise keep deferred.
- Reason: This is operationally useful and narrower than most deferred items,
  but it still adds a CLI contract.

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

- Recommended: consider implementing only if it is small and low risk.
- Reason: The config already advertises a logoff screen, so honoring it would
  reduce surprise. However, it touches caller flow and asset fallback behavior.

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

- File transfer support is listed as future work if still desired.
- No ZMODEM/XMODEM/YMODEM subsystem exists.

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

### Native door API and remote door providers

Current behavior:

- Door execution is currently DOS-door/DOSEMU2-centered.
- `design/STACK.md` lists a future native door API.
- `design/DOOR_GAME_RESOURCES.md` mentions BBSLink/DoorParty-style remote door
  providers as future integration targets.

Decision for v1.1.0:

- Recommended: keep deferred.
- Reason: The current release should stabilize local DOS door execution and
  sysop tooling before expanding the door model.

Future acceptance criteria:

- Clear boundary between local DOS doors, native local doors, and remote door
  providers.
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

- [ ] `docs/about/changelog.md` has `## [1.1.0] - YYYY-MM-DD`.
- [ ] All Rust crate versions are `1.1.0`.
- [ ] `Cargo.lock` is refreshed if crate package versions are reflected there.
- [ ] `package.json` and `package-lock.json` version decision is explicit.
- [ ] `README.md` still describes v1.1.0 accurately.
- [ ] `SECURITY.md` supported versions mention `v1.1.x` after release.
- [ ] `design/DECENTDB_SCHEMA.md` no longer calls current schema pre-alpha.
- [ ] `design/SPEC.md` no longer refers to current migration policy as
  pre-alpha.
- [ ] `docs/project/versioning.md` no longer implies the project is before
  `v1.0.0`.
- [ ] `design/TASKS.md` exists or all references to it are intentionally
  removed.
- [ ] `config/oxidebbs.example.toml` still passes config validation.
- [ ] Starter ANSI/ASCII/text assets do not advertise unimplemented commands.
- [ ] Deferred features are documented as deferred, not silently absent.

Suggested commands:

```bash
rg -n -i 'alpha|beta|pre-alpha|prealpha|pre-1\.0|unreleased|1\.0\.0|v1\.0\.0' \
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

Historical changelog references and versioning examples may be valid. Review
matches instead of bulk-replacing them.

## Suggested v1.1.0 Release Sequence

1. Confirm the intended `v1.1.0` scope.
2. Decide which optional/deferred items remain deferred.
3. Update stale pre-release wording in current docs.
4. Restore or remove references to `design/TASKS.md`.
5. Bump Rust crate versions to `1.1.0`.
6. Refresh `Cargo.lock` if needed.
7. Decide docs package metadata version handling.
8. Finalize `docs/about/changelog.md`.
9. Update `SECURITY.md` supported versions for `v1.1.x`.
10. Run:

```bash
./scripts/dev-check.sh
npm ci
npm run docs:build
```

11. Run Docker first-boot smoke testing.
12. Run optional DOSEMU2 smoke testing where supported.
13. Run stale-string scans.
14. Create tag `v1.1.0`.
15. Publish the GitHub release.
16. Confirm release artifacts and checksums upload correctly.
17. Download at least one artifact and run package smoke tests.
18. Confirm the docs site deploys successfully.

## Final Recommendation

Treat `v1.1.0` as a release centered on:

- local sysop TUI,
- logging and log rotation,
- release artifact packaging,
- stronger documentation,
- clearer operator workflows,
- and bug fixes discovered after `v1.0.0`.

Do not expand `v1.1.0` into real FTN, OxideNet, physical serial, file transfer,
native door APIs, or remote web admin. Those are large enough to deserve their
own milestones and review passes.

The highest-value remaining work is release hygiene:

- finalize versions,
- finalize changelog,
- remove stale pre-release language,
- fix missing task-doc references,
- and validate packaged release artifacts.
