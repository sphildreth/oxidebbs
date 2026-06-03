# ADR 0021: FTN Packet Format Policy

## Status

Accepted

## Context

The FTN plan requires real packet support for legacy FTN interop while keeping
OxideNet free to use its own internal packet profile.

Legacy FTN packets vary across Type-2, Type-2+, and related extensions. OxideBBS
must be tolerant when reading but predictable when writing.

## Decision

The legacy FTN adapter reads Type-2 and Type-2+ packets and writes Type-2+
packets by default.

Packet parsing is byte-oriented. Raw packet field bytes remain authoritative.
String accessors may be provided for display, logs, and tests, but the parser
must not require valid UTF-8 for message text.

Outbound packet writing must include FSC-0048 4D addressing fields and the
Type-2+ capability word. Legacy Type-2 packets without reliable zone fields are
resolved through explicit link/network context.

## Consequences

- OxideBBS can participate in real FTN networks while producing one stable
  outbound packet shape.
- Malformed packets are handled by the tosser quarantine policy, not by panics.
- Product-code selection must be documented before shipping outbound packets.
