# OxideBBS Tasks

This file tracks active release work and near-term follow-up items. It is not a
replacement for `design/ROADMAP.md`, `docs/about/changelog.md`, or ADRs; it is a
short operational checklist for work that needs explicit closure.

## v1.2.0 Release Work

v1.2 is the deferred-scope release described in
[`design/RELEASE_v1_2_PLAN.md`](./RELEASE_v1_2_PLAN.md) and
[ADR 0018](./adr/0018-v1-2-completes-deferred-scope.md).
All implementation agents must work from the release plan rather than
re-opening scope decisions.

| Phase | Title | Status |
| --- | --- | --- |
| P0 | Scope freeze and ADR baseline | Complete |
| P1 | Release hygiene and stale-future sweep | Complete |
| P2 | Schema, config, and DbWriter foundation | Partial |
| P3 | Caller authorization and flow polish | Partial |
| P4 | Serial/modem transport and file transfers | Partial |
| P5 | Door ecosystem expansion | Partial |
| P6 | Database maintenance operations | Blocked |
| P7 | Sysop CLI completion | Partial |
| P8 | Sysop TUI completion | Partial |
| P9 | Shared network foundation | Complete |
| P10 | Legacy FTN packet and message engine | Complete |
| P11 | FTN toss, scan, and bundles | Partial |
| P12 | FTN routing, nodelist, and AreaFix | Partial |
| P13 | BinkP transport | Partial |
| P14 | FTN operations, hardening, and docs | Partial |
| P15 | OxideNet implementation | Partial |
| P16 | Remote admin and status surface | Partial |
| P17 | Repository and release automation | Complete |
| P18 | Final integration and release readiness | Partial |

## v1.1.0 Release Readiness

- [x] Bump all OxideBBS Rust workspace crate versions to `1.1.0`.
- [x] Refresh `Cargo.lock` after the crate version bump.
- [x] Bump documentation package metadata to `1.1.0`.
- [x] Refresh `package-lock.json` after the docs package version bump.
- [x] Finalize the `docs/about/changelog.md` `1.1.0` entry with a release date.
- [x] Add v1.1.0 compatibility notes to the changelog.
- [x] Update `SECURITY.md` supported versions for the `v1.1.x` release line.
- [x] Update `README.md` project status for the `v1.1.x` release line.
- [x] Remove stale current-state pre-release language from release-facing docs.
- [x] Restore this task tracker so process references to `design/TASKS.md` are
  valid again.
- [x] Document v1.1.0 release blockers, validation, and deferred-scope decisions
  in `design/RELEASE_v1_1_PLAN.md`.
- [x] Update release workflow manual-dispatch default to `v1.1.0`.
- [x] Run the Rust CI gate with `./scripts/dev-check.sh`.
- [x] Build the documentation site with `npm run docs:build`.
- [x] Run Docker first-boot smoke testing where Docker is available.
- [x] Run optional live DOSEMU2 smoke testing where DOSEMU2 is available.
- [x] Run local release-package smoke testing for the Linux archive.
- [x] Run stale release-state and stale version-string scans.

## Approval-Gated Publication

These steps remain pending until the maintainer explicitly approves tag creation
and release publication in the current conversation:

- [ ] Create and push tag `v1.1.0`.
- [ ] Publish the GitHub release.
- [ ] Confirm hosted Linux, macOS, and Windows release archives and checksums.
- [ ] Download at least one hosted artifact and repeat package smoke testing.
- [ ] Confirm the docs site deployment after publication.

## Deferred From v1.1.0

**Note:** v1.2 is now consuming these deferred items. Implementation agents
should use [`design/RELEASE_v1_2_PLAN.md`](./RELEASE_v1_2_PLAN.md) for active
scope decisions rather than re-scoping items from this historical list.

The following items were intentionally deferred from `v1.1.0` according to the
recommended decisions in `design/RELEASE_v1_1_PLAN.md`. These are now active
v1.2 tasks; see the phase status map above for current completion state.

- [x] Menu-level `min_security_level` enforcement for caller menu routing.
- [x] Caller-side `S` / Sysop command or sysop submenu.
- [x] CLI/TUI door add/edit workflows that mutate door definitions.
- [x] Additional door drop-file formats such as `CHAIN.TXT`, `DOORFILE.SR`,
  Wildcat variants, and PCBoard variants.
- [ ] `db compact`, pending a safe DecentDB compaction API contract.
- [x] Audit retention purge CLI wrapper.
- [x] Dedicated logoff screen rendering from `terminal.logoff_screen`.
- [x] DbWriter service for high-contention write scaling.
- [ ] Physical serial/modem caller transport.
- [ ] Caller file-area transfers: ZMODEM primary and XMODEM-CRC fallback.
- [ ] Full FTN implementation, including packet parser/writer, tosser, scanner,
  bundles, nodelist, duplicate detection, netmail routing, AreaFix, required
  BinkP transport for FTN/FidoNet mail exchange, and operational CLI.
- [ ] OxideNet implementation beyond the current foundation/design work.
- [ ] Remote web admin or status dashboard with a full security model.
- [ ] Native door API and remote door-provider integrations.
- [x] Codeberg mirror automation.

## Future Backlog

These items were pre-v1.2 backlog notes. They have been absorbed into the v1.2
plan above. Implementation agents should use `design/RELEASE_v1_2_PLAN.md`.

- [x] Decide the next post-v1.1 milestone split between sysop polish, network
  foundation, and caller-facing features. *(Resolved: v1.2 deferred-scope release.)*
- [x] Revisit whether audit retention purge should be a small `v1.1.x` patch or
  part of the next minor release. *(Resolved: v1.2, phase P6.)*
- [x] Revisit dedicated logoff screen rendering as a small caller-flow polish
  item. *(Resolved: v1.2, phase P3.)*
- [x] Keep release artifact workflow behavior documented as package formats and
  supported targets evolve. *(Resolved: v1.2, phase P17.)*

## Recently Completed

- [x] `v1.0.0` initial release shipped.
- [x] Local sysop TUI implemented.
- [x] File logging and log rotation implemented.
- [x] GitHub release-artifact workflow added.
- [x] Caller command and security-level docs added.
- [x] Docker deployment path documented and supported.
