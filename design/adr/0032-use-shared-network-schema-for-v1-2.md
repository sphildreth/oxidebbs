# ADR 0032: Use Shared Network Schema For v1.2

## Status

Accepted

## Context

The FTN implementation plan required a final table-naming decision before code
was written. Legacy FTN and OxideNet share concepts such as network profiles,
links, packets, duplicate logs, poll logs, area subscriptions, and nodelists.

Keeping all of those in `ftn_*` tables would make OxideNet depend on legacy FTN
names. Creating separate FTN and OxideNet tables would duplicate state and make
shared admin views harder.

## Decision

v1.2 uses shared `network_*` DecentDB tables for protocol-neutral network state.

Shared tables include:

- `network_profiles`
- `network_links`
- `network_areas`
- `network_packets`
- `network_messages`
- `network_seen_by`
- `network_path`
- `network_duplicate_log`
- `network_poll_log`
- `network_area_subscriptions`
- `network_nodelist`

Legacy FTN packet, kludge, bundle, and nodelist parser code remains in
`oxidebbs-ftn`. OxideNet application, policy, and credential tables also use
`network_*` names where they represent general network registry state.

## Consequences

- OxideNet can use the shared network foundation without depending on legacy
  `.pkt` details.
- Admin tooling can show network status across legacy FTN and OxideNet through
  one repository layer.
- Existing v1 foundation types must migrate from `oxidebbs-core` into
  `oxidebbs-network` and be temporarily re-exported by `oxidebbs-core`.
