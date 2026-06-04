# File Transfers

Caller file-transfer support is part of the v1.2 release plan and remains in
progress.

Current foundation:

- DecentDB tables and repository APIs for file areas, file entries, and transfer
  history.
- Sysop CLI commands for file-area administration, file import/removal, and
  recent transfer listing.
- XMODEM-CRC send/receive fallback primitives in `oxidebbs-transfer`.
- ZMODEM binary and hex header framing primitives in `oxidebbs-transfer`.

Not release-ready yet:

- Caller-facing file-area menus and upload/download workflows.
- Full ZMODEM send/receive state machines.
- End-to-end transfer handshakes, retries, cancel handling, path sanitization,
  and telnet IAC escaping coverage.

The intended protocol boundary remains ZMODEM as the primary caller transfer
protocol and XMODEM-CRC as the required fallback. YMODEM, XMODEM-1k, Kermit,
external `rz`/`sz`, and FTN BinkP mail transport are not caller file-transfer
protocols for this release plan.
