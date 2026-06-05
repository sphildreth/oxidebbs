# FTN CLI

OxideBBS exposes FTN administration under `oxidebbs-server net`.

Implemented read-only, import, toss, scan, poll, and metadata commands:

```bash
oxidebbs-server net toss <network>
oxidebbs-server net scan <network>
oxidebbs-server net status <network>
oxidebbs-server net links list [--network <network>]
oxidebbs-server net links show <link>
oxidebbs-server net areas list [--network <network>]
oxidebbs-server net areas subscribe <area-tag> <link> [--network <network>]
oxidebbs-server net areas unsubscribe <area-tag> <link> [--network <network>]
oxidebbs-server net queue <link>
oxidebbs-server net packets summary [--network <network>]
oxidebbs-server net packets show <packet-id>
oxidebbs-server net packets retry <packet-id>
oxidebbs-server net packets mark-quarantined <packet-id> --reason <text>
oxidebbs-server net packets inbound [--network <network>] [--limit N]
oxidebbs-server net packets outbound [--network <network>] [--limit N]
oxidebbs-server net packets quarantine [--network <network>] [--limit N]
oxidebbs-server net logs [link] [--limit N]
oxidebbs-server net poll <link>
oxidebbs-server net poll --all
oxidebbs-server net poll <link> --dry-run
oxidebbs-server net poll --all --dry-run
oxidebbs-server net areafix send <link> "<commands>" --password <password> [--network <network>]
oxidebbs-server net nodelist import <file> [--network <network>]
oxidebbs-server net nodelist apply-diff <file> --base <full-list-file> [--network <network>]
oxidebbs-server net nodelist list [--network <network>] [--limit N]
oxidebbs-server net nodelist lookup <address> [--network <network>]
```

`--json` is supported through the global CLI flag for these commands.

`net links list`, `net links show`, `net queue`, `net logs`, and `net poll
--dry-run` redact link passwords in JSON output. Poll dry-run output includes a
transport-security preflight plan showing whether the link requires TLS,
attempts TLS, allows plaintext, or carries an operator warning.

`net toss <network>` scans
`paths.runtime/network/<network>/inbound/drop`, imports known echomail AREA
messages from raw `.pkt` files and safe top-level `.pkt` entries inside ZIP
bundles, records packet/message state in DecentDB, moves successful inputs to
`inbound/archive`, and moves malformed or unauthorized inputs to
`inbound/quarantine`. The packet origin address and packet password must match
an enabled link on the selected network profile. Unknown AREA tags and netmail
are recorded as quarantined network messages until the remaining routing and
netmail phases land.

`net scan <network>` scans subscribed local echomail areas and writes outbound
Type-2+ `.pkt` files under
`paths.runtime/network/<network>/outbound/<link>/ready`. It records outbound
packet rows as `pending` and network-message rows as `exported`.

`net poll <link>` connects to a plaintext-legacy BinkP peer, authenticates with
`M_ADR` and `M_PWD`, sends pending outbound packet files for that link, receives
the peer batch into `paths.runtime/network/<network>/inbound/drop`, marks sent
packet rows processed after `M_GOT`, and records `network_poll_log` state.
`net poll --all` repeats that workflow for enabled links. TLS-required and
TLS-opportunistic links use the implemented TLS session support with opportunistic
fallback; use `--dry-run` to inspect their security plan.

`net packets summary`, `net packets show`, `net packets retry`, and
`net packets mark-quarantined` operate on DecentDB packet state. `retry` resets
failed or quarantined packet rows to `pending`, clearing processed/error
metadata. `mark-quarantined` records a quarantine reason and processed
timestamp. These state controls do not move files.

`net areas subscribe` and `net areas unsubscribe` update DecentDB subscription
metadata and audit the change. `net areafix send` executes AreaFix command text
locally for a link after password authentication, mutates
`network_area_subscriptions`, audits the activity, and prints the reply text.
Generated AreaFix replies are not queued as netmail yet, and rescan requests
are acknowledged without queueing historical messages.

`net nodelist import` parses a full nodelist file and atomically replaces the
stored nodelist rows for the selected network profile. `net nodelist apply-diff`
applies a plain text FTS-style `NODEDIFF.xxx` file to a supplied full base list,
parses the resulting nodelist, and uses the same atomic replacement path.

Remaining network CLI gaps are inbound AreaFix netmail processing, AreaFix reply
netmail queueing, packet retention, and TLS-capable BinkP sessions.
