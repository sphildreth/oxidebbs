# ADR 0023: FTN Duplicate Detection Policy

## Status

Accepted

## Context

FTN networks can resend packets, retry polls, or route the same message through
multiple paths. Duplicate detection is required before importing network
messages into local areas.

## Decision

Duplicate detection is MSGID-primary with a hash fallback.

For echomail with MSGID:

```text
sha256(network_key + "\0" + area_tag + "\0" + msgid)
```

For netmail with MSGID:

```text
sha256(network_key + "\0" + from_address + "\0" + to_address + "\0" + msgid)
```

When MSGID is absent, OxideBBS uses a fallback hash over network, area or
recipient tuple, origin, timestamp, subject, and normalized body hash. The
fallback query must tolerate a plus or minus five minute clock skew window.

Duplicate decisions are stored in DecentDB and duplicate rejections are logged.

## Consequences

- MSGID is authoritative when present.
- Messages with the same MSGID in different echo areas are distinct.
- MSGID-less messages still get practical duplicate protection.
- Duplicate logs become part of network troubleshooting.
