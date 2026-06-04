# BinkP

BinkP is the TCP/IP mailer protocol used by OxideBBS for FTN packet and bundle
exchange. Caller file-transfer protocols such as XMODEM-CRC and ZMODEM are not
used for network mail exchange.

The `oxidebbs-binkp` crate currently provides tested frame primitives:

- command and data frame encoding
- command and data frame decoding
- stream read/write helpers
- protocol errors for truncated or malformed frames
- client handshake sending with `M_ADR` and optional `M_PWD`
- server handshake acceptance with address/password validation and `M_OK` /
  `M_ERR` responses
- secret-safe refusal errors that do not echo configured passwords
- `M_FILE` offer parsing and writing
- bounded data-frame send/receive helpers
- `M_GOT` acknowledgement and `M_EOB` end-of-batch handling
- batch send/receive helpers for empty and multi-file stream exchanges
- acknowledged batch sending for synchronous send-then-receive sessions
- session-level filename validation that rejects path-like names
- transport-security preflight policy for `tls_required`,
  `tls_opportunistic`, and `plaintext_legacy`
- exponential retry policy calculation for future poll loops
- in-process one-active-session-per-link guard primitive

Implemented command constants include `M_NUL`, `M_ADR`, `M_PWD`, `M_FILE`,
`M_OK`, `M_EOB`, `M_GOT`, `M_ERR`, `M_BSY`, `M_GET`, and `M_SKIP`.

`net poll --dry-run` uses the transport-security policy helper to report
whether a link requires TLS, attempts TLS, allows plaintext, or needs an
operator warning. Non-dry-run `net poll` currently supports plaintext-legacy
client polling: it sends ready outbound files, receives the peer batch into the
inbound drop directory, marks acknowledged outbound packets processed, and
records `network_poll_log` rows.

Batch helpers remain stream-level primitives: an empty batch writes only
`M_EOB`, received files are acknowledged with `M_GOT`, and the server listener
connection lifetime remains outside the helper.

Retry policy support is calculation-only. It decides whether more attempts
remain and what delay should precede the next attempt; it does not sleep or run
poll loops.

The link session guard is also a primitive: it can prevent a second active
session for the same link once poll/listener loops use it.

TLS socket/session implementation, retry execution, one-session guard
integration, and inbound server/listener loops are tracked by the BinkP
transport phase in the v1.2 release plan.
