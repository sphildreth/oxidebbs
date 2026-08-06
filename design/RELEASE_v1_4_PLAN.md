# OxideBBS v1.4 Release Plan

Document status: Planning draft

Created: 2026-08-05 (reconciled from `design/RELEASE_v1_3_PLAN.md`)

Last reconciled: 2026-08-05

Release intent: `v1.4.0` is the post-v1.3 compatibility release. It carries
forward the remaining door, terminal-profile, transfer-protocol, and
FTN/OxideNet compatibility scope that did not ship in v1.3.0.

`v1.3.0` shipped on 2026-08-05 containing the C64/PETSCII terminal core (ADR
0034), CP437 low-range glyph support, ANSI parser hardening, and workspace
dependency hygiene. See `docs/about/changelog.md` and the closed
[`design/RELEASE_v1_3_PLAN.md`](./RELEASE_v1_3_PLAN.md) for what that release
contained. This plan does not reopen v1.3.0 or v1.2.x completion; features
marked complete in earlier release plans remain their respective baselines.
v1.4 work must be scoped by new ADRs where existing ADRs deliberately kept
behavior out of earlier releases.

## Phase Status Map

Status values:

- `Complete`: the planning artifact exists and no code work remains for that
  planning phase.
- `Partial`: some foundation, scaffolding, documentation, or narrow behavior
  exists, but the phase exit gate is not fully satisfied.
- `Planned`: ready for implementation, no coding started.
- `Active`: implementation is underway.
- `Blocked`: implementation cannot proceed until the named dependency changes.
- `Deferred`: the phase was explicitly moved out of v1.4 by a
  maintainer-approved ADR or release-plan decision.

| Phase | Title | Status | Exit Gate |
| --- | --- | --- | --- |
| P0 | Scope freeze and ADR baseline | Partial | ADR 0034 accepted (PETSCII core shipped in v1.3.0). ADRs still needed for transfer protocol expansion and FTN/OxideNet compatibility boundaries. |
| P1 | Documentation reconciliation and task tracker update | Active | v1.3/v1.4 docs no longer contradict each other about door compatibility, Wildcat variants, OxideNet converter work, or transfer protocol scope. |
| P2 | C64/PETSCII terminal completion | Complete | Shipped in v1.3.0: full PETSCII encode/decode, `TerminalCharset::Petscii`, C64 profile routing, and tests. See ADR 0034 and `design/TASKS.md`. |
| P3 | Manual terminal profile persistence | Planned | User/account schema stores terminal preference, onboarding/manual selection can persist it, and telnet detection/default-profile fallback order is documented and tested. |
| P4 | Caller transfer protocol decision and expansion | Blocked | A maintainer-approved ADR supersedes or preserves ADR 0031 for YMODEM, XMODEM-1k, checksum XMODEM, and related caller-transfer variants. |
| P5 | Door drop-file compatibility expansion | Planned | Current drop-file docs and renderers agree; additional Wildcat/vendor-specific variants are either implemented with byte-exact tests or explicitly deferred by ADR. |
| P6 | FTN/OxideNet interoperability hardening | Blocked | Real-network/operator feedback or a maintainer-approved compatibility ADR defines Seen-by/PATH tuning, archive-format expansion, mailer scheduling/status gaps, and non-OxideBBS bridge expectations. |
| P7 | OxideNet topology and public-network expansion | Planned | Backup hub, future net ranges, policy-authority workflow, public listing, and reachability validation are either implemented or explicitly kept as reserved capacity. |
| P8 | Final integration and release readiness | Planned | Rust gate, docs build, stale wording scan, release notes, version metadata, and local package smoke checks pass for v1.4. |

## Reviewed Documentation

This plan is based on a review of:

- `README.md`
- `docs/**/*.md`
- `design/RELEASE_v1_1_PLAN.md`
- `design/RELEASE_v1_2_PLAN.md`
- `design/RELEASE_v1_3_PLAN.md`
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

## ADRs For v1.4

| ADR | Topic | Status | Used By |
| --- | --- | --- | --- |
| ADR 0033 | Door compatibility scope | Accepted | P0, P1, P5 |
| ADR 0034 | PETSCII translation and terminal-profile persistence policy | Accepted (PETSCII core shipped in v1.3.0; P3 persistence remains) | P2, P3 |
| ADR 0035 | Caller file-transfer protocol expansion decision | Proposed placeholder | P4 |
| ADR 0036 | FTN/OxideNet interoperability and bridge policy | Proposed placeholder | P6, P7 |

If other ADRs are created before the placeholders, renumber the placeholder
rows before implementation starts.

## v1.4 Candidate Coverage Matrix

Every row below must either be implemented, tested, and documented before v1.4
is declared complete, or explicitly moved out of v1.4 by a maintainer-approved
ADR (recorded as `Deferred` in the phase map).

| Candidate Item | Source Documents | v1.4 Phase |
| --- | --- | --- |
| ~~Full PETSCII encode/decode rendering beyond ASCII fallback~~ Shipped in v1.3.0 | `TASKS.md`, `SPEC.md`, `TELNET.md`, `ANSI_CP437.md`, `PRD.md` | P2 (Complete) |
| Persist manual terminal profile selection in user/account settings | `TASKS.md`, `SPEC.md`, `TELNET.md`, ADR 0034 | P3 |
| YMODEM and XMODEM-1k reconsideration | `FILE_TRANSFERS.md`, ADR 0031, `docs/project/file-transfers.md` | P4 |
| Checksum XMODEM, XMODEM-g, ZedZap/ZMODEM-8K, Kermit, and other transfer variants | `FILE_TRANSFERS.md`, ADR 0031 | P4 |
| Additional Wildcat and vendor-specific drop-file variants | `DOORS.md`, `DOOR_GAME_RESOURCES.md`, `RELEASE_v1_1_PLAN.md`, `RELEASE_v1_2_PLAN.md` | P5 |
| Seen-by and PATH interoperability tuning after real-network feedback | `FTN_PLAN.md`, `OXIDENET_PRD.md`, `RELEASE_v1_1_PLAN.md` | P6 |
| Additional outbound bundle archive formats beyond ZIP | `FTN_PLAN.md`, ADR 0028 | P6 |
| Scheduled polling, external-mailer directory drop docs, and mailer CLI status/queue checklist reconciliation | `MAILER.md`, `RELEASE_v1_2_PLAN.md` | P1, P6 |
| Non-OxideBBS OxideNet participation and FTN-to-internal converter reconciliation | `FTN_PLAN.md`, `OXIDENET_PRD.md`, `RELEASE_v1_2_PLAN.md` | P1, P6 |
| OxideNet backup hub, future net ranges, and public-network topology expansion | `OXIDENET_PRD.md`, `docs/oxidenet/addressing.md`, `RELEASE_v1_2_PLAN.md` | P7 |

## Known Scope Tensions

The v1.4 scope starts with several documentation tensions that must be resolved
before implementation agents treat the plan as executable:

- ADR 0033 defines the door compatibility boundary. v1.4 door work is limited
  to compatibility with existing door games, drop-file formats, provider
  behavior, and sysop tooling.
- `design/RELEASE_v1_2_PLAN.md` says P5 completed Wildcat/PCBoard drop-file
  coverage, while `design/DOORS.md` says additional Wildcat and vendor-specific
  variants remain future compatibility work. v1.4 must identify exact remaining
  variants or move them out of active scope.
- `design/RELEASE_v1_2_PLAN.md` says P15 covered non-OxideBBS participation
  boundaries and an FTN-to-internal converter, while `design/FTN_PLAN.md` and
  `design/OXIDENET_PRD.md` still describe those as post-v1.2 or later
  compatibility. v1.4 must reconcile whether v1.2 delivered documentation
  boundaries only, or whether concrete bridge implementation is still intended.
- `design/MAILER.md` still has unchecked implementation checklist rows for
  scheduled polling, external-mailer directory drop documentation, and CLI
  status/queue views, while the v1.2 release plan marks BinkP and FTN
  operations complete. v1.4 must decide whether those rows are stale checklist
  text, v1.4 compatibility work, or intentionally out of scope.
- ADR 0031 explicitly keeps YMODEM, XMODEM-1k, checksum XMODEM, ZedZap, and
  similar caller-transfer variants outside v1.2 unless a later ADR supersedes
  it. v1.4 cannot implement or advertise those protocols until P4 accepts a new
  transfer policy.

Resolved since the v1.3 draft of this scope:

- ADR 0034 was accepted and its PETSCII encode/decode core shipped in v1.3.0;
  only the terminal-profile persistence work (P3) remains open from that ADR.

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
13. Database migrations must be atomic: each migration runs in a single
    transaction so a failed migration cannot leave the schema between versions
    (lesson from the v1.3.0 review fixes to migrations 5→6 and 6→7).

## P0: Scope Freeze And ADR Baseline

Status: Partial

Objective: Turn the remaining post-v1.3 notes into explicit v1.4 decisions
before implementation starts.

Implementation tasks:

- [x] Accept ADR 0034 (PETSCII translation and terminal-profile persistence
      policy); PETSCII core shipped in v1.3.0.
- Use ADR 0033 as the door compatibility scope boundary.
- Create or update ADRs for transfer protocol expansion (ADR 0035) and
  FTN/OxideNet interoperability (ADR 0036).
- Decide whether v1.4 is allowed to include transfer protocols outside ADR 0031.
- Decide whether OxideNet bridge/converter work is concrete implementation or
  documentation-only boundary work.
- Decide whether remaining mailer checklist rows are stale, v1.4 scope, or
  superseded by v1.2 implementation.
- Decide whether additional Wildcat/vendor drop files are required for v1.4 or
  remain compatibility backlog.
- Update this plan if accepted scope differs from the current draft.

Acceptance criteria:

- Every v1.4 candidate has an accepted ADR or explicit release-plan decision.
- `design/TASKS.md` links to this plan for active v1.4 work.
- Implementation agents can pick a phase without re-litigating release scope.

Validation (release-facing files only; historical release plans and ADRs keep
their original wording by design):

```bash
rg -n -i "post-v1\\.3|outside v1\\.3|PETSCII|YMODEM|XMODEM-1k|Wildcat|non-OxideBBS|FTN-to-internal" \
  README.md docs design/TASKS.md design/PRD.md design/ROADMAP.md config .github
```

## P1: Documentation Reconciliation And Task Tracker Update

Status: Active

Objective: Remove contradictions left after v1.3 so v1.4 has one authoritative
scope story.

Implementation tasks:

- [x] Point `design/TASKS.md` active-work section at this plan for v1.4.
- [x] Update `design/PRD.md` to point to this plan for post-v1.3 candidate work.
- Update `design/ROADMAP.md` to point to this plan for post-v1.3 candidate work.
- Clarify in door docs that v1.4 door work is compatibility with existing door
  games, drop-file formats, provider behavior, and sysop tooling.
- Clarify which Wildcat and vendor-specific drop-file variants remain.
- Clarify whether non-OxideBBS participation and FTN-to-internal conversion were
  completed as v1.2 boundaries or remain v1.4 implementation work.
- Reconcile `design/MAILER.md` checklist rows for scheduled polling,
  external-mailer directory drop docs, and CLI status/queue views against the
  current BinkP/FTN commands.

Acceptance criteria:

- Searches of release-facing files for stale v1.3/future language produce only
  historical references, explicit v1.4 scope, or deliberate reserved-capacity
  notes.
- `design/TASKS.md`, `design/PRD.md`, `design/ROADMAP.md`, and this plan agree
  on v1.4 status.
- No code behavior changes are made in this phase.

Validation:

```bash
rg -n -i "v1\\.4|post-v1\\.3|outside v1\\.3|deferred" \
  README.md docs design/TASKS.md design/PRD.md design/ROADMAP.md config .github
```

## P2: C64/PETSCII Terminal Completion

Status: Complete — shipped in v1.3.0 (2026-08-05).

This phase was completed under the v1.3 plan and released as part of v1.3.0:

- Full PETSCII encode/decode tables and tests in `oxidebbs-term`
  (`decode_petscii`, `petscii_byte_to_char`, `char_to_petscii_byte`,
  `render_petscii`, `render_petscii_lossy`).
- `TerminalCharset::Petscii` (config string `"petscii"`) with the built-in C64
  profile defaulting to it; `petscii_ascii_fallback` remains supported.
- C64 caller output routed through PETSCII-aware rendering at the central
  `encode_text_into` chokepoint; ANSI/CP437 and plain-ASCII behavior unchanged;
  binary file-transfer and telnet negotiation bytes never re-encoded.
- Round-trip, box-drawing, lossy-replacement, 40-column capability, and
  capability-negotiation tests in `oxidebbs-term` and `oxidebbs-server`.

Authoritative record: ADR 0034, `docs/about/changelog.md` `[1.3.0]`, and the
P2 checklist in `design/TASKS.md`.

## P3: Manual Terminal Profile Persistence

Status: Planned

Objective: Let callers or sysops persist terminal profile preference once the
user schema has an explicit terminal preference field.

Implementation tasks:

- Add a DecentDB migration for terminal profile preference on user/account
  records (`SCHEMA_VERSION` 10 → 11). The migration must be atomic per global
  rule 13.
- Define fallback order between telnet terminal-type detection, persisted user
  preference, configured default profile, and manual session override per
  ADR 0034.
- Add onboarding or account-settings flow for manual profile selection.
- Add sysop CLI and TUI edit support for terminal profile preference.
- Document config behavior and caller-facing profile choices.

Acceptance criteria:

- Existing users migrate with no forced terminal preference.
- The migration is transactional: a mid-migration failure rolls the schema
  back to version 10 with no partial columns.
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

Objective: Decide whether v1.4 expands caller file-transfer protocols beyond
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
- If expansion is rejected, record ADR 0035 as preserving ADR 0031 and mark
  this phase `Deferred`.

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
  release plans to say v1.4 intentionally closes this compatibility note, and
  mark this phase `Deferred`.

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

- Open a GitHub tracking issue (or equivalent operator-feedback channel) for
  Seen-by/PATH behavior, archive-format needs, and bridge requirements, and
  link it from `design/MAILER.md` and `design/FTN_PLAN.md`. If no actionable
  feedback exists by v1.4 feature freeze, close the bridge-implementation
  portion of this phase as `Deferred` by ADR 0036 rather than leaving it
  permanently blocked.
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
behavior in v1.4.

Implementation tasks:

- Confirm whether backup hub activation and multi-hub topology management are
  implementation scope or reserved-address documentation only.
- Confirm whether `42:2/*`, `42:3/*`, and `42:100/*` remain future ranges or
  become assignable/validated ranges.
- Define policy-authority group behavior if governance is no longer single-admin.
- Add DNS/BinkP reachability validation for public listings if public listing is
  accepted.
- Document operator runbooks for any public-network behavior implemented in
  v1.4.

Acceptance criteria:

- Address validation, config-package generation, nodelist publication, and TUI
  views agree on accepted ranges.
- Suspended-node and credential-rotation behavior remain enforced.
- Reserved ranges that are not implemented remain clearly labeled as reserved,
  not incomplete v1.4 work.

Validation:

```bash
cargo test -p oxidebbs-oxidenet --locked
cargo test -p oxidebbs-server --locked oxidenet
npm run docs:build
./scripts/dev-check.sh
```

## P8: Final Integration And Release Readiness

Status: Planned

Objective: Prove the accepted v1.4 scope is complete, documented, and
releasable without stale future/backlog language describing shipped behavior as
absent.

Implementation tasks:

- When release preparation begins, bump release metadata with
  `scripts/bump-version.sh 1.4.0` and follow `design/VERSIONING_GUIDE.md`
  (root `Cargo.toml [workspace.package]`, `VERSION`, `Cargo.lock`,
  `package.json`/lock, `compose.yaml`; crate manifests no longer carry their
  own versions).
- Update `docs/about/changelog.md` entries and operator compatibility notes
  (keep an `## [Unreleased]` section; the bump script requires it).
- Update `README.md`, `SECURITY.md`, `design/TASKS.md`, and public docs for the
  accepted v1.4 scope.
- Run stale wording scans and reconcile any hits that describe implemented v1.4
  behavior as future, absent, or outside scope.
- Run Rust validation, docs build, Docker smoke, a validate-mode release
  workflow run, and package smoke checks.

Acceptance criteria:

- Every phase in this plan is `Complete` or `Deferred` by ADR.
- `./scripts/dev-check.sh` passes.
- `npm run docs:build` passes (run from the repository root; the script lives
  in the root `package.json`).
- Local release-package smoke checks pass.
- No tags, pushes, GitHub releases, or hosted publication steps are performed
  without explicit maintainer approval in the current conversation.

Validation:

```bash
./scripts/dev-check.sh
npm run docs:build
rg -n -i "post-v1\\.3|outside v1\\.3|deferred|not implemented|not yet" \
  README.md docs design/TASKS.md design/PRD.md design/ROADMAP.md config .github
```

## Approval-Gated Publication

These steps remain pending until the maintainer explicitly approves tag creation
and release publication in the current conversation:

- [ ] Create and push tag `v1.4.0`.
- [ ] Publish the GitHub release.
- [ ] Confirm hosted Linux, macOS, and Windows release archives and checksums.
- [ ] Download at least one hosted artifact and repeat package smoke testing.
- [ ] Confirm the docs site deployment after publication.

## Final Recommendation

Use v1.4 as a focused compatibility release, not another large deferred-scope
sweep. The highest-confidence scope is manual terminal-profile persistence
(P3), door drop-file compatibility (P5), documentation reconciliation (P1),
and explicit ADR decisions for transfer-protocol (P4) and FTN/OxideNet (P6)
compatibility work — including deciding to defer them. Any protocol expansion
without a new ADR should remain blocked.
