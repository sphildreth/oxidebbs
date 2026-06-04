# OxideNet Addressing

OxideNet uses FTN-style addresses from the shared `oxidebbs-network`
`FtnAddress` model.

Current validated ranges:

| Range | Meaning |
| --- | --- |
| `42:1/1` | Primary hub and registrar. |
| `42:1/2` | Reserved backup hub. |
| `42:1/10-99` | Infrastructure and future hubs. |
| `42:1/100-899` | Assignable top-level member BBS nodes. |
| `42:1/900+` | Test and lab nodes. |
| `42:2/*` and beyond | Future nets. |

The `oxidebbs-oxidenet` crate validates that OxideNet addresses are in zone
`42`, rejects node `0`, classifies the ranges above, and provides a helper to
select the next unused top-level member address.

Member address assignment currently excludes point addresses such as
`42:1/100.1`. Points can still be parsed by the shared address model, but
top-level OxideNet member allocation is intentionally limited to node addresses
for the schema-8 hub/member registry.

DecentDB stores approved application assignments and node records in
`network_applications` and `network_nodes`. Runtime application approval,
address tombstones, explicit address reassignment, and public nodelist
generation remain planned v1.2 work.
