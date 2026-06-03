# ADR 0025: FTN Outbound MSGID Policy

## Status

Accepted

## Context

Outbound FTN messages need unique MSGID values. The FTN plan identified random
hex, content hash, and counter strategies.

## Decision

OxideBBS uses high-entropy random hexadecimal MSGID serials generated from OS
randomness.

The full MSGID is:

```text
<local-ftn-address> <random-128-bit-hex>
```

The random serial is stored with the local message's network export record so
retries reuse the same MSGID for the same outbound network representation.

Content hashes are rejected because they leak information about message content
and can collide across edited retries. Simple counters are rejected because they
require extra coordination across concurrent writers and reveal system activity
patterns.

## Consequences

- MSGID generation works across concurrent scanner runs when backed by the
  database export record.
- Retries remain stable.
- The implementation must use a reviewed randomness source already in the
  workspace or added through `cargo add` with justification.
