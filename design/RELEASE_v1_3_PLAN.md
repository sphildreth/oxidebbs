# OxideBBS v1.3 Release Plan

Document status: Closed — released as `v1.3.0` on 2026-08-05

Created: 2026-06-05

Closed: 2026-08-05

## What v1.3.0 Actually Shipped

The original draft of this plan scoped `v1.3.0` as a broad post-v1.2
compatibility release (phases P0–P8). When release preparation began, the
completed work in the changelog's `Unreleased` section was released as
`v1.3.0` under the SemVer highest-impact rule in
[`design/VERSIONING_GUIDE.md`](./VERSIONING_GUIDE.md):

- Full PETSCII encode/decode for the C64 terminal profile (phase P2 core;
  ADR 0034), including `TerminalCharset::Petscii`, config support, and
  charset-aware caller output at the central encoding chokepoint.
- CP437 low-range glyph decode/encode (`0x01..=0x1F`, `0x7F`) in
  `oxidebbs-term`.
- ECMA-48 private parameter byte support in the ANSI parser.
- ANSI parser bounds/overflow hardening, `insert_network_path` rollback
  atomicity, and transactional migrations 5→6 and 6→7.
- Workspace dependency hygiene: crate versions centralized under
  `[workspace.package]`, unused dependencies removed.
- Documentation corrections across `AGENTS.md`, `design/ARCHITECTURE.md`,
  `design/SPEC.md`, and the Rust code-generation skill.

The authoritative record is `docs/about/changelog.md` `[1.3.0] - 2026-08-05`
and the P2 checklist in [`design/TASKS.md`](./TASKS.md).

## What Moved To v1.4

The remaining phases from this plan's original scope — manual terminal profile
persistence (P3), caller transfer protocol decision (P4), door drop-file
compatibility expansion (P5), FTN/OxideNet interoperability hardening (P6),
OxideNet topology expansion (P7), and the associated documentation
reconciliation (P0/P1 remainder) — now live in
[`design/RELEASE_v1_4_PLAN.md`](./RELEASE_v1_4_PLAN.md), which also carries the
current phase status map and ADR table.

The original draft text of this plan is preserved in git history prior to the
v1.3.0 release.
