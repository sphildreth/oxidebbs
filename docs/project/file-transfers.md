# File Transfers

Caller file transfers are available through the configured `files` menu action
when `[file_transfers].enabled = true` and at least one enabled file area exists.

Default configuration:

```toml
[file_transfers]
enabled = false
max_upload_bytes = 1048576
```

Enable the feature, add a menu item that uses `action = "files"`, and create at
least one enabled file area before callers can enter the file-area workflow.
`max_upload_bytes` is a global upload cap; file-area roots and per-area security
levels are managed in DecentDB through the sysop CLI or TUI.

The caller workflow is:

- Select a file area.
- Choose `D` to download or `U` to upload.
- Choose `Z` for ZMODEM or `X` for XMODEM.
- For XMODEM uploads, provide a filename before the protocol starts because
  XMODEM does not carry metadata.

ZMODEM is the primary protocol and supports send, receive, batch protocol
state, retry via `ZRPOS`, cancel handling, metadata parsing, and CRC-32 data
subpackets. XMODEM is the single-file fallback. Downloads prefer CRC mode when
the receiver sends `C` and fall back to classic checksum mode when the receiver
sends `NAK`.

ZMODEM downloads are normally auto-detected by BBS-aware terminal clients.
XMODEM downloads are not auto-started; after OxideBBS prints the transfer start
line, the caller must manually start an XMODEM receive in the terminal client.
OxideBBS waits up to 60 seconds for the receiver's initial CRC or checksum
request byte.

File area roots are sysop-controlled storage directories. Downloaded files are
read from the entry storage name under the area root. Caller uploads are
sanitized to a basename, written under a generated storage name inside the area
root, recorded as unapproved/pending sysop review, and checked against the area
or global upload size limit.

Transfer history is stored in DecentDB with node, user, area, file entry,
direction, protocol, byte counts, duration, outcome, and error details.

Sysop commands:

```bash
oxidebbs-server files areas list
oxidebbs-server files areas add main --name "Main Files" --root files/main --read-level 0 --download-level 10 --upload-level 20
oxidebbs-server files areas edit main --enabled true
oxidebbs-server files list --area main
oxidebbs-server files import main ./uploads/demo.zip --description "Demo archive"
oxidebbs-server files remove <file-id> --reason "duplicate upload"
oxidebbs-server files transfers recent --limit 50
```

`files remove` marks a file entry unapproved and requires a reason. It does not
delete stored file bytes.

YMODEM, XMODEM-1k, Kermit, external `rz`/`sz`, and FTN BinkP mail transport are
not caller file-transfer protocols.
