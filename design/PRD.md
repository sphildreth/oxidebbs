# OxideBBS Product Requirements Document

## 1. Product summary

OxideBBS is a modern BBS software package built in Rust. It targets sysops who want
experience of a classic 1990s BBS with modern reliability, clean deployment,
DecentDB-backed persistence, and first-class support for telnet callers, ANSI/
CP437 screens, DOS door games, and FTN-style message networking.

OxideBBS is software for running a board. It is not itself a single hosted board.

## 2. Target users

### Primary user: hobbyist sysop

A technically comfortable retro-computing enthusiast who wants to run a modern BBS with classic aesthetics.

They value:

- ANSI screens
- Telnet access
- Door games
- Message bases
- Sysop control
- Simple deployment
- Good logging
- Reliable persistence

### Secondary user: retro developer

A developer interested in Rust, terminal systems, DecentDB, telnet, CP437, protocol
work, and BBS internals.

### Secondary user: C64/C64 Ultimate caller

A caller using a native C64, C64 Ultimate, or C64 terminal application to reach
OxideBBS over normal BBS-style network access. This user expects a 40-column,
ASCII/PETSCII-friendly experience that works without modern 80-column ANSI
assumptions.

User story: As a C64 or C64 Ultimate user, I want to connect to OxideBBS using
a native C64 terminal application so that I can use the BBS without needing a
modern ANSI/80-column terminal.

### Secondary user: message-network sysop

A sysop who wants FTN/FidoNet-style shared echomail and netmail using OxideBBS and
possibly OxideNet.

## 3. Product goals

1. Make it enjoyable to run a classic-style BBS in 2026 and beyond.
2. Provide a Rust-native BBS runtime with clean architecture.
3. Use DecentDB as the only system database.
4. Treat ANSI/CP437 as a first-class user experience.
5. Make DOS door launching a core capability.
6. Support disabled-by-default physical modem/serial transport.
7. Support FTN-style message networks without forcing every board to enable them.

## 4. v1 requirements

### Caller access

- Telnet listener
- Multi-node sessions
- Login and new-user flow
- Guest access policy: no guest access by default in v1; callers create or use
  an account before reaching the main menu.
- Idle timeout
- Clean disconnect handling
- Session audit logging

### Terminal experience

- ANSI color output
- CP437-aware byte-oriented rendering
- Configurable login and post-login screens; dedicated terminal welcome/logoff
  fields are present as config metadata, while v1 logoff returns a text goodbye.
- Menu system with hotkeys
- Basic line input
- Paging for long text
- Status bar support
- Supported 40-column and 80-column caller layouts
- Named C64/C64 Ultimate caller compatibility profile (`c64`) for remote
  callers. OxideBBS does not run on C64 hardware; it remains a Rust server
  accessed by C64 clients.
- C64-friendly navigation through 40-column menus, prompts, message lists,
  message reader output, file lists, and plain fallbacks for ANSI art.
- ASCII fallback is required for basic navigation. PETSCII-aware rendering is a
  terminal-profile concern; until full PETSCII translation exists, C64 callers
  must receive usable ASCII/PETSCII-friendly plain output.
- Minimal control sequence assumptions for C64/plain callers: no advanced ANSI
  escape sequences unless explicitly configured for an ANSI-capable profile.
- CR, LF, and CRLF input handling plus `0x08` and `0x7f` backspace/delete
  normalization for login, password, and prompt editing.
- Optional output pacing per terminal profile or connection for slower clients.
- Manual profile selection path during onboarding or account settings when
  terminal detection is unreliable: ANSI / 80-column, plain ASCII, and C64 /
  40-column / PETSCII-friendly.

### User system

- User profile
- Alias
- Real name field, optional
- Password hash
- Security level
- Last login
- Total calls
- Time-left tracking
- Sysop/admin flag

### Message bases

- Local message areas
- Read messages
- Post messages
- Reply to messages
- Private mail foundation
- Sysop moderation primitives

### Door runner

- Door definitions in TOML
- Per-node working directories
- Drop-file generation
- DOSEMU2 runner for v1
- Timeout and disconnect cleanup
- Door run history in DecentDB
- Exclusive-door option
- Door test command for sysop

### Database

- DecentDB-backed storage
- No external SQL database
- Schema/version tracking
- Repository layer
- Write path designed around DecentDB's embedded model

### Sysop/admin

- CLI admin commands
- Local sysop console roadmap
- View active nodes
- View recent calls
- Edit users
- Test door configuration
- View logs
- Configure text or JSON log files with daily or size-based rotation

## 5. v1.1 / v1.2 requirements

All items in this section have been completed as of the v1.2 release.

### FTN-style networking foundation

- Internal network-address model
- Message area mapping
- Echomail-ready local schema
- Netmail-ready local schema
- Duplicate detection strategy
- Packet import/export design

### Sysop TUI

- Ratatui-based local console
- Live nodes view
- Recent events
- Door run status
- Message stats
- Config inspection

## 6. v1.2 shipped scope and post-v1.2 candidates

**Note:** The v1.2 release implements the former deferred-scope items listed in
the active release plan.

The following items were previously listed as v2 or future scope and are now
shipped in v1.2 per
[`design/RELEASE_v1_2_PLAN.md`](./RELEASE_v1_2_PLAN.md) and
[ADR 0018](./adr/0018-v1-2-completes-deferred-scope.md).

- Physical modem/serial transport
- BinkP polling for FTN/FidoNet mail exchange
- FTN packet tosser/scanner
- OxideNet network support
- Web-based read-only status dashboard
- Caller file-area transfers: ZMODEM primary and XMODEM-CRC fallback

Post-v1.2 candidates are tracked in
[`design/RELEASE_v1_3_PLAN.md`](./RELEASE_v1_3_PLAN.md). Door work after v1.2
focuses on compatibility with existing door games, drop-file formats, provider
behavior, and sysop tooling.

## 7. Success criteria

OxideBBS v1 succeeds when:

- It runs reliably as a Linux service.
- Multiple telnet users can connect concurrently.
- ANSI screens render correctly in SyncTERM or equivalent clients.
- Users can log in, navigate menus, and use local message bases.
- At least one DOS door can be launched and exited cleanly.
- Door runs are isolated by node and logged.
- DecentDB is used as the system database without SQLite/PostgreSQL/etc.
- Sysop has usable CLI tools for administration.
