# OxideBBS Initial Task List

This is intended for a local coding agent to work from.

## Phase 1 — Make the skeleton real

- [x] Confirm workspace crate names.
- [x] Add real package metadata to each crate.
- [x] Add common dependencies.
- [x] Add CI workflow.
- [x] Add VitePress documentation site.
- [x] Add `cargo fmt`, `cargo clippy`, and `cargo test` commands to scripts.
- [x] Add a minimal server binary that loads config and logs startup.

## Phase 2 — Configuration

- [x] Define `OxideConfig`.
- [x] Parse TOML config.
- [x] Validate paths.
- [x] Validate node count.
- [x] Validate telnet bind address.
- [x] Add `oxidebbs-server --config config/oxidebbs.example.toml check`.

## Phase 3 — Terminal

- [x] Add ANSI writer helper.
- [x] Add CP437 byte/string helper.
- [ ] Add screen asset loader.
- [ ] Add 40-column screen asset selection.
- [ ] Add plain text fallback renderer.
- [ ] Add width-aware menu/status/pager layout tests.
- [x] Add tests for CP437 round-trip cases.
- [x] Add tests for ANSI escape generation.
- [x] Add ANSI escape sequence parser.

## Phase 4 — Telnet

- [x] Define `Transport` trait.
- [ ] Add telnet parser for IAC sequences.
- [ ] Support basic WILL/WONT/DO/DONT.
- [ ] Add terminal type negotiation later.
- [ ] Add NAWS/window-size support later.
- [ ] Add session lifecycle logging.
- [x] Add integration test with loopback transport.

## Phase 5 — DecentDB

- [x] Add DecentDB dependency/path.
- [x] Add `oxidebbs-db` repository traits.
- [x] Add database open/init routine.
- [x] Add schema version record.
- [x] Add user repository.
- [x] Add audit event repository.
- [x] Add test database fixture.

## Phase 6 — Users and menus

- [x] Add user model.
- [ ] Add password hashing decision ADR.
- [ ] Add new user flow.
- [ ] Add login flow.
- [ ] Add menu model.
- [ ] Add menu command router.
- [ ] Add basic main menu.

## Phase 7 — Doors

- [x] Add door definition model.
- [ ] Parse `doors.toml`.
- [ ] Add node runtime directory handling.
- [ ] Add DOOR.SYS generation.
- [ ] Add DORINFO1.DEF generation.
- [ ] Add dry-run door test.
- [ ] Add DOSBox runner.
- [ ] Add timeout and disconnect cleanup.
- [ ] Record door runs in DecentDB.

## Phase 8 — Messages

- [x] Add message area model.
- [x] Add message model.
- [ ] Add post message command.
- [ ] Add read message command.
- [ ] Add reply command.
- [ ] Add private mail foundation.
- [ ] Add local-only moderation.

## Phase 9 — Sysop CLI/TUI

- [ ] Add admin command group.
- [ ] List users.
- [ ] Reset password.
- [ ] List nodes.
- [ ] Show recent calls.
- [ ] Test door config.
- [ ] Prototype Ratatui sysop console.

## Phase 10 — FTN/OxideNet design

- [ ] Add ADR for FTN abstraction.
- [ ] Define network address model.
- [ ] Define echomail area mapping.
- [ ] Define netmail model.
- [ ] Define duplicate detection approach.
- [ ] Define packet import/export boundary.
