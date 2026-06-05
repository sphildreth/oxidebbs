# OxideNet Addressing

OxideNet uses FTN-style addresses from the shared `oxidebbs-network`
`FtnAddress` model.

Validated ranges:

| Range | Meaning |
| --- | --- |
| `42:1/1` | Primary hub and registrar. |
| `42:1/2` | Backup hub reservation. |
| `42:1/10-99` | Infrastructure and additional hubs. |
| `42:1/100-899` | Assignable member BBS nodes. |
| `42:1/900+` | Test and lab nodes. |
| `42:2/*` and beyond | Future nets. |

Approving an application without `--address` assigns the next unused top-level
member address. Point addresses parse through the shared model, but v1.2 member
allocation intentionally assigns top-level `42:1/N` nodes.
