# ADR 0024: FTN Toss Quarantine Policy

## Status

Accepted

## Context

Inbound packet processing must handle malformed packets, bad passwords, unknown
areas, duplicate messages, and partial failures without crashing the BBS.

## Decision

The tosser uses quarantine for packet-level trust or format failures and
message-level skip records for individual message failures.

Packet-level quarantine applies to:

- malformed packet headers
- truncated packets
- wrong packet password
- unknown or disallowed originating link
- unsupported bundle or compression format

Message-level skip applies to:

- duplicate messages
- unknown AREA tags when auto-create is disabled
- malformed non-critical kludges
- messages that fail local moderation or security policy

The tosser is not all-or-nothing per packet. Successfully imported messages
remain imported even if a later message in the same packet fails. Each failure
must be recorded with enough context to prevent silent retry loops.

## Consequences

- Operators can inspect quarantined packets and decide what to do.
- Bad traffic does not stop later packets from being processed.
- Tests must cover both packet quarantine and message skip behavior.
