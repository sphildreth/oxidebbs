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
- session-level filename validation that rejects path-like names

Implemented command constants include `M_NUL`, `M_ADR`, `M_PWD`, `M_FILE`,
`M_OK`, `M_EOB`, `M_GOT`, `M_ERR`, `M_BSY`, `M_GET`, and `M_SKIP`.

Full client/server connection loops, TLS policy, retries, concurrency guards,
poll logging, and filesystem spool integration are tracked by the BinkP
transport phase in the v1.2 release plan.
