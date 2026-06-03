# ADR 0026: FTN Netmail Routing Policy

## Status

Accepted

## Context

Netmail can be delivered locally, sent directly to a linked node, routed through
a hub, held for pickup, or sent as crash mail.

## Decision

OxideBBS routes netmail in this order:

1. Local delivery when the destination is one of the local addresses.
2. Direct link delivery when the destination matches a configured link.
3. Crash direct delivery when the message has the Crash attribute and a direct
   route can be resolved.
4. Hold delivery when the message has the Hold attribute and the destination is
   a known configured link.
5. Hub-routed delivery through the configured hub for the destination zone or
   network.
6. Unknown destination with an operator-visible queue/error record.

Hub-routed netmail preserves the final destination in INTL, FMPT, and TOPT
kludges as needed.

## Consequences

- Routing decisions are deterministic and testable.
- Unknown routes do not disappear silently.
- Crash and hold behavior remain explicit instead of being accidental side
  effects of queue state.
