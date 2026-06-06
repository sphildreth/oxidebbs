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

Full toss/scan behavior, SEEN-BY/PATH propagation, and duplicate-log writes are
implemented by later FTN workflow phases.

Duplicate keys follow ADR 0023: MSGID is hashed with SHA-256 as the primary key
within the network and echo area, while MSGID-less messages use a fallback hash
over network, area, origin, timestamp bucket, subject, and normalized body hash
with a five-minute clock-skew candidate window.
