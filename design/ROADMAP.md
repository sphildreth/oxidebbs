# OxideBBS Roadmap

## Milestone 0 — Repo foundation

- [x] Workspace compiles
- [x] Documentation baseline
- [x] Example config
- [x] ADRs established
- [x] Basic CI checks

## Milestone 1 — Terminal foundation

- [x] ANSI asset loader
- [x] CP437 conversion helper
- [x] Screen renderer
- [x] Line input
- [x] Pager
- [x] Login and post-login screen routing

## Milestone 2 — Telnet sessions

- [x] Telnet listener
- [x] Telnet negotiation parser
- [x] Transport trait
- [x] Session task lifecycle
- [x] Node assignment
- [x] Idle timeout
- [x] Clean disconnect

## Milestone 3 — Users and menus

- [x] New user flow
- [x] Login flow
- [x] Password hashing
- [x] User stats
- [x] Security level
- [x] Configurable menus
- [x] Hotkey routing

## Milestone 4 — DecentDB persistence

- [x] Database open/init
- [x] Schema/version tracking
- [x] User repository
- [x] Message repository
- [x] Door repository
- [x] Audit event repository
- [x] Direct repository write model documented for v1; DbWriter deferred until
  write contention requires it

## Milestone 5 — Local messages

- [x] Message areas
- [x] Read message
- [x] Post message
- [x] Reply to visible messages
- [x] Private mail foundation
- [x] Moderation primitives

## Milestone 6 — Doors

- [x] Door definition TOML
- [x] Node runtime directories
- [x] DOOR.SYS writer
- [x] DORINFO1.DEF writer
- [x] Door test command
- [x] DOSEMU2 runner
- [x] Timeout cleanup
- [x] Door run logging

## Milestone 7 — Sysop tools

- [x] CLI user management
- [x] CLI node view
- [x] CLI door test
- [x] CLI config check
- [x] Local Ratatui console prototype

## Milestone 8 — FTN/OxideNet foundation

- [x] Network address model
- [x] Echomail-ready schema
- [x] Netmail-ready schema
- [x] Area mapping
- [x] Duplicate detection design
- [x] Packet import/export design

## V1 release-candidate hardening

- [x] Make `.github/workflows/pages.yml` resilient when Pages is disabled by
  enabling it in-workflow (`actions/configure-pages@v5` with
  `enablement: true`) and using an optional `GITHUB_PAGES_TOKEN` fallback to
  `github.token`.
- [x] Reconcile roadmap/spec v1 readiness items before code-complete declaration.
- [x] Implement configured `submenu` runtime behavior.
- [x] Add end-to-end telnet/runtime smoke coverage.
- [x] Add graceful shutdown and lifecycle observability coverage.
- [x] Complete the 2026-06-02 security and performance review phases covering
  telnet exposure/auth hardening, local control socket safety, door runtime
  containment, audit/CP437 policy, caller I/O performance, message query
  performance, and low-risk load-test notes.

## v1.2 Deferred Scope (In Progress)

The following items were previously listed as future scope. They are now tracked
as active v1.2 deferred-scope work per
[`design/RELEASE_v1_2_PLAN.md`](./RELEASE_v1_2_PLAN.md) and
[ADR 0018](./adr/0018-v1-2-completes-deferred-scope.md).

- [ ] Physical serial/modem transport
- [ ] BinkP polling for FTN/FidoNet mail exchange
- [ ] Full FTN tosser/scanner
- [ ] OxideNet network support
- [ ] Caller file-area transfers: ZMODEM primary and XMODEM-CRC fallback
- [x] Dedicated logoff screen rendering
- [x] DbWriter service
