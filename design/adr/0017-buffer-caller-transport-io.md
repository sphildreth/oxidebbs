# ADR 0017: Buffer Caller Transport I/O Internally

## Status

Accepted

## Context

The current caller transport exposes byte reads and byte writes. That matches
telnet parsing and future serial support, but the TCP implementation reads one
byte at a time from the socket. A busy caller or ANSI-heavy door can therefore
drive excessive syscalls and small writes.

The BBS session logic should remain byte-oriented because telnet IAC parsing
and CP437/ANSI rendering are byte-oriented. The performance problem is in the
transport implementation and reply flushing strategy, not in the high-level
session model.

## Decision

Keep the public `Transport` trait byte-oriented for v1, but buffer TCP I/O
inside transport/session plumbing.

`TcpTransport` MUST maintain an internal read buffer. When the buffer is empty,
it MUST read up to 4096 bytes from the socket and then serve bytes from memory
to `read_byte`.

Telnet negotiation replies produced while parsing input SHOULD be accumulated
and flushed together at the next safe output boundary instead of forcing a
socket write for every negotiation byte. A safe output boundary is:

- before sending caller-visible text or screen bytes
- before blocking for the next caller input after replies have accumulated
- before hangup

The `Transport` trait MUST NOT be expanded for v1 unless the internal buffering
approach cannot satisfy tests. If a trait expansion becomes necessary, the
chosen method name is `read_into`, and it must be implemented for both
`TcpTransport` and `LoopbackTransport`.

No Nagle or TCP_NODELAY policy change is included in this ADR.

## Consequences

- Telnet parsing remains byte-oriented.
- TCP read syscall count drops substantially under normal caller traffic.
- Loopback tests remain deterministic.
- Future serial transport can implement the same trait without committing to a
  network-specific bulk API.

## Rejected Options

- Convert all session code to chunk-oriented parsing: unnecessary and more
  invasive than the performance issue requires.
- Add a new dependency for buffering: the standard library and Tokio are enough.
- Change TCP_NODELAY as the first optimization: it does not address one-byte
  read syscalls.
