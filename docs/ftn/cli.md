# FTN CLI

OxideBBS exposes FTN administration under `oxidebbs-server net`.

Implemented read-only and import commands:

```bash
oxidebbs-server net status <network>
oxidebbs-server net links list [--network <network>]
oxidebbs-server net areas list [--network <network>]
oxidebbs-server net logs <link>
oxidebbs-server net nodelist import <file> [--network <network>]
oxidebbs-server net nodelist apply-diff <file> --base <full-list-file> [--network <network>]
oxidebbs-server net nodelist list [--network <network>] [--limit N]
oxidebbs-server net nodelist lookup <address> [--network <network>]
```

`--json` is supported through the global CLI flag for these commands.

`net links list` and `net logs` redact link passwords in JSON output.

`net nodelist import` parses a full nodelist file and atomically replaces the
stored nodelist rows for the selected network profile. `net nodelist apply-diff`
applies a plain text FTS-style `NODEDIFF.xxx` file to a supplied full base list,
parses the resulting nodelist, and uses the same atomic replacement path.

Not implemented yet:

```bash
oxidebbs-server net toss <network>
oxidebbs-server net scan <network>
oxidebbs-server net poll <link>
```

Those commands return explicit errors until the tosser, scanner, and BinkP
session engine are implemented.
