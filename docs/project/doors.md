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
