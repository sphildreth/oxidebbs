# Netmail Routing

OxideBBS v1.2 includes a pure `oxidebbs-ftn` routing decision layer for
netmail. The tosser uses that layer to deliver local netmail, queue direct or
hub-routed outbound netmail, and quarantine unknown destinations. The scanner
materializes pending outbound netmail rows into Type-2+ packet files for BinkP
delivery.

The router implements ADR 0026 with these outcomes:

- `LocalDelivery`: destination is one of the board's local FTN addresses.
- `Direct`: destination matches a configured direct link.
- `Crash`: destination matches a configured direct link and the message has the
  FTN Crash attribute.
- `Hold`: destination matches a configured direct link and the message has the
  FTN Hold attribute.
- `RoutedViaHub`: destination does not match a direct link, but a configured hub
  scope matches the destination.
- `UnknownDestination`: no local, direct, or hub route matches.

Crash and hold are explicit direct-link decisions. If a message is addressed to
a configured link and has one of those attributes, the router returns `Crash` or
`Hold` instead of a generic `Direct` decision. Hub routing is used only after
local and direct-link checks fail.

Hub scopes are deterministic:

1. exact zone/net hub scopes win first
2. zone hub scopes win next
3. catch-all hub scopes win last

When two hub links have the same specificity, the first configured link wins.
That keeps routing stable and auditable from sysop configuration order.

Runtime boundaries:

- the router is a library decision layer, not a `net route` CLI command
- routing uses configured network links; DecentDB nodelist lookup is available
  to operators but is not required for routing decisions
- AreaFix replies and forwarded netmail are queued as pending outbound netmail
  and materialized by `net scan`
