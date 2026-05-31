# OxideBBS Initial Task List

This is intended for a local coding agent to work from.

## Phase 1 — Make the skeleton real

- [ ] Confirm workspace crate names.
- [ ] Add real package metadata to each crate.
- [ ] Add common dependencies.
- [x] Add CI workflow.
- [x] Add `cargo fmt`, `cargo clippy`, and `cargo test` commands to scripts.
- [ ] Add a minimal server binary that loads config and logs startup.

## Phase 2 — Configuration

- [ ] Define `OxideConfig`.
- [ ] Parse TOML config.
- [ ] Validate paths.
- [ ] Validate node count.
- [ ] Validate telnet bind address.
- [ ] Add `oxidebbs-server --config config/oxidebbs.example.toml check`.

## Phase 3 — Terminal

- [x] Add ANSI writer helper.
- [x] Add CP437 byte/string helper.
- [ ] Add screen asset loader.
- [ ] Add 40-column screen asset selection.
- [ ] Add plain text fallback renderer.
- [ ] Add width-aware menu/status/pager layout tests.
- [x] Add tests for CP437 round-trip cases.
- [x] Add tests for ANSI escape generation.

## Phase 4 — Telnet

- [ ] Define `Transport` trait.
- [ ] Add telnet parser for IAC sequences.
- [ ] Support basic WILL/WONT/DO/DONT.
- [ ] Add terminal type negotiation later.
- [ ] Add NAWS/window-size support later.
- [ ] Add session lifecycle logging.
- [ ] Add integration test with loopback transport.

## Phase 5 — DecentDB

- [x] Add DecentDB dependency/path.
- [ ] Add `oxidebbs-db` repository traits.
- [x] Add database open/init routine.
- [x] Add schema version record.
- [ ] Add user repository.
- [ ] Add audit event repository.
- [ ] Add test database fixture.

## Phase 6 — Users and menus

- [ ] Add user model.
- [ ] Add password hashing decision ADR.
- [ ] Add new user flow.
- [ ] Add login flow.
- [ ] Add menu model.
- [ ] Add menu command router.
- [ ] Add basic main menu.

## Phase 7 — Doors

- [ ] Add door definition model.
- [ ] Parse `doors.toml`.
- [ ] Add node runtime directory handling.
- [ ] Add DOOR.SYS generation.
- [ ] Add DORINFO1.DEF generation.
- [ ] Add dry-run door test.
- [ ] Add DOSBox runner.
- [ ] Add timeout and disconnect cleanup.
- [ ] Record door runs in DecentDB.

## Phase 8 — Messages

- [ ] Add message area model.
- [ ] Add message model.
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
