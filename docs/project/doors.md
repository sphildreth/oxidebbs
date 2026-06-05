# Doors

OxideBBS supports isolated DOS door planning and dry-run validation through
DOSEMU2-oriented runners. Door execution remains separated from core session
logic.

Current capabilities:

- Door definitions are stored in DecentDB after setup/config synchronization.
- `doors list`, `doors check`, `doors test --dry-run`, `doors add`,
  `doors edit`, and `doors dropfile --format` are available through the sysop
  CLI.
- Drop-file writers cover `DOOR.SYS`, `DORINFO1.DEF`, `CHAIN.TXT`,
  `DOORFILE.SR`, `PCBOARD.SYS`, and `CALLINFO.BBS` with CRLF byte-output tests.
- Live caller door validation accepts the same supported drop-file formats.
- Doors marked `exclusive = true` cannot be launched a second time while an
  unfinished run for that same door exists in DecentDB.
- Remote door-provider abstractions, BBSLink-style dry runs, DoorParty-style dry
  runs, TCP/telnet live connectors, and localhost fake-server tests exist.
- Provider credential references are stored in DecentDB through CLI and sysop
  service methods. CLI output, TUI detail display, audit details, and JSON
  exports show provider credential refs only as `[redacted]`.

## DORINFO1.DEF Layout

OxideBBS writes `DORINFO1.DEF` using the legacy 12-line shape expected by
line-number-driven DOS doors:

```text
1  board name
2  sysop first name
3  sysop last name
4  COM port
5  baud string
6  reserved/zero
7  caller first name
8  caller last name
9  caller location
10 ANSI/graphics flag
11 caller security level
12 caller minutes remaining
```

For DOSEMU2 live doors, prefer disabling RTS/CTS hardware handshaking in the
door's own config when it offers those options. OxideBBS bridges a PTY-backed
virtual COM port, not a physical modem with hardware flow-control lines.

Remote provider definitions use the existing door fields:

- `runner = "remote:bbslink"` or `runner = "remote:doorparty"`
- `working_dir` stores the provider endpoint, such as `telnet://host:port`
- `command` stores the provider-side door key or command

Add a remote door with a secret reference, not a raw provider secret:

```bash
oxidebbs-server doors add bbslink-lord "BBSLink LORD" \
  --provider bbslink \
  --endpoint telnet://bbslink.example:23 \
  --credential-ref env:BBSLINK_AUTH_CODE \
  . LORD
```

The `.` argument satisfies the legacy local working-directory positional; for a
remote provider door, `--endpoint` is the value stored as the provider endpoint.

OxideBBS does not bundle copyrighted or abandonware DOS doors. Operators provide
their own door binaries or use the project-owned Oxide Door Check fixture for
validation.
