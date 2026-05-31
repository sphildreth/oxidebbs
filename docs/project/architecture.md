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
DOS Runtime
```

## Constraints

- Use Rust.
- Use DecentDB as the only system database.
- Keep v1 telnet-only.
- Treat ANSI/CP437 as a first-class byte-oriented terminal format.
- Do not use Ratatui for the remote caller UI.
- Keep door execution isolated from core session logic.
- Do not bundle copyrighted or abandonware DOS doors.

## Terminal Profiles

The remote caller UI must support both 80-column and 40-column terminal
profiles. Menus, prompts, status bars, wrapping, paging, and ANSI assets should
fit the active terminal width without corrupting CP437 art or truncating
commands.
