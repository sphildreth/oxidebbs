# Architecture

OxideBBS is a Rust modular monolith. It ships as one primary server binary with
small internal crates for focused boundaries.

```text
Telnet Listener
    |
Transport
    |
Session
    |
Menu / Command Router
    |
Core Services
    |
DecentDB Repository Layer
```

Door execution follows a separate path:

```text
Session
    |
Door Service
    |
Node Manager
    |
Drop File Writer
    |
Door Runner
    |
DOSEMU2 Runtime
    |
PTY/COM1 Bridge
```

## Constraints

- Use Rust.
- Use DecentDB as the only system database.
- Keep the current caller runtime telnet-first until the v1.2 serial/modem
  phase is fully implemented and validated.
- Treat ANSI/CP437 as a first-class byte-oriented terminal format.
- Do not use Ratatui for the remote caller UI.
- Keep door execution isolated from core session logic.
- Do not bundle copyrighted or abandonware DOS doors.

## Terminal Profiles

The remote caller UI must support both 80-column and 40-column terminal
profiles. Menus, prompts, status bars, wrapping, paging, and ANSI assets should
fit the active terminal width without corrupting CP437 art or truncating
commands.

## Menus

Menus are configured as safe key-to-action mappings. Screen assets draw the visual
menu, while menu entries decide what a pressed key does. The current implementation
also includes pure core flows for new-user creation, login state updates, local
message commands, private mail targeting, moderation state changes, and FTN/OxideNet
address and packet-boundary models. Door support has tested drop-file
generation, per-node runtime directories, dry-run planning, DOSEMU2 command
planning, and DecentDB run logging helpers.

## Door byte bridge model

Door launch is separated from caller transport logic.

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOSEMU2-emulated COM1 UART
  <-> DOS door program
```

OxideBBS owns `Transport` and bridge ownership during door sessions. DOSEMU2
remains an isolated runtime that receives caller bytes over a host PTY file and
presents them as COM1 UART bytes. This is not a Rust-hosted FOSSIL driver. A
DOS-side FOSSIL TSR may be loaded by a specific door if that door requires
FOSSIL APIs.
