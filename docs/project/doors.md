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
- Remote door-provider abstractions, BBSLink-style dry runs, DoorParty-style dry
  runs, and secret redaction primitives exist.

Remaining v1.2 work:

- Live remote-provider connectors with fake-server integration tests.
- Secret-reference storage and redaction coverage across every CLI, TUI, log,
  backup, and export path.
- Full TUI reuse of the same door mutation service layer.

OxideBBS does not bundle copyrighted or abandonware DOS doors. Operators provide
their own door binaries or use the project-owned Oxide Door Check fixture for
validation.
