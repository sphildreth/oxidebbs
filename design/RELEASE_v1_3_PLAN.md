# OxideBBS v1.3 Release Plan

Document status: Planning draft

Created: 2026-06-05

Release intent: `v1.3.0` is the post-v1.2 compatibility release. It collects
the remaining items that current documentation still marks as post-v1.2, future,
outside v1.2 scope, or unresolved compatibility work after the v1.2
deferred-scope release.

This plan does not reopen `v1.2.0` completion. Features marked complete in
[`design/RELEASE_v1_2_PLAN.md`](./RELEASE_v1_2_PLAN.md) remain the v1.2
baseline. v1.3 work must be scoped by new ADRs where existing ADRs deliberately
kept behavior outside v1.2.

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
| P0 | Scope freeze and ADR baseline | Planned | This plan is accepted and ADRs exist for door compatibility, PETSCII/manual profile persistence, transfer protocol expansion, and FTN/OxideNet compatibility boundaries. |
| P1 | Documentation reconciliation and task tracker update | Planned | v1.2/v1.3 docs no longer contradict each other about door compatibility, Wildcat variants, OxideNet converter work, or transfer protocol scope. |
| P2 | C64/PETSCII terminal completion | Planned | Full PETSCII encode/decode support, profile-aware rendering tests, and caller-flow acceptance tests exist beyond the current ASCII/PETSCII-friendly fallback. |
| P3 | Manual terminal profile persistence | Planned | User/account schema stores terminal preference, onboarding/manual selection can persist it, and telnet detection/default-profile fallback order is documented and tested. |
| P4 | Caller transfer protocol decision and expansion | Blocked | A maintainer-approved ADR supersedes or preserves ADR 0031 for YMODEM, XMODEM-1k, checksum XMODEM, and related caller-transfer variants. |
| P5 | Door drop-file compatibility expansion | Planned | Current drop-file docs and renderers agree; additional Wildcat/vendor-specific variants are either implemented with byte-exact tests or explicitly deferred by ADR. |
| P6 | FTN/OxideNet interoperability hardening | Blocked | Real-network/operator feedback or a maintainer-approved compatibility ADR defines Seen-by/PATH tuning, archive-format expansion, mailer scheduling/status gaps, and non-OxideBBS bridge expectations. |
| P7 | OxideNet topology and public-network expansion | Planned | Backup hub, future net ranges, policy-authority workflow, public listing, and reachability validation are either implemented or explicitly kept as reserved capacity. |
| P8 | Final integration and release readiness | Planned | Rust gate, docs build, stale wording scan, release notes, version metadata, and local package smoke checks pass for v1.3. |

## Reviewed Documentation

This plan is based on a review of:

- `README.md`
- `docs/**/*.md`
- `design/RELEASE_v1_1_PLAN.md`
- `design/RELEASE_v1_2_PLAN.md`
- `design/TASKS.md`
- `design/PRD.md`
- `design/ROADMAP.md`
- `design/SPEC.md`
- `design/TELNET.md`
- `design/ANSI_CP437.md`
- `design/DOORS.md`
- `design/DOOR_GAME_RESOURCES.md`
- `design/FILE_TRANSFERS.md`
- `design/MAILER.md`
- `design/FTN_PLAN.md`
- `design/OXIDENET_PRD.md`
- `design/adr/*.md`

Third-party package documentation under `node_modules/` was intentionally not
treated as OxideBBS product scope.

## ADRs For v1.3

| ADR | Topic | Used By |
| --- | --- | --- |
| ADR 0033 | Door compatibility scope | P0, P1, P5 |
| ADR 0034 | PETSCII translation and terminal-profile persistence policy | P2, P3 |
| ADR 0035 | Caller file-transfer protocol expansion decision | P4 |
| ADR 0036 | FTN/OxideNet interoperability and bridge policy | P6, P7 |

ADR 0033 is accepted. ADR 0034 through ADR 0036 are proposed placeholders. If
other ADRs are created first, renumber the placeholder rows before
implementation starts.

## v1.3 Candidate Coverage Matrix

Every row below must either be implemented, tested, and documented before v1.3
is declared complete, or explicitly moved out of v1.3 by a maintainer-approved
ADR.

| Candidate Item | Source Documents | v1.3 Phase |
| --- | --- | --- |
| Full PETSCII encode/decode rendering beyond ASCII fallback | `TASKS.md`, `SPEC.md`, `TELNET.md`, `ANSI_CP437.md`, `PRD.md` | P2 |
| Persist manual terminal profile selection in user/account settings | `TASKS.md`, `SPEC.md`, `TELNET.md` | P3 |
| YMODEM and XMODEM-1k reconsideration | `FILE_TRANSFERS.md`, ADR 0031, `docs/project/file-transfers.md` | P4 |
| Checksum XMODEM, XMODEM-g, ZedZap/ZMODEM-8K, Kermit, and other transfer variants | `FILE_TRANSFERS.md`, ADR 0031 | P4 |
| Additional Wildcat and vendor-specific drop-file variants | `DOORS.md`, `DOOR_GAME_RESOURCES.md`, `RELEASE_v1_1_PLAN.md`, `RELEASE_v1_2_PLAN.md` | P5 |
| Seen-by and PATH interoperability tuning after real-network feedback | `FTN_PLAN.md`, `OXIDENET_PRD.md`, `RELEASE_v1_1_PLAN.md` | P6 |
| Additional outbound bundle archive formats beyond ZIP | `FTN_PLAN.md`, ADR 0028 | P6 |
| Scheduled polling, external-mailer directory drop docs, and mailer CLI status/queue checklist reconciliation | `MAILER.md`, `RELEASE_v1_2_PLAN.md` | P1, P6 |
| Non-OxideBBS OxideNet participation and FTN-to-internal converter reconciliation | `FTN_PLAN.md`, `OXIDENET_PRD.md`, `RELEASE_v1_2_PLAN.md` | P1, P6 |
| OxideNet backup hub, future net ranges, and public-network topology expansion | `OXIDENET_PRD.md`, `docs/oxidenet/addressing.md`, `RELEASE_v1_2_PLAN.md` | P7 |

## Known Scope Tensions

The v1.3 scope starts with several documentation tensions that must be resolved
before implementation agents treat the plan as executable:

- ADR 0033 defines the door compatibility boundary. v1.3 door work is limited
  to compatibility with existing door games, drop-file formats, provider
  behavior, and sysop tooling.
- `design/RELEASE_v1_2_PLAN.md` says P5 completed Wildcat/PCBoard drop-file
  coverage, while `design/DOORS.md` says additional Wildcat and vendor-specific
  variants remain future compatibility work. v1.3 must identify exact remaining
  variants or move them out of active scope.
- `design/RELEASE_v1_2_PLAN.md` says P15 covered non-OxideBBS participation
  boundaries and an FTN-to-internal converter, while `design/FTN_PLAN.md` and
  `design/OXIDENET_PRD.md` still describe those as post-v1.2 or later
  compatibility. v1.3 must reconcile whether v1.2 delivered documentation
  boundaries only, or whether concrete bridge implementation is still intended.
- `design/MAILER.md` still has unchecked implementation checklist rows for
  scheduled polling, external-mailer directory drop documentation, and CLI
  status/queue views, while the v1.2 release plan marks BinkP and FTN
  operations complete. v1.3 must decide whether those rows are stale checklist
  text, v1.3 compatibility work, or intentionally out of scope.
- ADR 0031 explicitly keeps YMODEM, XMODEM-1k, checksum XMODEM, ZedZap, and
  similar caller-transfer variants outside v1.2 unless a later ADR supersedes
  it. v1.3 cannot implement or advertise those protocols until P4 accepts a new
  transfer policy.

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

Status: Planned

Objective: Turn the remaining post-v1.2 notes into explicit v1.3 decisions
before implementation starts.

Implementation tasks:

- Use ADR 0033 as the door compatibility scope boundary.
- Create or update ADRs for PETSCII/manual profile policy, transfer protocol
  expansion, and FTN/OxideNet interoperability.
- Decide whether v1.3 is allowed to include transfer protocols outside ADR 0031.
- Decide whether OxideNet bridge/converter work is concrete implementation or
  documentation-only boundary work.
- Decide whether remaining mailer checklist rows are stale, v1.3 scope, or
  superseded by v1.2 implementation.
- Decide whether additional Wildcat/vendor drop files are required for v1.3 or
  remain compatibility backlog.
- Update this plan if accepted scope differs from the current draft.

Acceptance criteria:

- Every v1.3 candidate has an accepted ADR or explicit release-plan decision.
- `design/TASKS.md` links to this plan for active v1.3 work.
- Implementation agents can pick a phase without re-litigating release scope.

Validation:

```bash
rg -n -i "post-v1\\.2|future|later|outside v1\\.2|PETSCII|YMODEM|XMODEM-1k|Wildcat|non-OxideBBS|FTN-to-internal" \
  README.md docs design config .github
```

## P1: Documentation Reconciliation And Task Tracker Update

Status: Planned

Objective: Remove contradictions left after v1.2 so v1.3 has one authoritative
scope story.

Implementation tasks:

- Add a `v1.3.0 Release Work` section to `design/TASKS.md` with this phase map.
- Update `design/PRD.md` and `design/ROADMAP.md` to point to this plan for
  post-v1.2 candidate work.
- Clarify in door docs that v1.3 door work is compatibility with existing door
  games, drop-file formats, provider behavior, and sysop tooling.
- Clarify which Wildcat and vendor-specific drop-file variants remain.
- Clarify whether non-OxideBBS participation and FTN-to-internal conversion were
  completed as v1.2 boundaries or remain v1.3 implementation work.
- Reconcile `design/MAILER.md` checklist rows for scheduled polling,
  external-mailer directory drop docs, and CLI status/queue views against the
  current BinkP/FTN commands.

Acceptance criteria:

- Searches for stale v1.2/future language produce only historical references,
  explicit v1.3 scope, or deliberate reserved-capacity notes.
- `design/TASKS.md`, `design/PRD.md`, `design/ROADMAP.md`, and this plan agree
  on v1.3 status.
- No code behavior changes are made in this phase.

Validation:

```bash
rg -n -i "v1\\.3|post-v1\\.2|future|later|outside v1\\.2|deferred" \
  README.md docs design config .github
```

## P2: C64/PETSCII Terminal Completion

Status: Planned

Objective: Replace the current ASCII/PETSCII-friendly fallback with tested
PETSCII encode/decode support for C64-oriented caller profiles.

Implementation tasks:

- Define the supported PETSCII character set, control bytes, line endings, and
  unsupported-character replacement policy in ADR 0034.
- Add full encode/decode tables and tests in the terminal layer.
- Keep CP437/ANSI behavior unchanged for ANSI and plain 80-column callers.
- Route C64 profile output through PETSCII-aware rendering where enabled.
- Add 40-column wrapping/truncation tests for generated menus, message lists,
  message bodies, file lists, and logoff flow.
- Add fixture coverage for lowercase/uppercase mode, common punctuation, CR/LF,
  backspace/delete, and unsupported glyph fallback.

Acceptance criteria:

- C64 callers can log in, navigate menus, read messages, view file lists, and
  log off through PETSCII-aware rendering.
- Existing ANSI/CP437 snapshots remain stable or are intentionally updated.
- Tests prove PETSCII bytes are not accidentally treated as Unicode-first caller
  UI.

Validation:

```bash
cargo test -p oxidebbs-term --locked petscii
cargo test -p oxidebbs-server --locked terminal
./scripts/dev-check.sh
```

## P3: Manual Terminal Profile Persistence

Status: Planned

Objective: Let callers or sysops persist terminal profile preference once the
user schema has an explicit terminal preference field.

Implementation tasks:

- Add a DecentDB migration for terminal profile preference on user/account
  records.
- Define fallback order between telnet terminal-type detection, persisted user
  preference, configured default profile, and manual session override.
- Add onboarding or account-settings flow for manual profile selection.
- Add sysop CLI and TUI edit support for terminal profile preference.
- Document config behavior and caller-facing profile choices.

Acceptance criteria:

- Existing users migrate with no forced terminal preference.
- A caller can choose ANSI/80-column, plain ASCII, or C64/40-column/PETSCII
  where the flow is enabled.
- Persisted preference survives reconnect and overrides unreliable detection
  according to ADR 0034.
- CLI, TUI, docs, config examples, and tests agree on valid profile names.

Validation:

```bash
cargo test -p oxidebbs-db --locked terminal
cargo test -p oxidebbs-core --locked terminal
cargo test -p oxidebbs-server --locked terminal
./scripts/dev-check.sh
```

## P4: Caller Transfer Protocol Decision And Expansion

Status: Blocked

Blocked by: ADR 0035.

Objective: Decide whether v1.3 expands caller file-transfer protocols beyond
ZMODEM and XMODEM-CRC.

Implementation tasks:

- Review ADR 0031 and decide whether to supersede it.
- If expansion is accepted, choose exact protocol variants and exclude the rest.
- If YMODEM is accepted, define batch behavior, metadata handling, resume
  behavior, cancellation, and caller-menu naming.
- If XMODEM-1k or checksum XMODEM is accepted, define negotiation, fallback,
  and error-reporting behavior.
- Keep BinkP clearly separated from caller file-area transfer protocols.
- Update docs, config examples, caller menus, and transfer history if new
  protocols are implemented.

Acceptance criteria:

- No new protocol is advertised until the protocol engine, caller flow, docs,
  and tests exist.
- Existing ZMODEM and XMODEM-CRC behavior remains compatible.
- Unsupported protocols remain absent from caller menus and config examples.

Validation:

```bash
cargo test -p oxidebbs-transfer --locked
cargo test -p oxidebbs-server --locked file_transfer
./scripts/dev-check.sh
```

## P5: Door Drop-File Compatibility Expansion

Status: Planned

Objective: Finish or explicitly retire the remaining Wildcat/vendor-specific
drop-file compatibility notes.

Implementation tasks:

- Inventory which Wildcat and vendor-specific formats remain beyond the current
  `DOOR.SYS`, `DORINFO1.DEF`, `CHAIN.TXT`, `DOORFILE.SR`, `PCBOARD.SYS`, and
  `CALLINFO.BBS` renderers.
- Add exact CRLF byte-output renderers and tests for accepted formats.
- Add docs for selecting each format from TOML seeds and DecentDB door records.
- Keep copyrighted door packages and abandonware out of fixtures.
- If no additional formats are accepted, update `design/DOORS.md` and the
  release plans to say v1.3 intentionally closes this compatibility note.

Acceptance criteria:

- Door docs and renderer list agree.
- Every accepted renderer has byte-exact tests.
- CLI/TUI door add/edit paths can select accepted formats.

Validation:

```bash
cargo test -p oxidebbs-door --locked drop
cargo test -p oxidebbs-server --locked doors
./scripts/dev-check.sh
```

## P6: FTN/OxideNet Interoperability Hardening

Status: Blocked

Blocked by: real-network/operator feedback or ADR 0036.

Objective: Turn broad interoperability notes into concrete FTN/OxideNet bridge
behavior only where the project has enough requirements to avoid speculative
protocol work.

Implementation tasks:

- Decide whether Seen-by/PATH tuning needs implementation changes or only
  operational documentation.
- Decide which additional outbound archive formats beyond ZIP are worth
  supporting.
- Reconcile scheduled polling, external-mailer directory drop documentation, and
  CLI status/queue checklist rows in `design/MAILER.md`.
- Reconcile v1.2 documentation about non-OxideBBS participation and an
  FTN-to-internal converter.
- If bridge work is accepted, define packet mapping, address policy, origin
  policy, duplicate detection, loop prevention, audit logging, and failure
  handling.
- Add CLI/TUI status and diagnostics for accepted bridge workflows.

Acceptance criteria:

- Compatibility work is driven by documented operator feedback or ADR 0036.
- ZIP bundle behavior and existing FTN toss/scan/poll commands keep passing.
- Bridge/converter behavior cannot create message loops or bypass duplicate
  detection.

Validation:

```bash
cargo test -p oxidebbs-ftn --locked
cargo test -p oxidebbs-oxidenet --locked
cargo test -p oxidebbs-server --locked net
./scripts/dev-check.sh
```

## P7: OxideNet Topology And Public-Network Expansion

Status: Planned

Objective: Decide how much reserved OxideNet topology becomes real operational
behavior in v1.3.

Implementation tasks:

- Confirm whether backup hub activation and multi-hub topology management are
  implementation scope or reserved-address documentation only.
- Confirm whether `42:2/*`, `42:3/*`, and `42:100/*` remain future ranges or
  become assignable/validated ranges.
- Define policy-authority group behavior if governance is no longer single-admin.
- Add DNS/BinkP reachability validation for public listings if public listing is
  accepted.
- Document operator runbooks for any public-network behavior implemented in
  v1.3.

Acceptance criteria:

- Address validation, config-package generation, nodelist publication, and TUI
  views agree on accepted ranges.
- Suspended-node and credential-rotation behavior remain enforced.
- Reserved ranges that are not implemented remain clearly labeled as reserved,
  not incomplete v1.3 work.

Validation:

```bash
cargo test -p oxidebbs-oxidenet --locked
cargo test -p oxidebbs-server --locked oxidenet
npm run docs:build
./scripts/dev-check.sh
```

## P8: Final Integration And Release Readiness

Status: Planned

Objective: Prove the accepted v1.3 scope is complete, documented, and releasable
without stale future/backlog language describing shipped behavior as absent.

Implementation tasks:

- Update crate versions and docs package metadata for `1.3.0` when release
  preparation begins.
- Update changelog entries and operator compatibility notes.
- Update `README.md`, `SECURITY.md`, `design/TASKS.md`, and public docs for the
  accepted v1.3 scope.
- Run stale wording scans and reconcile any hits that describe implemented v1.3
  behavior as future, absent, or outside scope.
- Run Rust validation, docs build, Docker smoke, release dry-run, and package
  smoke checks.

Acceptance criteria:

- Every phase in this plan is `Complete` or explicitly moved out of v1.3 by ADR.
- `./scripts/dev-check.sh` passes.
- `npm run docs:build` passes.
- Local release-package smoke checks pass.
- No tags, pushes, GitHub releases, or hosted publication steps are performed
  without explicit maintainer approval in the current conversation.

Validation:

```bash
./scripts/dev-check.sh
cd docs && npm run docs:build
rg -n -i "post-v1\\.2|future|later|outside v1\\.2|deferred|not implemented|not yet|partial|blocked|incomplete" \
  README.md docs design config .github
```

## Approval-Gated Publication

These steps remain pending until the maintainer explicitly approves tag creation
and release publication in the current conversation:

- [ ] Create and push tag `v1.3.0`.
- [ ] Publish the GitHub release.
- [ ] Confirm hosted Linux, macOS, and Windows release archives and checksums.
- [ ] Download at least one hosted artifact and repeat package smoke testing.
- [ ] Confirm the docs site deployment after publication.

## Final Recommendation

Use v1.3 as a focused compatibility release, not another large deferred-scope
sweep. The highest-confidence scope is PETSCII/manual terminal completion, door
drop-file compatibility, documentation reconciliation, and explicit decisions
for transfer-protocol and FTN/OxideNet compatibility work. Any protocol
expansion without a new ADR should remain blocked.
