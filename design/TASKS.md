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
- [x] Add interactive setup command for new systems.

## Phase 3 — Terminal

- [x] Add ANSI writer helper.
- [x] Add CP437 byte/string helper.
- [x] Add screen asset loader.
- [x] Add 40-column screen asset selection.
- [x] Add plain text fallback renderer.
- [x] Add width-aware menu/status/pager layout tests.
- [x] Add tests for CP437 round-trip cases.
- [x] Add tests for ANSI escape generation.
- [x] Add ANSI escape sequence parser.

## Phase 4 — Telnet

- [x] Define `Transport` trait.
- [x] Add telnet listener and `serve` runtime.
- [x] Add telnet parser for IAC sequences.
- [x] Support basic WILL/WONT/DO/DONT.
- [x] Add terminal type negotiation later.
- [x] Add NAWS/window-size support later.
- [x] Add node assignment and idle timeout handling.
- [x] Add clean disconnect handling.
- [x] Add session lifecycle logging.
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
- [x] Add password hashing decision ADR.
- [x] Add new user flow.
- [x] Add login flow.
- [x] Add menu model.
- [x] Add menu command router.
- [x] Add basic main menu.

## Phase 7 — Doors

- [x] Add door definition model.
- [x] Parse `doors.toml`.
- [x] Add node runtime directory handling.
- [x] Add DOOR.SYS generation.
- [x] Add DORINFO1.DEF generation.
- [x] Add dry-run door test.
- [x] Add DOSBox runner.
- [x] Add timeout and disconnect cleanup.
- [x] Record door runs in DecentDB.

## Phase 8 — Messages

- [x] Add message area model.
- [x] Add message model.
- [x] Add post message command.
- [x] Add read message command.
- [x] Add reply command.
- [x] Add private mail foundation.
- [x] Add local-only moderation.

## Phase 9 — Sysop CLI/TUI

- [x] Add admin command group.
- [x] List users.
- [x] Reset password.
- [x] List nodes.
- [x] Show recent calls.
- [x] Test door config.
- [x] Prototype Ratatui sysop console.

## Phase 10 — FTN/OxideNet design

- [x] Add ADR for FTN abstraction.
- [x] Define network address model.
- [x] Define echomail area mapping.
- [x] Define netmail model.
- [x] Define duplicate detection approach.
- [x] Define packet import/export boundary.
