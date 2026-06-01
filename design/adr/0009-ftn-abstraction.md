# ADR 0009: Keep FTN Packet I/O Behind a Network Boundary

## Status

Accepted

## Context

OxideBBS is planned to support FTN-style networking after the local BBS
experience is stable. FTN support involves addresses, echomail mappings,
netmail, duplicate detection, and packet import/export. These concerns should
not leak into local message commands or telnet session code.

## Decision

OxideBBS will model FTN/OxideNet concerns in core domain types first:

- FTN addresses use `zone:net/node.point` semantics.
- Echomail mappings connect a local message area to a network and echo tag.
- Netmail is a directed message between FTN addresses.
- Duplicate detection uses a stable tuple of network, area tag, origin address,
  and message id.
- Packet import/export is represented as a boundary object that names the peer,
  direction, network, and spool path.

Actual packet parsing, bundling, compression, and transport are future
infrastructure concerns behind this boundary.

## Consequences

- Local message commands can remain local-first.
- FTN packet handling can be tested independently.
- Duplicate detection can be designed before packet import writes into
  DecentDB.
- Future OxideNet-specific behavior can reuse the same boundary rather than
  creating a separate message pipeline.
