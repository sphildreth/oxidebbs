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
- Keep telnet as the default caller path. Physical serial/modem support remains
  disabled by default and should be enabled only by sysops who intentionally
  configure it.
- Treat ANSI/CP437 as a first-class byte-oriented terminal format.
- Do not use Ratatui for the remote caller UI.
- Keep door execution isolated from core session logic.
- Do not bundle copyrighted or abandonware DOS doors.

## Terminal Profiles

The remote caller UI must support both 80-column and 40-column terminal
profiles. Menus, prompts, status bars, wrapping, paging, and ANSI assets should
fit the active terminal width without corrupting CP437 art or truncating
commands.

| Profile | Caller target | Width | ANSI | Charset behavior |
| --- | --- | --- | --- | --- |
| `ansi80` | BBS/ANSI clients such as SyncTERM | 80 | Yes | CP437 |
| `plain` | Generic telnet clients | 80 | No | ASCII |
| `c64` | C64, C64 Ultimate, and C64 terminal apps | 40 | No by default | PETSCII-friendly ASCII fallback |

The `c64` profile is for callers connecting from Commodore-compatible terminal
software. OxideBBS is not ported to run on C64 hardware.

## Menus

Menus are configured as safe key-to-action mappings. Screen assets draw the
visual menu, while menu entries decide what a pressed key does. For sysops, this
means ANSI art can change without changing command behavior, and command routing
can change without editing art files.

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
