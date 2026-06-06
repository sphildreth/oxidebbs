# ADR 0022: FTN Kludge Handling Policy

## Status

Accepted

## Context

Echomail and netmail metadata is embedded in message text through FTN kludge
lines, SEEN-BY, PATH, tear lines, and origin lines. Real-world FTN traffic is
not perfectly formatted.

## Decision

OxideBBS uses tolerant parsing and strict composition.

Inbound parsing must:

- accept CR, LF, and CRLF line endings
- recognize SOH-prefixed kludges and caret-rendered test fixtures
- preserve unknown kludges as raw bytes
- skip malformed non-critical kludges with a warning
- preserve raw body bytes for duplicate hashing and re-export decisions

Outbound composition must:

- emit CR line endings
- emit known kludges in canonical order
- sort SEEN-BY addresses
- preserve PATH routing order
- include tear and origin lines for echomail

## Consequences

- Real packets are accepted without letting malformed metadata crash the toss.
- OxideBBS-generated packets are predictable and testable.
- Unknown metadata can be inspected later rather than being lost.
