# BinkP

BinkP is the TCP/IP mailer protocol used by OxideBBS for FTN packet and bundle
exchange. Caller file-transfer protocols such as XMODEM and ZMODEM are not
used for network mail exchange.

The `oxidebbs-binkp` crate provides:

- command and data frame encoding/decoding with stream read/write helpers
- command constants for `M_NUL`, `M_ADR`, `M_PWD`, `M_FILE`, `M_OK`, `M_EOB`,
  `M_GOT`, `M_ERR`, `M_BSY`, `M_GET`, and `M_SKIP`
- client and server handshakes with address/password validation
- per-link password validation for inbound listener sessions
- TLS-required, TLS-opportunistic, and plaintext-legacy security planning
- TLS client and server socket helpers using TLS 1.2 or newer
- file offer, bounded data-frame send/receive, `M_GOT`, and `M_EOB` helpers
- batch send/receive helpers for empty polls, multi-file exchange, and large
  files
- retry policy and one-active-session-per-link guard primitives

`net poll <link>` opens a BinkP client session, sends pending outbound packet
files, receives the peer batch into the inbound drop directory, marks
acknowledged outbound packet rows processed, and writes `network_poll_log`
rows. `net poll --all` repeats that workflow for enabled links.

TLS-required links attempt TLS and fail the poll if certificate validation or
the TLS handshake fails. TLS-opportunistic links attempt TLS first and fall back
to plaintext only when the link is explicitly legacy-compatible. Plaintext
legacy links skip TLS and emit operator-visible warnings through the security
preflight and poll-log surfaces.

The inbound BinkP listener accepts plaintext or TLS sockets, authenticates the
remote address/password against enabled links, rejects plaintext sessions for
TLS-required links before `M_OK`, writes received files into the selected
network profile inbound drop directory, sends pending outbound files for the
authenticated link, and marks DB-backed outbound packets processed after
acknowledgement.

Only one active session per link is allowed inside each running poll/listener
process. A second concurrent session for the same link is refused by the
session guard.
