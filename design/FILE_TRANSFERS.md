# OxideBBS Caller File Transfer Design

Document status: v1.2 implementation specification

Created: 2026-06-03

Applies to: `design/RELEASE_v1_2_PLAN.md` P4 and ADR 0031

## Scope

This document defines the caller file-transfer subsystem for v1.2. It is an
implementation specification for coding agents. Where historical protocols
allow incompatible behavior, this document chooses the OxideBBS behavior.

The v1.2 caller-transfer scope is:

- ZMODEM send and receive as the primary protocol.
- XMODEM-CRC send and receive as the fallback protocol.
- Transport-agnostic protocol engines that work over telnet and serial caller
  transports.
- File-area metadata, access control, path safety, transfer history, and
  end-to-end tests.

The v1.2 caller-transfer scope is not:

- YMODEM, YMODEM batch, YMODEM-g, or YMODEM-1k.
- XMODEM checksum mode.
- XMODEM-1k.
- XMODEM-g.
- ZedZap, ZMODEM-8K, Kermit, SEAlink, Hydra, Janus, FTP, HTTP, SCP, SFTP, or
  BinkP.
- Shelling out to `rz`, `sz`, `rx`, `sx`, `rb`, `sb`, or other external
  transfer tools at runtime.

YMODEM and XMODEM-1k can be reconsidered only through a later ADR that
supersedes ADR 0031. They must not appear in v1.2 caller menus, config examples,
or release notes.

## Layering

The implementation must create an `oxidebbs-transfer` crate. That crate owns
the protocol state machines. It must not depend on `oxidebbs-server`.

Expected dependency direction:

```text
oxidebbs-server -> oxidebbs-core -> oxidebbs-transfer
oxidebbs-server -> oxidebbs-telnet
oxidebbs-server -> oxidebbs-db
```

The protocol crate must expose byte-oriented APIs. Caller UI text remains
ANSI/CP437, but transfer engines must operate on bytes and must not decode file
payloads as text.

The transfer crate must not know about DecentDB. The server/core layer maps
transfer events to DecentDB rows.

Recommended crate modules:

```text
crates/oxidebbs-transfer/src/
  lib.rs
  error.rs
  transport.rs
  progress.rs
  xmodem.rs
  zmodem/
    mod.rs
    constants.rs
    crc.rs
    escape.rs
    header.rs
    subpacket.rs
    session.rs
  tests/
```

## Runtime Ownership

The caller session owns the transfer lifecycle. A transfer suspends normal menu
input and terminal rendering until the protocol finishes, aborts, or times out.

The caller session must:

- Flush pending text output before entering transfer mode.
- Stop CP437 text decoding for inbound bytes during the active transfer.
- Stop ANSI screen rendering for outbound bytes during the active transfer.
- Route bytes directly between the transport and the transfer protocol engine.
- Restore the normal caller input mode after cleanup.
- Record transfer outcome even when the transfer fails.

The transfer protocol engine must:

- Receive bytes from a `ByteTransport`.
- Send bytes through the same `ByteTransport`.
- Report progress through structured callbacks/events.
- Return typed errors.
- Avoid `unwrap()` and `expect()` in library code.
- Avoid holding locks across `.await`.

Suggested API shape:

```rust
pub enum TransferProtocol {
    XmodemCrc,
    Zmodem,
}

pub enum TransferDirection {
    SendToCaller,
    ReceiveFromCaller,
}

pub struct TransferRequest {
    pub protocol: TransferProtocol,
    pub direction: TransferDirection,
    pub files: Vec<TransferFileSpec>,
    pub limits: TransferLimits,
    pub behavior: TransferBehavior,
}

pub enum TransferRead {
    Byte(u8),
    TimedOut,
    Closed,
}

pub trait ByteTransport {
    async fn read_byte(&mut self, timeout: Duration) -> Result<TransferRead, TransferError>;
    async fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransferError>;
    async fn flush(&mut self) -> Result<(), TransferError>;
}
```

The actual Rust API may differ if the surrounding codebase suggests a better
local pattern, but it must preserve this separation: protocol state in
`oxidebbs-transfer`, caller/session orchestration in server/core, and persistence
in `oxidebbs-db`.

## Transport Adapter

The current `oxidebbs-telnet::Transport` trait is not the same as the
protocol-facing `ByteTransport` sketched above. Integration must use an adapter
rather than changing protocol code to depend directly on telnet.

Existing transport shape:

```rust
pub trait Transport: Send {
    fn read_byte(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Option<u8>, TransportError>> + Send;

    fn write_all(
        &mut self,
        bytes: &[u8],
    ) -> impl std::future::Future<Output = Result<(), TransportError>> + Send;

    fn hangup(&mut self) -> impl std::future::Future<Output = Result<(), TransportError>> + Send;
}
```

Adapter rules:

- Wrap `Transport::read_byte()` in `tokio::time::timeout()` to implement
  protocol-level read deadlines.
- Map timeout expiration to `TransferRead::TimedOut`.
- Map `Ok(Some(byte))` to `TransferRead::Byte(byte)`.
- Map `Ok(None)` to `TransferRead::Closed`.
- Map `TransportError` to `TransferError::Transport`.
- Delegate `write_all()` to `Transport::write_all()`.
- Implement `flush()` as a no-op until the lower-level transport exposes a real
  flush. `write_all()` already awaits the current write operation.
- Do not expose `hangup()` to the transfer crate. Caller disconnect, sysop kill,
  and final session hangup remain server/session responsibilities.

Telnet needs a second adapter layer during binary transfers:

```text
oxidebbs-transfer protocol engine
  -> BinaryTelnetTransferAdapter
  -> oxidebbs-telnet::Transport
```

`BinaryTelnetTransferAdapter` must parse telnet commands, convert inbound
`IAC IAC` to one payload byte `0xFF`, and double outbound payload byte `0xFF`
as `IAC IAC`. It must not use the normal line editor or text-mode
`TelnetConnection` event stream while the transfer is active.

## File-Area Model

P4 adds these DecentDB-backed concepts:

- `file_areas`: configured file libraries.
- `file_entries`: files visible to callers.
- `file_transfers`: per-transfer audit/history rows.

Minimum `file_areas` fields:

- `id`: stable UUID.
- `key`: ASCII sysop-facing key, unique case-insensitively.
- `name`: display name.
- `description`: optional display text.
- `root_path`: canonical root directory for this area.
- `read_security_level`: minimum level to list files.
- `download_security_level`: minimum level to download files.
- `upload_security_level`: minimum level to upload files.
- `max_upload_bytes`: nullable per-area override.
- `enabled`: boolean.
- `created_at`, `updated_at`.

Minimum `file_entries` fields:

- `id`: stable UUID.
- `area_id`: foreign key.
- `storage_name`: sanitized basename used on disk.
- `display_name`: caller-visible name after sanitization.
- `original_name`: optional original protocol metadata name.
- `size_bytes`: committed file size.
- `content_crc32`: CRC-32 of committed bytes for inventory checks.
- `description`: optional sysop/caller description.
- `uploader_user_id`: nullable for sysop-imported files.
- `download_count`.
- `approved`: boolean. Uploads should default to unapproved unless config says
  the area is auto-approved.
- `created_at`, `updated_at`.

Minimum `file_transfers` fields:

- `id`: stable UUID.
- `node_id`.
- `user_id`.
- `area_id`: nullable until upload target is known.
- `file_entry_id`: nullable for failed uploads or protocol startup failures.
- `direction`: `download` or `upload`.
- `protocol`: `zmodem` or `xmodem_crc`.
- `requested_name`: nullable original caller/server requested filename.
- `storage_name`: nullable committed storage filename.
- `declared_size_bytes`: nullable protocol/user-provided size.
- `transferred_payload_bytes`: bytes accepted from or sent to the protocol,
  excluding telnet IAC escaping and excluding ZMODEM/XMODEM frame overhead.
- `committed_size_bytes`: nullable final file size.
- `started_at`, `ended_at`.
- `duration_ms`.
- `outcome`: `success`, `skipped`, `canceled_by_caller`,
  `canceled_by_server`, `timeout`, `protocol_error`, `io_error`,
  `security_denied`, `quota_denied`.
- `error_code`: nullable stable machine-readable code.
- `error_message`: nullable sysop-facing message. Do not include sensitive host
  paths in caller-facing text.
- `retry_count`.

## Path Sanitization

All filenames received from callers or external clients are untrusted.

The server must never use a protocol-provided path as a filesystem path. It must
extract and sanitize a basename, then store uploaded bytes under a path chosen by
OxideBBS.

Rules for inbound filenames:

- Reject empty names.
- Reject names longer than 255 bytes before sanitization.
- Reject absolute paths.
- Reject drive prefixes such as `C:`, `C:\`, `C:/`, `AUX:`, or `COM1:`.
- Reject path separators: `/`, `\`, and repeated separator lookalikes.
- Reject `.` and `..`.
- Reject names containing ASCII control bytes `0x00..0x1F` or `0x7F`.
- Reject names containing telnet command bytes after telnet decoding has run.
- Reject names with leading or trailing spaces after trimming.
- Reject names whose sanitized form is empty.
- Treat case-insensitive collisions as collisions.

Allowed storage basename characters:

```text
A-Z a-z 0-9 . _ - space
```

Sanitization algorithm:

1. Decode display metadata as UTF-8 if valid. If invalid, decode bytes with the
   existing CP437 helper for display only.
2. Convert disallowed storage characters to `_`.
3. Collapse repeated `_` characters to one `_`.
4. Trim leading and trailing spaces, dots, and underscores.
5. Limit the storage basename to 120 bytes, preserving the extension when
   possible.
6. If the result is empty, use `upload-<transfer-id>`.
7. If the target storage name already exists in the area, append
   `-<short-transfer-id>` before the extension.

Storage layout:

```text
<area root>/
  files/
    <file-entry-id>/
      <storage-name>
  incoming/
    <transfer-id>.part
```

Uploads must write to `incoming/<transfer-id>.part` first. On success, move the
temporary file into `files/<file-entry-id>/<storage-name>` and then create or
update the `file_entries` row. On failure, delete the partial file unless config
explicitly enables partial retention for sysop debugging.

The implementation must verify that the final canonical path remains under the
canonical area root. Because the remote filename is reduced to a basename and
OxideBBS chooses the parent directory, path traversal should be impossible even
before canonicalization. The canonical containment check is still required.

## Transfer Limits

The caller must pass all security checks before a protocol starts.

Download checks:

- Area is enabled.
- File entry is enabled/approved.
- Caller security level is at least `download_security_level`.
- Sysop policy does not block the caller.

Upload checks:

- Area is enabled.
- Caller security level is at least `upload_security_level`.
- Upload filename passes sanitization.
- Declared or observed bytes do not exceed:
  - per-area `max_upload_bytes`, if set
  - global `file_transfers.max_upload_bytes`
  - available disk-space safety threshold

If an upload exceeds a limit mid-transfer, cancel the protocol, delete the
temporary file, and persist `quota_denied`.

## Telnet Binary Transfer Behavior

File-transfer protocols are binary protocols. They must not run through the
normal text path.

For telnet callers, the server must enter a `BinaryTransferGuard` before
starting XMODEM-CRC or ZMODEM:

1. Flush pending caller text.
2. Send telnet negotiation for binary transmission in both directions:
   - `IAC WILL TRANSMIT-BINARY`
   - `IAC DO TRANSMIT-BINARY`
3. Wait up to `telnet_binary_negotiation_timeout`, default `2s`.
4. If both directions are accepted, start the protocol.
5. If either direction is refused or times out:
   - XMODEM-CRC must abort before sending file bytes.
   - ZMODEM must abort by default.
   - A future config escape hatch may allow ZMODEM over non-binary telnet with
     aggressive ZMODEM control escaping, but v1.2 default behavior must prefer a
     clean failure over silent file corruption.

While in binary transfer mode:

- The telnet transport still parses telnet commands.
- Incoming `IAC IAC` must be delivered to the transfer engine as one data byte
  `0xFF`.
- Incoming `IAC <command>` sequences must be handled by the telnet layer and
  must not be delivered as protocol payload bytes.
- Outgoing protocol data byte `0xFF` must be encoded on the telnet stream as
  `IAC IAC`.
- No CR/LF normalization is allowed.
- No CP437 decoding is allowed.
- No ANSI filtering is allowed.
- No line editor, menu hotkey, pager, or prompt parser receives transfer bytes.

ZMODEM escaping and telnet IAC escaping are separate layers:

```text
file bytes -> ZMODEM escape/framing -> telnet IAC escaping -> TCP
TCP -> telnet command parser/IAC unescaping -> ZMODEM unescape/framing -> file bytes
```

For serial callers, no telnet IAC handling exists. Serial transfers use the raw
byte stream.

## Session State Machine

The caller session must use explicit states so normal menu input cannot race
with protocol bytes.

Required states:

```text
MenuIdle
FileAreaMenu
TransferPreflight
TransportBinaryNegotiation
TransferStarting
TransferActive
TransferCompleting
TransferAborting
TransferRestoringTerminal
MenuIdle
```

Required transition shape:

```text
MenuIdle
  -> FileAreaMenu
  -> TransferPreflight
      -> TransportBinaryNegotiation
          -> TransferStarting
              -> TransferActive
                  -> TransferCompleting
                      -> TransferRestoringTerminal
                          -> MenuIdle

TransferPreflight
  -> TransferRestoringTerminal
      -> MenuIdle

TransportBinaryNegotiation
  -> TransferAborting
      -> TransferRestoringTerminal
          -> MenuIdle

TransferStarting
  -> TransferAborting
      -> TransferRestoringTerminal
          -> MenuIdle

TransferActive
  -> TransferAborting
      -> TransferRestoringTerminal
          -> MenuIdle
```

State details:

- `TransferPreflight`
  - Checks security, file existence, upload destination, limits, and disk space.
  - Prompts for and validates the upload filename before protocol start when
    `direction = ReceiveFromCaller` and `protocol = XmodemCrc`, because XMODEM
    carries no filename metadata. This prompt runs in normal text mode before
    `TransportBinaryNegotiation`.
  - Creates a `file_transfers` row with `started_at`.
  - Creates upload temp file if receiving from caller.
- `TransportBinaryNegotiation`
  - Enters telnet binary mode or confirms raw serial mode.
  - Fails before protocol start when the transport cannot support safe binary.
- `TransferStarting`
  - Sends the protocol startup sequence.
  - Starts timers.
  - Initializes progress counters.
- `TransferActive`
  - Protocol engine owns all caller bytes.
  - Session heartbeat must continue so the sysop UI can see the node is active.
    This is a server/core session concern; `oxidebbs-transfer` must report
    progress events but must not know about node heartbeats or sysop UI state.
  - The node state should include protocol, direction, filename, bytes, and
    retry count.
- `TransferCompleting`
  - Flushes protocol output.
  - Commits upload temp file or increments download counters.
  - Updates `file_transfers`.
- `TransferAborting`
  - Sends protocol cancel sequence when possible.
  - Deletes upload temp file unless partial retention is enabled.
  - Updates `file_transfers`.
- `TransferRestoringTerminal`
  - Restores telnet text handling.
  - Sends a clean caller-facing status line.
  - Returns to menu rendering.

Any error from `TransferStarting` or `TransferActive` must transition through
`TransferAborting` and `TransferRestoringTerminal`.

## Progress Events

The transfer crate must emit structured progress events:

```rust
pub enum TransferEvent {
    Started { protocol, direction, files_total },
    FileStarted { index, name, declared_size },
    BytesAdvanced { file_bytes_done, session_bytes_done },
    Retry { reason, retry_count },
    Resumed { offset },
    Skipped { name, reason },
    Canceled { by },
    Completed { files_completed, bytes },
}
```

The server should use these events for:

- `file_transfers` history.
- Sysop node status.
- Logs.
- Caller-facing final result text after the binary transfer ends.

Do not print progress UI during transfer unless the protocol permits it. During
XMODEM and ZMODEM active transfer, arbitrary text written to the caller stream
will corrupt the transfer.

## XMODEM-CRC

XMODEM-CRC is the required fallback protocol. It is single-file only, has no
metadata, has no resume support, and uses fixed 128-byte data blocks.

### XMODEM-CRC Control Bytes

| Name | Hex | Decimal | Meaning |
| --- | --- | ---: | --- |
| `SOH` | `0x01` | 1 | Starts a 128-byte data block. |
| `EOT` | `0x04` | 4 | Sender has no more data. |
| `ACK` | `0x06` | 6 | Receiver accepts block or end-of-transfer. |
| `NAK` | `0x15` | 21 | Receiver rejects block. Used only after CRC mode starts or for controlled abort compatibility. |
| `CAN` | `0x18` | 24 | Cancel byte. OxideBBS treats repeated CAN as abort. |
| `CRC_REQ` | `0x43` | 67 | ASCII `C`; receiver requests CRC mode. |
| `CPMEOF` | `0x1A` | 26 | Padding byte for final partial block. |

Checksum-only XMODEM is not supported. OxideBBS must not fall back from
`CRC_REQ` to `NAK` checksum mode. If the peer does not answer CRC requests, the
transfer fails as `protocol_error` or `timeout`.

### XMODEM-CRC Frame Format

Each data block is exactly 133 bytes:

```text
SOH block_number block_number_ones_complement data[128] crc_hi crc_lo
```

Rules:

- `block_number` starts at `0x01`.
- `block_number` increments by one after every accepted block.
- `block_number` wraps from `0xFF` to `0x00`, not back to `0x01`.
- `block_number_ones_complement` must equal `block_number ^ 0xFF`.
- `data` is exactly 128 bytes.
- The final block is padded with `0x1A` bytes when the source file is not an
  exact multiple of 128 bytes.
- The CRC is computed over the 128 data bytes only.
- CRC bytes are sent high byte first, then low byte.

### XMODEM-CRC CRC Algorithm

Use CRC-16/XMODEM:

- Width: 16 bits.
- Polynomial: `0x1021`.
- Initial value: `0x0000`.
- Input reflected: no.
- Output reflected: no.
- Final XOR: `0x0000`.
- Check value: CRC of ASCII `123456789` is `0x31C3`.

Implement this directly or use a reviewed checksum helper crate. If adding a
crate, use `cargo add` and justify it in implementation notes.

### XMODEM-CRC Download Sequence

Download means OxideBBS sends a file to the caller.

Required sequence:

```text
caller -> server: CRC_REQ
server -> caller: SOH 01 FE data[128] crc_hi crc_lo
caller -> server: ACK
server -> caller: SOH 02 FD data[128] crc_hi crc_lo
caller -> server: ACK
...
server -> caller: EOT
caller -> server: ACK
```

Server behavior:

- Wait for `CRC_REQ`.
- Ignore printable garbage before the first `CRC_REQ` up to
  `xmodem_startup_garbage_limit`, default 256 bytes.
- Abort after `xmodem_startup_retries`, default 10, if no `CRC_REQ` arrives.
- Send one block and wait for `ACK`, `NAK`, repeated `CAN`, or timeout.
- On `ACK`, advance to the next block.
- On `NAK`, resend the same block.
- On timeout, resend the same block.
- On repeated `CAN`, abort.
- After 10 failed attempts for the same block, abort.
- Send `EOT` after the final data block.
- Resend `EOT` until `ACK` or until the EOT retry limit is reached.

Download size handling:

- The source file size is known from `file_entries`.
- The protocol sends padded bytes, but `file_transfers.committed_size_bytes`
  and download counters must record the real file size.
- `transferred_payload_bytes` should record real file bytes, not padding.
- A lower-level diagnostic counter may record padded protocol payload bytes.

### XMODEM-CRC Upload Sequence

Upload means OxideBBS receives a file from the caller.

Required sequence:

```text
server -> caller: CRC_REQ
caller -> server: SOH 01 FE data[128] crc_hi crc_lo
server -> caller: ACK
caller -> server: SOH 02 FD data[128] crc_hi crc_lo
server -> caller: ACK
...
caller -> server: EOT
server -> caller: ACK
```

Server behavior:

- The caller must choose the target file area before the protocol starts.
- The caller must provide an upload filename before XMODEM starts because
  XMODEM carries no filename.
- The caller may provide a declared byte size before XMODEM starts.
- Send `CRC_REQ` every `3s` until the first `SOH`, `EOT`, or cancel sequence.
- Abort after 10 unanswered `CRC_REQ` attempts.
- After receiving `SOH`, read block number, complement, 128 data bytes, and CRC
  with a `1s` per-byte timeout.
- Validate complement, expected block number, and CRC.
- On success, append the 128 data bytes to the temp file and send `ACK`.
- On CRC/header/timeout failure, purge the incoming line until `1s` of silence,
  send `NAK`, and retry the same expected block.
- If the peer repeats the previous valid block, send `ACK` again but do not
  append the duplicate bytes.
- If the peer sends any other block number, abort as loss of synchronization.
- On `EOT`, send `ACK`, finalize temp file, and commit metadata.

Upload size handling:

- If a declared size was supplied, truncate the committed file to that exact
  size after successful transfer. The declared size must be less than or equal
  to the padded bytes received.
- If no declared size was supplied, commit the full received payload including
  final `0x1A` padding and mark the file entry/transfer as
  `size_source = padded_xmodem`. Do not guess by stripping trailing `0x1A`
  because `0x1A` can be legitimate binary data.
- Caller-facing docs must recommend ZMODEM for uploads because it carries file
  size metadata.

### XMODEM-CRC Timeouts And Retries

Defaults:

| Setting | Default | Applies To |
| --- | ---: | --- |
| `xmodem_startup_interval` | `3s` | Receiver repeats `CRC_REQ` before first block. |
| `xmodem_receiver_idle_timeout` | `10s` | Receiver waits for first byte of next block or `EOT`. |
| `xmodem_byte_timeout` | `1s` | Receiver waits for remaining bytes after `SOH`. |
| `xmodem_sender_response_timeout` | `10s` | Sender waits for `ACK`, `NAK`, or cancel. |
| `xmodem_block_retries` | `10` | Failed attempts for one block. |
| `xmodem_startup_retries` | `10` | Unanswered startup requests. |
| `xmodem_eot_retries` | `10` | Unacknowledged `EOT`. |

These defaults are intentionally conservative. Config may expose them later, but
v1.2 implementation should keep constants internal unless config is added in the
same phase and documented.

### XMODEM-CRC Cancel Behavior

Sending cancel:

- Send eight `CAN` bytes.
- Send eight `BS` bytes (`0x08`) after the CAN bytes to reduce terminal garbage
  if the peer has already returned to command mode.
- Flush the transport.

Receiving cancel:

- One `CAN` byte alone is not enough to abort.
- Two consecutive `CAN` bytes outside a data block should abort.
- Five or more consecutive `CAN` bytes must abort.
- A `CAN` byte inside a data block is payload and must not abort.

After cancel, delete upload temp files and record `canceled_by_caller` or
`canceled_by_server`.

## ZMODEM

ZMODEM is the primary caller file-transfer protocol for v1.2. OxideBBS must
implement owned Rust ZMODEM state machines. Runtime use of external `rz`/`sz`
programs is forbidden.

Required ZMODEM capabilities:

- Send files to caller.
- Receive files from caller.
- Send and receive repeated `ZFILE` cycles in one session for batch transfers.
- Binary file transfer using `ZCBIN`.
- File metadata parsing and sanitization.
- Resume from a receiver-provided offset with `ZRPOS`.
- CRC-32 frame/data checks when the peer advertises `CANFC32`.
- CRC-16 fallback when the peer does not advertise `CANFC32`.
- Cancel and abort handling.
- Telnet-safe escaping.

Not required in v1.2:

- `ZCOMMAND` command execution.
- Encryption.
- Compression.
- RLE.
- Sparse file handling.
- Server mode commands.
- ZedZap/ZMODEM-8K block extensions.
- 7-bit ASCII encoded data subpackets.

### ZMODEM Control Bytes

| Name | Hex | Meaning |
| --- | --- | --- |
| `ZPAD` | `0x2A` (`*`) | Begins a ZMODEM frame. |
| `ZDLE` | `0x18` | ZMODEM escape byte. |
| `ZBIN` | `0x41` (`A`) | Binary header with CRC-16. |
| `ZHEX` | `0x42` (`B`) | Hex header with CRC-16. |
| `ZBIN32` | `0x43` (`C`) | Binary header with CRC-32. |
| `ZBINR32` | `0x44` (`D`) | RLE binary header with CRC-32; parse as unsupported. |
| `XON` | `0x11` | Flow-control byte ignored when received in ZMODEM data parsing. |
| `XOFF` | `0x13` | Flow-control byte ignored when received in ZMODEM data parsing. |
| `CAN` | `0x18` | Same byte as `ZDLE`; repeated CAN sequence can cancel. |
| `BS` | `0x08` | Used in cancel cleanup sequence. |

`ZDLE` and `CAN` share byte value `0x18`. Meaning depends on parser context.

### ZMODEM Frame Types

| Name | Value | Required | Direction | Meaning |
| --- | ---: | --- | --- | --- |
| `ZRQINIT` | `0` | yes | sender -> receiver | Request receiver init. |
| `ZRINIT` | `1` | yes | receiver -> sender | Receiver capabilities. |
| `ZSINIT` | `2` | parse/respond | sender -> receiver | Optional sender init/attention string. |
| `ZACK` | `3` | yes | either | Acknowledge `ZSINIT` or `ZCRCW`. |
| `ZFILE` | `4` | yes | sender -> receiver | File metadata follows. |
| `ZSKIP` | `5` | yes | receiver -> sender | Skip current file. |
| `ZNAK` | `6` | yes | receiver -> sender | Header was garbled. |
| `ZABORT` | `7` | yes | receiver -> sender | Abort batch. |
| `ZFIN` | `8` | yes | both | Finish session. |
| `ZRPOS` | `9` | yes | receiver -> sender | Start/resume at offset. |
| `ZDATA` | `10` | yes | sender -> receiver | Data subpackets follow. |
| `ZEOF` | `11` | yes | sender -> receiver | End of current file at offset. |
| `ZFERR` | `12` | yes | either | File read/write error. |
| `ZCRC` | `13` | parse/respond | receiver -> sender | File CRC request and response. Direction-specific behavior is defined below. |
| `ZCHALLENGE` | `14` | parse/respond | receiver -> sender | Echo challenge in `ZACK`. |
| `ZCOMPL` | `15` | parse/respond | sender -> receiver | Request complete. |
| `ZCAN` | `16` | internal | either | Pseudo-frame for repeated CAN. |
| `ZFREECNT` | `17` | parse/respond | sender -> receiver | Free space request. Respond with `ZACK` and encoded free bytes. |
| `ZCOMMAND` | `18` | reject | sender -> receiver | Remote command. Respond `ZFERR`; must not execute. |
| `ZSTDERR` | `19` | discard/debug-log | sender -> receiver | Stderr data follows. Log at debug level only. |

Unknown frame types must not panic. They should produce `ZNAK` when recovery is
possible or abort as `protocol_error`.

Required handling for less-common known frames:

- `ZCRC` while OxideBBS is sending a file:
  - Compute CRC-32/ISO-HDLC over the source file's unframed payload bytes.
  - Reply with `ZACK` whose `ZP0..ZP3` header bytes encode the CRC-32 as a
    little-endian `u32`.
  - If the CRC path is temporarily unavailable for a non-I/O reason, reply
    `ZNAK`.
  - If the CRC cannot be computed because the source file cannot be read, reply
    `ZFERR` and record an I/O failure.
- `ZCRC` while OxideBBS is receiving a file:
  - OxideBBS does not request file CRCs in v1.2.
  - If a caller sends an unexpected `ZCRC`, reply `ZNAK` once if recovery is
    possible, otherwise abort the current file with `ZFERR`.
- `ZCHALLENGE`:
  - Echo the received challenge value in a `ZACK` header.
- `ZCOMPL`:
  - Accept only when no file is currently active, either after a completed file
    or as an empty-batch completion.
  - Reply with `ZFIN` and proceed to the normal `ZFIN`/`OO` finish handshake.
  - Do not reply with another `ZCOMPL`.
  - If received while a file is active or before the current `ZEOF` is valid,
    reply `ZFERR` and abort the current file.
- `ZFREECNT`:
  - Reply with `ZACK`.
  - Encode available upload bytes in `ZACK` header `ZP0..ZP3` as little-endian
    `u32`.
  - The value should be the minimum of filesystem free bytes, per-area remaining
    upload quota, and global remaining upload quota.
  - If free space cannot be determined cheaply, encode `0`. A zero value means
    "unknown" for this response, not necessarily "disk full".
  - Cap values larger than `u32::MAX` to `u32::MAX`.
- `ZCOMMAND`:
  - Never execute the command and never pass command text to a shell.
  - Reply `ZFERR`, discard any following command data subpacket if one is
    already buffered, and end the session through `ZFIN` or local cancel.
- `ZSTDERR`:
  - Read and discard the following data subpacket when present.
  - Log a truncated debug-level diagnostic for sysops.
  - Do not show `ZSTDERR` text to the caller and do not log it at info level.

### ZMODEM Header Bytes

Every ZMODEM header has:

```text
frame_type header[4] fcs
```

For positions, `header[0..4]` is little-endian:

```text
ZP0 = low byte
ZP1
ZP2
ZP3 = high byte
```

For flags, the same four bytes are named:

```text
header[0] = ZF3
header[1] = ZF2
header[2] = ZF1
header[3] = ZF0
```

The implementation must provide typed helpers so agents do not manually index
the header array at call sites:

```rust
fn header_from_pos(pos: u32) -> [u8; 4];
fn pos_from_header(header: [u8; 4]) -> u32;
fn header_from_flags(zf0: u8, zf1: u8, zf2: u8, zf3: u8) -> [u8; 4];
fn flags_from_header(header: [u8; 4]) -> ZmodemFlags;
```

### ZMODEM Header Encodings

Hex header:

```text
ZPAD ZPAD ZDLE ZHEX hex(type) hex(header[0]) hex(header[1])
hex(header[2]) hex(header[3]) hex(crc_hi) hex(crc_lo)
CR LF-with-high-bit-set [XON]
```

Rules:

- Hex digits must be lower-case on send.
- Receiver must accept lower-case and upper-case hex.
- Receiver must ignore the high bit on hex header bytes.
- Append `CR` followed by LF-with-high-bit-set (`0x0D 0x8A`) after sent hex
  headers.
- Append `XON` after sent hex headers except `ZFIN` and `ZACK`.

Binary CRC-16 header:

```text
ZPAD ZDLE ZBIN type header[0] header[1] header[2] header[3] crc_hi crc_lo
```

Binary CRC-32 header:

```text
ZPAD ZDLE ZBIN32 type header[0] header[1] header[2] header[3]
crc0 crc1 crc2 crc3
```

Rules:

- `type`, `header`, and CRC bytes in binary headers must pass through ZMODEM
  link escaping.
- CRC-16 bytes are sent high byte then low byte.
- CRC-32 bytes are sent least-significant byte first after final XOR.
- Use hex headers for startup/recovery control frames where interop expects
  them: `ZRQINIT`, `ZRINIT`, `ZRPOS`, `ZNAK`, and initial `ZFIN`.
- Use binary headers for `ZFILE`, `ZDATA`, and `ZEOF`.
- Use `ZBIN32` for binary headers and data subpackets when peer advertises
  `CANFC32`; otherwise use `ZBIN`.

### ZMODEM Capability Flags

`ZRINIT` `ZF0` flags:

| Flag | Value | OxideBBS Behavior |
| --- | ---: | --- |
| `CANFDX` | `0x01` | Advertise yes. OxideBBS can read reverse-channel frames between subpackets. |
| `CANOVIO` | `0x02` | Advertise yes only when file writes are async/nonblocking enough not to stall reads. |
| `CANBRK` | `0x04` | Do not advertise. |
| `CANRLE` | `0x08` | Do not advertise in v1.2. |
| `CANLZW` | `0x10` | Do not advertise. |
| `CANFC32` | `0x20` | Advertise yes. Required for primary path. |
| `ESCCTL` | `0x40` | Advertise yes for telnet and serial by default. |
| `ESC8` | `0x80` | Do not advertise unless implemented and tested. |

`ZFILE` flags:

- `ZF0 = ZCBIN (0x01)` for all OxideBBS file payloads.
- `ZF1 = 0` unless a future policy implements management options.
- `ZF2 = 0`; compression/encryption/RLE are unsupported.
- `ZF3 = 0`; sparse files are unsupported.

If a peer sends unsupported compression, encryption, RLE, sparse, or command
options, skip the file with `ZSKIP` or abort with `ZFERR` if skipping cannot
continue safely.

### ZMODEM Link Escaping

The sender must escape bytes that can interfere with terminal links.

Always escape:

- `ZDLE` (`0x18`) as `ZDLE ZDLEE`, where `ZDLEE = ZDLE ^ 0x40 = 0x58`.
- `XON` (`0x11`) and `XON|0x80` (`0x91`).
- `XOFF` (`0x13`) and `XOFF|0x80` (`0x93`).
- `DLE` (`0x10`) and `DLE|0x80` (`0x90`).
- `CR` (`0x0D`) and `CR|0x80` (`0x8D`) when preceded by `@` (`0x40`) or
  `@|0x80` (`0xC0`).

When `ESCCTL` is active, also escape bytes where `(byte & 0x60) == 0`.

Escape encoding:

```text
escaped byte -> ZDLE (byte ^ 0x40)
```

Receive behavior:

- Ignore unescaped `XON`, `XON|0x80`, `XOFF`, and `XOFF|0x80`.
- On `ZDLE next`, recover `next ^ 0x40` for escaped control bytes.
- On `ZDLE ZCRCE/ZCRCG/ZCRCQ/ZCRCW`, end the current data subpacket.
- Treat five or more consecutive `CAN` bytes as cancel.
- Invalid escape sequences produce `ZNAK` or `ZRPOS` depending on state.

Telnet IAC escaping still happens outside this ZMODEM escaping layer.

### ZMODEM Data Subpackets

Maximum v1.2 data subpacket payload: 1024 bytes.

Data subpacket format:

```text
escaped_payload ZDLE frame_end fcs
```

Frame-end values:

| Name | Byte | Meaning |
| --- | --- | --- |
| `ZCRCE` | `0x68` (`h`) | End of frame; no response expected unless error. |
| `ZCRCG` | `0x69` (`i`) | Frame continues; no response expected unless error. |
| `ZCRCQ` | `0x6A` (`j`) | Frame continues; `ZACK` expected. |
| `ZCRCW` | `0x6B` (`k`) | End of frame; `ZACK` expected before next frame. |

Send policy:

- For `ZFILE` metadata, terminate the metadata subpacket with `ZCRCW`.
- For file data, use `ZCRCG` for middle subpackets when peer supports streaming.
- Use `ZCRCW` at least every `zmodem_ack_interval_bytes`, default `32768`, to
  give slow receivers a recovery point.
- Use `ZCRCE` for the final data subpacket before `ZEOF`.
- If peer advertises a nonzero receive buffer length in `ZRINIT`, do not send
  more than that many data bytes without `ZCRCW` and `ZACK`.

Receive policy:

- Verify subpacket FCS before writing bytes to the committed destination.
- It is acceptable to buffer one subpacket in memory before writing.
- If CRC fails, discard the subpacket bytes and send `ZRPOS` with the last
  committed offset.
- If offset in `ZDATA` does not match committed offset, send `ZRPOS` with the
  committed offset.

### ZMODEM CRC Algorithms

CRC-16:

- Same algorithm as XMODEM-CRC: polynomial `0x1021`, init `0x0000`, no
  reflection, final XOR `0x0000`.
- Used for hex headers, CRC-16 binary headers, and CRC-16 data subpackets.
- Header CRC is computed over frame type plus four header bytes, then finalized
  by processing two zero bytes.
- Data CRC is computed over payload bytes plus frame-end byte, then finalized by
  processing two zero bytes.

CRC-32:

- Use CRC-32/ISO-HDLC, also known as CRC-32/ADCCP.
- Reflected polynomial: `0xEDB88320`.
- Initial value: `0xFFFFFFFF`.
- Input reflected: yes.
- Output reflected: yes.
- Final XOR: `0xFFFFFFFF`.
- Check value: CRC of ASCII `123456789` is `0xCBF43926`.
- ZMODEM sends CRC-32 least-significant byte first after final XOR.

Required receive-side CRC-32 verification is compare-based: read the four FCS
bytes, reconstruct the transmitted little-endian CRC-32 value, compute CRC-32
over the frame or subpacket payload, and compare the two values.

An implementation may use the standard ZMODEM residual check internally only if
tests prove it accepts the same valid frames and rejects the same corrupt frames
as the compare-based method. The lrzsz implementation checks for residual
`0xDEBB20E3` after consuming the appended FCS bytes; OxideBBS tests must not
depend on copying lrzsz source.

### ZMODEM Download Sequence

Download means OxideBBS sends one or more files to the caller.

Required sequence:

```text
server -> caller: ZRQINIT
caller -> server: ZRINIT
[optional]
server -> caller: ZSINIT
caller -> server: ZACK
[/optional]
repeat for each file:
  server -> caller: ZFILE + metadata subpacket
  caller -> server: ZRPOS offset OR ZSKIP
  server -> caller: ZDATA offset + data subpackets
  server -> caller: ZEOF final_size
  caller -> server: ZRINIT OR ZRPOS offset OR ZFERR
server -> caller: ZFIN
caller -> server: ZFIN
server -> caller: "OO"
```

Server behavior:

- Send `ZRQINIT` as a hex header.
- Repeat `ZRQINIT` every `10s` until `ZRINIT` or startup retry exhaustion.
- Parse `ZRINIT` flags and choose CRC-32 if `CANFC32` is set.
- Send `ZFILE` for each selected file.
- Metadata pathname must be a sanitized display name, not an absolute host path.
- Include decimal file size and octal modification time in metadata.
- Accept `ZSKIP` as a skipped file, record it, and continue with the next file.
- Accept `ZRPOS offset` if `offset <= file_size`; seek to offset and send from
  there.
- Reject `ZRPOS offset` beyond EOF with `ZFERR`.
- Send `ZEOF` with final file size.
- Wait for `ZRINIT` before sending the next file.
- Finish with `ZFIN`, wait for peer `ZFIN`, then send ASCII `OO`.

### ZMODEM Upload Sequence

Upload means OxideBBS receives one or more files from the caller.

Required sequence:

```text
server -> caller: ZRINIT
caller -> server: ZFILE + metadata subpacket
server -> caller: ZRPOS offset OR ZSKIP OR ZFERR
caller -> server: ZDATA offset + data subpackets
caller -> server: ZEOF final_size
server -> caller: ZRINIT OR ZRPOS offset OR ZFERR
...
caller -> server: ZFIN
server -> caller: ZFIN
caller -> server: "OO"
```

Server behavior:

- Display text instructions before entering binary mode, then stop text output.
- Send `ZRINIT` as a hex header every `10s` while waiting for sender startup.
- If the caller sends `ZRQINIT` while OxideBBS is waiting to receive `ZFILE`,
  treat it as caller startup and respond with `ZRINIT` instead of rejecting it.
  Some clients begin uploads with `ZRQINIT` even though OxideBBS is already in
  receiver mode.
- Advertise `CANFC32` and `ESCCTL`.
- On `ZFILE`, read and validate metadata before accepting data.
- If filename, size, area policy, or quota fails, send `ZSKIP` or `ZFERR` and
  record the failure.
- If a partial temp file exists for the same upload and policy allows resume,
  send `ZRPOS` at the partial length. Otherwise send `ZRPOS 0`.
- On `ZDATA`, verify the offset equals the committed temp-file length.
- On CRC failure or wrong offset, send `ZRPOS` with the last committed offset.
- On `ZEOF`, verify the final offset equals bytes committed.
- Commit the file entry only after `ZEOF` validation and successful temp-file
  move.
- May send `ZRINIT` after each committed file so the caller can send another
  file or finish. Do not require the peer to wait for that response; if a valid
  next `ZFILE`, `ZFIN`, or `ZCOMPL` arrives before or instead of another
  `ZRINIT` exchange, handle it according to the current state.
- On `ZFIN`, send `ZFIN`, read optional `OO`, and complete the session.

### ZMODEM Metadata

`ZFILE` metadata subpacket:

```text
pathname NUL size SP mtime_octal [SP mode_octal [SP serial ...]] NUL
```

Required parse behavior:

- `pathname` ends at first NUL.
- The remaining metadata fields are ASCII tokens separated by spaces.
- `size` is decimal bytes. If missing or invalid, reject upload with `ZFERR`.
- `mtime` is octal seconds since Unix epoch. If missing or invalid, store
  `None` and use current time for filesystem write.
- `mode` is octal. Store for diagnostics only; do not apply remote permissions.
- Ignore extra fields after mode.
- Reject metadata subpackets larger than `zmodem_max_metadata_bytes`, default
  4096.

Outbound metadata:

- Send sanitized display filename only.
- Send decimal size.
- Send octal Unix modification timestamp if known; otherwise send `0`.
- Send mode `100644` for regular files.
- Always use `ZCBIN`.

Inbound metadata:

- Sanitize pathname using this document's path rules.
- Use declared size for quota checks before accepting data.
- Store original metadata name separately from sanitized storage name.
- Do not apply remote mode bits.
- Do not create directories from remote path components.

### ZMODEM Resume

Resume is required for ZMODEM.

When OxideBBS sends:

- Honor peer `ZRPOS offset` when `offset <= file_size`.
- Seek to `offset` and send `ZDATA` with the same offset.
- If peer repeatedly requests the same offset after valid data, abort after
  `zmodem_position_retries`, default 10.

When OxideBBS receives:

- If a partial temp file exists and belongs to the same transfer/user/area/name,
  send `ZRPOS partial_size`.
- If no safe partial exists, send `ZRPOS 0`.
- If peer sends `ZDATA` at a different offset, send `ZRPOS committed_offset`.
- Never append data at a mismatched offset.
- Do not resume XMODEM.

Partial-file matching must use:

- Same user.
- Same file area.
- Same sanitized storage name.
- Same declared size.
- Same transfer protocol.
- Partial file still under the configured incoming directory.

### ZMODEM Timeouts And Retries

Defaults:

| Setting | Default | Applies To |
| --- | ---: | --- |
| `zmodem_startup_interval` | `10s` | Repeat `ZRQINIT` or `ZRINIT`. |
| `zmodem_header_timeout` | `10s` | Wait for next frame header. |
| `zmodem_data_timeout` | `10s` | Wait for next data byte/subpacket. |
| `zmodem_ack_timeout` | `10s` | Wait for `ZACK`, `ZRINIT`, `ZRPOS`, or `ZFIN`. |
| `zmodem_startup_retries` | `10` | Startup frames. |
| `zmodem_frame_retries` | `10` | Garbled frame/header retries. |
| `zmodem_position_retries` | `10` | Repeated `ZRPOS` for same offset. |
| `zmodem_max_subpacket_bytes` | `1024` | Payload bytes per subpacket. |
| `zmodem_ack_interval_bytes` | `32768` | Force `ZCRCW`/`ZACK` recovery point. |
| `zmodem_max_metadata_bytes` | `4096` | `ZFILE` metadata payload. |

Timeout policy:

- Startup timeout sends the startup frame again until retry exhaustion.
- Header CRC failure sends `ZNAK` when not currently receiving file data.
- Data CRC failure sends `ZRPOS` with last committed offset.
- Repeated timeouts or repeated same-position retries abort the file.
- A failed file in a batch should skip or abort according to direction:
  - Download: record failed file and continue only if caller sends `ZSKIP`;
    otherwise abort session.
  - Upload: reject current file, delete temp, send `ZRINIT` for next file unless
    the peer sent `ZABORT` or cancel.

### ZMODEM Cancel Behavior

Sending cancel:

```text
ZPAD ZPAD CAN CAN CAN CAN CAN CAN CAN CAN BS BS BS BS BS BS BS BS BS BS
```

In bytes:

```text
0x2A 0x2A 0x18 0x18 0x18 0x18 0x18 0x18 0x18 0x18
0x08 0x08 0x08 0x08 0x08 0x08 0x08 0x08 0x08 0x08
```

Receiving cancel:

- Detect five or more consecutive `CAN` bytes outside normal escaped data as
  cancel.
- Detect `ZABORT` as caller-requested abort.
- Detect `ZFERR` as current-file failure.
- After local user/sysop cancel, send the cancel sequence and then restore the
  terminal.

Cancel cleanup:

- Delete upload temp file unless partial retention is enabled.
- Mark transfer history with `canceled_by_caller` or `canceled_by_server`.
- Do not create or approve a `file_entries` row.

## Error Mapping

Protocol errors should map to stable error codes:

| Code | Meaning |
| --- | --- |
| `transfer.transport.binary_refused` | Telnet binary mode was refused or timed out. |
| `transfer.transport.closed` | Caller disconnected. |
| `transfer.protocol.timeout` | Protocol timed out. |
| `transfer.protocol.crc` | CRC validation failed too many times. |
| `transfer.protocol.cancel_remote` | Remote sent cancel/abort. |
| `transfer.protocol.cancel_local` | Server/sysop canceled. |
| `transfer.protocol.unsupported_option` | Peer required unsupported protocol option. |
| `transfer.path.invalid_name` | Inbound filename failed sanitization. |
| `transfer.quota.exceeded` | Upload exceeded size or disk policy. |
| `transfer.security.denied` | Caller did not meet area security policy. |
| `transfer.io.read_failed` | Server failed to read source file. |
| `transfer.io.write_failed` | Server failed to write upload temp file. |
| `transfer.io.commit_failed` | Server failed to commit upload. |

Caller-facing messages must be short and non-sensitive. Sysop logs can include
more detail, but must not leak credentials.

## End-To-End Test Fixtures

Tests must cover protocol behavior without relying on live telnet sockets first.
Then they must cover telnet and serial integration.

### Deterministic Payload Fixtures

Create fixture files under a transfer test fixture helper, not checked-in large
binary blobs unless they are small.

Required payloads:

- `empty.bin`: zero bytes.
- `one-byte.bin`: one byte.
- `127.bin`: 127 bytes.
- `128.bin`: 128 bytes.
- `129.bin`: 129 bytes.
- `1024.bin`: 1024 bytes.
- `1025.bin`: 1025 bytes.
- `all-bytes.bin`: byte values `0x00..0xFF`, repeated at least twice.
- `iac-heavy.bin`: includes many `0xFF` bytes for telnet IAC escaping tests.
- `control-heavy.bin`: includes `0x00..0x1F`, `0x7F`, XON, XOFF, CR, LF, and
  ZDLE/CAN bytes.
- `ends-with-cpmeof.bin`: ends with real `0x1A` bytes to prove XMODEM upload
  does not strip binary data by guessing.
- `seeded-64k.bin`: deterministic pseudo-random bytes from a fixed seed.

### Unit Tests

Required unit tests:

- Existing `Transport` to `ByteTransport` adapter maps byte, timeout, closed,
  and transport error outcomes correctly.
- CRC-16/XMODEM check value `123456789 -> 0x31C3`.
- CRC-32/ISO-HDLC check value `123456789 -> 0xCBF43926`.
- XMODEM block encode/decode.
- XMODEM duplicate previous block handling.
- XMODEM invalid complement rejection.
- XMODEM final padding behavior.
- ZMODEM hex header encode/decode.
- ZMODEM binary CRC-16 header encode/decode.
- ZMODEM binary CRC-32 header encode/decode.
- ZMODEM link escape/unescape for ZDLE, XON, XOFF, DLE, CR-after-`@`, and
  ESCCTL.
- ZMODEM data subpacket CRC failure.
- ZMODEM `ZFILE` metadata parse.
- ZMODEM `ZCRC` request while sending replies `ZACK` with CRC-32 encoded in
  `ZP0..ZP3`.
- ZMODEM `ZFREECNT` replies `ZACK` with little-endian free bytes in `ZP0..ZP3`
  and uses zero for unknown free space.
- ZMODEM `ZCOMMAND` replies `ZFERR` and never exposes command text to a shell.
- ZMODEM `ZSTDERR` is discarded and logged only at debug level.
- ZMODEM `ZCOMPL` outside an active file transitions to finish via `ZFIN`.
- Path sanitizer for absolute paths, `..`, drive prefixes, control bytes,
  reserved names, long names, collisions, and invalid encodings.
- Telnet IAC escaping maps outgoing `0xFF` to `0xFF 0xFF` and incoming
  `0xFF 0xFF` to one payload byte.

### Protocol Loopback Tests

Use in-memory duplex transports with fault injection.

XMODEM-CRC loopback tests:

- Upload preflight prompts for a filename before entering binary mode.
- Server download of every deterministic payload.
- Server upload of every deterministic payload with declared size.
- Upload without declared size stores padded length and marks size source.
- Corrupt one data byte and verify `NAK` plus resend.
- Drop one `ACK` and verify duplicate block is accepted without duplicate write.
- Delay a byte past timeout and verify retry.
- Repeated startup timeout aborts.
- Remote cancel aborts.
- Local cancel sends repeated `CAN`.
- Payload containing `0xFF` succeeds over telnet wrapper.

ZMODEM loopback tests:

- Server download of every deterministic payload.
- Server upload of every deterministic payload.
- Upload startup accepts caller `ZRQINIT` and replies with `ZRINIT`.
- Batch download with at least three files.
- Batch upload with at least three files.
- `ZFILE` metadata with unsafe path is rejected and does not write outside area.
- `ZRPOS` resume from offset 0.
- `ZRPOS` resume from a nonzero offset.
- Receiver sends `ZRPOS` after injected CRC failure and sender resends from
  correct offset.
- `ZSKIP` skips one file and continues batch.
- `ZABORT` aborts batch.
- `ZCOMPL` after completed upload enters the finish handshake.
- `ZFIN` handshake completes with `OO`.
- CRC-16 fallback works when peer does not advertise `CANFC32`.
- CRC-32 path works when peer advertises `CANFC32`.
- Payload containing all byte values succeeds over telnet wrapper.

### Telnet Integration Tests

Use the existing telnet transport test style.

Required telnet tests:

- Transfer preflight negotiates `TRANSMIT-BINARY` both directions.
- Refused binary negotiation fails before protocol bytes.
- Outgoing protocol byte `0xFF` is doubled as telnet `IAC IAC`.
- Incoming telnet command during transfer is handled by telnet layer and not
  delivered to protocol parser.
- No CR/LF normalization occurs during transfer.
- Normal menu rendering resumes after success.
- Normal menu rendering resumes after cancel.
- Caller disconnect during transfer records failure and cleans temp file.

### Serial Integration Tests

Use pseudo-terminal or loopback transports on Unix where available.

Required serial tests:

- XMODEM-CRC download over serial loopback.
- XMODEM-CRC upload over serial loopback.
- ZMODEM download over serial loopback.
- ZMODEM upload over serial loopback.
- Serial transfer does not apply telnet IAC escaping.

### External Interop Tests

Runtime code must not shell out to external transfer tools. Tests may use
external tools when explicitly enabled.

Automated optional interop:

- `lrzsz` `rz`/`sz` if installed.
- Enabled only when `OXIDEBBS_INTEROP_LRZSZ=1`.
- Tests must be skipped, not failed, when tools are absent.
- Run through a pseudo-terminal or pipe harness that accurately models a caller
  terminal.

Manual interop targets for release smoke:

- SyncTERM current stable.
- MuffinTerm current stable.
- Tera Term current stable on Windows.
- Qodem or Minicom with `lrzsz`.

Manual smoke scripts should cover:

- ZMODEM download.
- ZMODEM upload.
- ZMODEM batch download.
- ZMODEM resume after intentional disconnect if the client exposes resume.
- XMODEM-CRC single-file download.
- XMODEM-CRC single-file upload with declared size.

## Implementation Order

Agents should implement in this order:

1. Add `oxidebbs-transfer` crate with no server integration.
2. Implement CRC helpers and byte-transport test harness.
3. Implement path sanitizer and metadata structs.
4. Implement XMODEM-CRC send/receive and loopback tests.
5. Implement ZMODEM constants, escaping, headers, and CRC tests.
6. Implement ZMODEM receive state machine.
7. Implement ZMODEM send state machine.
8. Add fault-injection loopback tests.
9. Add server preflight state and DecentDB transfer history.
10. Add file-area storage and temp-file commit behavior.
11. Add telnet `BinaryTransferGuard`.
12. Add caller menu actions.
13. Add serial loopback coverage after serial transport exists.
14. Add optional external interop tests.
15. Update caller docs and config examples.

Do not start with caller menus. The protocol engines and fault-injection tests
must exist first.

## Documentation Required With Implementation

When P4 is implemented, update:

- `design/SPEC.md`
- `docs/project/file-transfers.md`
- `docs/project/caller-commands.md`
- `config/oxidebbs.example.toml`
- `README.md`
- `design/TASKS.md`
- `design/RELEASE_v1_2_PLAN.md` phase status

The caller docs must state:

- ZMODEM is recommended.
- XMODEM-CRC is fallback and single-file only.
- XMODEM uploads do not carry filename or file size metadata.
- YMODEM is not available in v1.2.
- Telnet clients must support binary transfers.

## References

Primary and compatibility references used for this design:

- Chuck Forsberg, `XMODEM/YMODEM Protocol Reference`:
  https://techheap.packetizer.com/communications/modems/xmodem-ymodem_reference.html
- Chuck Forsberg, `ZMODEM Protocol Description`:
  https://www.tuhs.org/Usenet/comp.sources.unix/1986-May/004372.html
- lrzsz `zmodem.h` constants and `zm.c` behavior, used as interop reference
  only. Do not copy GPL source into OxideBBS:
  https://raw.githubusercontent.com/UweOhse/lrzsz/master/src/zmodem.h
  https://raw.githubusercontent.com/UweOhse/lrzsz/master/src/zm.c
- RFC 854, Telnet Protocol Specification:
  https://datatracker.ietf.org/doc/html/rfc854
- RFC 856, Telnet Binary Transmission:
  https://www.rfc-editor.org/rfc/rfc856
- Tera Term ZMODEM implementation notes and telnet tips:
  https://github.com/TeraTermProject/teraterm/wiki/ZMODEM-Protocol
  https://teratermproject.github.io/manual/5/en/usage/tips/zmodem.html
- SyncTERM manual, transfer protocol support:
  https://syncterm.bbsdev.net/Manual.html
- MuffinTerm file transfer manual:
  https://muffinterm.app/manual/transfers
