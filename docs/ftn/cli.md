# FTN CLI

OxideBBS exposes FTN administration under `oxidebbs-server net`.

Implemented read-only, import, and metadata commands:

```bash
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
oxidebbs-server net poll <link> --dry-run
oxidebbs-server net poll --all --dry-run
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

`net packets summary`, `net packets show`, `net packets retry`, and
`net packets mark-quarantined` operate only on DecentDB packet state today.
`retry` resets failed or quarantined packet rows to `pending`, clearing
processed/error metadata. `mark-quarantined` records a quarantine reason and
processed timestamp. Neither command moves files or performs packet processing
until the tosser/scanner/spool runtime exists.

`net areas subscribe` and `net areas unsubscribe` update DecentDB subscription
metadata and audit the change. They do not send AreaFix netmail; AreaFix
execution is a separate planned workflow. The lower-level `oxidebbs-ftn`
AreaFix command parser exists for inbound netmail integration work.

`net nodelist import` parses a full nodelist file and atomically replaces the
stored nodelist rows for the selected network profile. `net nodelist apply-diff`
applies a plain text FTS-style `NODEDIFF.xxx` file to a supplied full base list,
parses the resulting nodelist, and uses the same atomic replacement path.

Not implemented yet:

```bash
oxidebbs-server net toss <network>
oxidebbs-server net scan <network>
oxidebbs-server net poll <link>
oxidebbs-server net poll --all
```

Those commands return explicit errors until the tosser, scanner, and BinkP
session engine are implemented, except for the `--dry-run` preflight described
above.
