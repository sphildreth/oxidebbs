# File Transfers

Caller file transfers are available through the configured `files` menu action
when `[file_transfers].enabled = true` and at least one enabled file area exists.

The caller workflow is:

- Select a file area.
- Choose `D` to download or `U` to upload.
- Choose `Z` for ZMODEM or `X` for XMODEM-CRC.
- For XMODEM-CRC uploads, provide a filename before the protocol starts because
  XMODEM does not carry metadata.

ZMODEM is the primary protocol and supports send, receive, batch protocol
state, retry via `ZRPOS`, cancel handling, metadata parsing, and CRC-32 data
subpackets. XMODEM-CRC is the single-file fallback.

ZMODEM downloads are normally auto-detected by BBS-aware terminal clients.
XMODEM-CRC downloads are not auto-started; after OxideBBS prints the transfer
start line, the caller must manually start an XMODEM-CRC receive in the terminal
client. OxideBBS waits up to 60 seconds for the receiver's initial CRC request
byte.

File area roots are sysop-controlled storage directories. Downloaded files are
read from the entry storage name under the area root. Caller uploads are
sanitized to a basename, written under a generated storage name inside the area
root, recorded as unapproved/pending sysop review, and checked against the area
or global upload size limit.

Transfer history is stored in DecentDB with node, user, area, file entry,
direction, protocol, byte counts, duration, outcome, and error details.

YMODEM, XMODEM-1k, Kermit, external `rz`/`sz`, and FTN BinkP mail transport are
not caller file-transfer protocols for this release plan.
