# Doors

OxideBBS supports isolated DOS door planning and dry-run validation through
DOSEMU2-oriented runners. Door execution remains separated from core session
logic.

Current capabilities:

- Door definitions are stored in DecentDB after setup/config synchronization.
- `doors list`, `doors show`, `doors check`, `doors enable`, `doors disable`,
  `doors package inspect`, `doors test --dry-run`, `doors add`, `doors edit`,
  `doors dropfile --format`,
  `doors runs list/show`, and `doors cleanup` are available through the sysop
  CLI.
- Drop-file writers cover `DOOR.SYS`, `DORINFO1.DEF`, `CHAIN.TXT`,
  `DOORFILE.SR`, `PCBOARD.SYS`, and `CALLINFO.BBS` with CRLF byte-output tests.
- Live caller door validation accepts the same supported drop-file formats.
- Doors marked `exclusive = true` cannot be launched a second time while an
  unfinished run for that same door exists in DecentDB.
- Live local DOS doors stage into a per-node runtime directory and sync
  door-owned output files back to the configured door directory before cleanup.
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

## Local Door Persistence

Local DOS doors are launched from a per-node runtime directory so generated
drop files, DOSEMU2 bridge files, and temporary node state do not pollute the
configured door directory. Before the runtime directory is cleaned, OxideBBS
syncs door-owned files back to the configured door directory. This preserves
scoreboards, hall-of-fame files, saved games, and other door-created data.

The sync excludes generated root-level runtime files:

- `DOOR.SYS`
- `DORINFO1.DEF`
- `CHAIN.TXT`
- `DOORFILE.SR`
- `PCBOARD.SYS`
- `CALLINFO.BBS`
- `OXNODE.TXT`
- `OXDOSEMU2.CONF`
- `OXCOM1.PTY`

Disk-image-backed door installs are intentionally deferred. Prefer normal host
directories for doors that store loose `.DAT`, `.CFG`, `.RNX`, `.SCR`, or save
files. If a future door requires a persistent DOS disk image, configure it as
exclusive unless the image is mounted per-node and does not share mutable state.

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

Door run history is visible through:

```bash
oxidebbs-server doors package inspect sample.oxdoor
oxidebbs-server doors runs list
oxidebbs-server doors runs show <run-id>
oxidebbs-server doors cleanup
```

`doors cleanup` removes leftover `node-*` door runtime directories under
`paths.runtime`. It does not rewrite persisted door-run history.

OxideBBS does not bundle copyrighted or abandonware DOS doors. Operators provide
their own door binaries or use the project-owned Oxide Door Check fixture for
validation.
