# Echomail

OxideBBS parses and composes common FTN echomail control lines through
`oxidebbs-ftn`.

Supported control lines include:

- `AREA`
- `MSGID`
- `REPLY`
- `INTL`
- `FMPT`
- `TOPT`
- `FLAGS`
- `SEEN-BY`
- `PATH`
- `Via`
- tear lines
- origin lines

The parser is tolerant: unknown `KEY: value` control lines are preserved as
unknown kludges instead of causing an import failure. The composer is stricter
and emits canonical control-line prefixes for known variants.

The runtime tosser and scanner build on these primitives. `net toss` stores
imported echomail metadata, SEEN-BY/PATH rows, duplicate-log decisions, packet
state, and local message rows in DecentDB. `net scan` exports subscribed local
messages into outbound packets, avoids re-exporting the same local message to
the same link, and bundles ready packet files for ZIP-compression links.

Duplicate keys follow ADR 0023: MSGID is hashed with SHA-256 as the primary key
within the network and echo area, while MSGID-less messages use a fallback hash
over network, area, origin, timestamp bucket, subject, and normalized body hash
with a five-minute clock-skew candidate window.
