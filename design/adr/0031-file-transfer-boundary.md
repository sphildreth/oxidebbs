# ADR 0031: File Transfer Boundary

## Status

Accepted

## Context

Caller file transfers were deferred from the v1 release line and are now
implemented in v1.2. These protocols are byte-oriented and interact directly
with caller transport timing and terminal negotiation.

FTN/FidoNet network mail exchange is a separate transport concern. It uses
BinkP in v1.2 and must not depend on caller file-transfer protocols such as
XMODEM, YMODEM, or ZMODEM.

## Decision

v1.2 adds an `oxidebbs-transfer` crate that owns file-transfer protocol state
machines and exposes a transport-agnostic API.

The crate must support:

- ZMODEM send and receive as the primary caller file-area transfer protocol.
- XMODEM-CRC send and receive as the required fallback protocol.

YMODEM, XMODEM-1k, checksum-only XMODEM, ZedZap/ZMODEM-8K, and other transfer
variants are outside v1.2 scope unless a later ADR supersedes this one. OxideBBS
must not advertise unsupported protocols in caller menus, docs, config examples,
or release notes.

The server adds file areas, file entries, and transfer history records in
DecentDB. Caller file menus use configured security levels and must work over
telnet and serial transports.

The transfer crate operates on bytes and must not assume Unicode terminal UI.
Protocol state machines should be owned Rust implementations inside
`oxidebbs-transfer` and must follow `design/FILE_TRANSFERS.md`. Helper
dependencies for checksums, CRCs, or test fixtures are allowed only when
reviewed, compatible with OxideBBS byte-transport requirements, added with
`cargo add`, and justified in the implementation notes. The implementation must
not shell out to external `rz`, `sz`, or similar programs.

## Consequences

- File transfer behavior can be tested without a live telnet socket.
- Transport code stays separate from protocol code.
- BinkP remains the only v1.2 FTN/FidoNet network mail transport.
- ZMODEM behavior requires integration tests with realistic client fixtures.
- XMODEM-CRC fallback behavior requires loopback integration tests.
- Unsupported protocols must remain absent from caller-facing surfaces.
