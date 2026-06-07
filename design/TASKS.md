# OxideBBS Tasks

This file tracks active release work and near-term follow-up items. It is not a
replacement for `design/ROADMAP.md`, `docs/about/changelog.md`, or ADRs; it is a
short operational checklist for work that needs explicit closure.

## v1.2.2 Docker Publication Patch

- [x] Bump OxideBBS release metadata to `1.2.2`.
- [x] Publish the smoke-tested Docker image to GitHub Container Registry during
  non-dry-run release workflow runs.
- [x] Make the default Compose file consume the published GHCR image.
- [x] Preserve local Docker source builds through a separate Compose override.
- [x] Document Docker image pull, tag, and source-build workflows.
- [ ] After maintainer approval, create and push tag `v1.2.2`, publish the
  GitHub release, and confirm hosted archives, checksums, docs, Docker image
  publication, and GHCR pull/run behavior.

## v1.2.1 Release Automation Patch

- [x] Bump OxideBBS release metadata to `1.2.1`.
- [x] Convert the release workflow to manual, build-first publication so release
  assets are created before the GitHub release is published.
- [x] Stop uploading assets from matrix jobs after a release has already been
  published.
- [x] Add maintainer release-process documentation and expose it from the docs
  header menu.
- [x] Update the versioning guide for immutable-release-safe publication.
- [x] Finalize the `docs/about/changelog.md` `1.2.1` entry with a release date.
- [x] Update `SECURITY.md` supported versions for the `v1.2.x` release line.
- [x] After maintainer approval, create and push tag `v1.2.1`, publish the
  GitHub release, and confirm hosted archives, checksums, docs, and Docker
  validation.

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
| P2 | Schema, config, and DbWriter foundation | Complete |
| P3 | Caller authorization and flow polish | Complete |
| P4 | Serial/modem transport and file transfers | Complete |
| P5 | Door ecosystem expansion | Complete |
| P6 | Database maintenance operations | Complete |
| P7 | Sysop CLI completion | Complete |
| P8 | Sysop TUI completion | Complete |
| P9 | Shared network foundation | Complete |
| P10 | Legacy FTN packet and message engine | Complete |
| P11 | FTN toss, scan, and bundles | Complete |
| P12 | FTN routing, nodelist, and AreaFix | Complete |
| P13 | BinkP transport | Complete |
| P14 | FTN operations, hardening, and docs | Complete |
| P15 | OxideNet implementation | Complete |
| P16 | Remote admin and status surface | Complete |
| P17 | Repository and release automation | Complete |
| P18 | Final integration and release readiness | Complete |

## Caller Terminal Compatibility

- [x] Add named `c64` terminal profile for C64, C64 Ultimate, and
  PETSCII-friendly 40-column callers.
- [x] Document that C64 support is caller compatibility, not a C64-native
  OxideBBS server port.
- [x] Add config contract for terminal profile width, height, ANSI/color flags,
  charset, line endings, backspace mode, and optional output pacing.
- [x] Add 40-column plain fallback asset slots and starter fallback assets for
  C64/plain narrow callers.
- [x] Cover CR/LF and `0x08`/`0x7f` input behavior in tests.
- [ ] Implement full PETSCII encode/decode rendering beyond ASCII fallback.
- [ ] Persist manual terminal profile selection in user/account settings once
  the user schema has a terminal preference field.

## Web Caller Terminal

- [x] Add `[web_terminal].enabled` config with default `true`.
- [x] Mount `GET /terminal` and `GET /terminal/ws` on the existing `[admin_web]`
  listener when enabled.
- [x] Add websocket transport wrapper with binary frame input/output and
  close/error handling.
- [x] Route browser websocket callers through existing raw caller loop with
  transport name `"websocket"`.
- [x] Add full-viewport terminal frontend with CP437-capable xterm.js UI.
- [x] Add `/terminal/zmodem.js` static endpoint and wire browser xterm frontend
  into `Zmodem.Sentry`.
- [x] Add caller session tracking test assertions for websocket transport rows.
- [x] Add browser-side frontend automated tests for viewport fit and websocket I/O.
- [x] Add browser-side ZMODEM tests and transfer-path coverage.

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
- [x] `db compact --output <path> [--overwrite]` using DecentDB output-file
  compaction.
- [x] Audit retention purge CLI wrapper.
- [x] Dedicated logoff screen rendering from `terminal.logoff_screen`.
- [x] DbWriter service for high-contention write scaling.
- [x] Physical serial/modem caller transport.
- [x] Caller file-area transfers: ZMODEM primary and XMODEM-CRC fallback.
- [x] FTN implementation slice for packet parser/writer, tosser, scanner,
  inbound bundle extraction, outbound ZIP bundle creation, nodelist import,
  lookup, count, differential apply, duplicate detection, netmail routing
  runtime integration, AreaFix, rescan queue processing, packet retention,
  operations stats, BinkP transport, OxideNet, and operational CLI.
- [x] OxideNet implementation beyond the current foundation/design work.
- [x] Remote web admin or status dashboard with a full security model.
- [x] Remote door-provider integrations with credential-reference storage and
  redacted CLI/TUI/audit/export paths.
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

- [x] Local DOS door runtime output sync-back preserves door-created score,
  ranking, hall-of-fame, save, and data files while excluding generated drop
  files and DOSEMU2 bridge files.
- [x] `v1.0.0` initial release shipped.
- [x] Local sysop TUI implemented.
- [x] File logging and log rotation implemented.
- [x] GitHub release-artifact workflow added.
- [x] Caller command and security-level docs added.
- [x] Docker deployment path documented and supported.
