# AreaFix

AreaFix is the FTN netmail robot used to manage echomail subscriptions. A linked
system sends netmail to `AreaFix` at the board's FTN address, with the link
password in the subject line and commands in the message body.

OxideBBS includes the pure `oxidebbs-ftn` command parser, inbound tosser
processing for netmail addressed to `AreaFix` or `AreaMgr`, and a local
sysop-side executor:

```bash
oxidebbs-server net areafix send <link> "<commands>" --password <password> [--network <network>]
```

The executor authenticates the supplied password against the configured link
password, applies subscription commands to DecentDB, audits the activity, queues
an AreaFix reply as outbound netmail, prints the reply text, and queues targeted
per-area/per-link rescans for `+AREA.TAG !` commands. Inbound AreaFix netmail
uses the same processor and queues reply netmail plus rescan requests during
`net toss`.

The parser recognizes every supported command form and normalizes area tags to
uppercase ASCII.

Supported commands:

| Command | Meaning |
| --- | --- |
| `%LIST` | Request a list of available areas. |
| `%QUERY` | Request the areas currently subscribed for the link. |
| `%HELP` | Request command help. |
| `+AREA.TAG` | Subscribe to an area. |
| `-AREA.TAG` | Unsubscribe from an area. |
| `+AREA.TAG !` | Subscribe and request a rescan. |

Command keywords are case-insensitive. Area tags may contain ASCII letters,
digits, `.`, `_`, and `-`; parsed tags are normalized to uppercase.

Example body:

```text
+FSX_GEN
+RETRO.BBS !
%QUERY
```

Runtime behavior:

- inbound AreaFix netmail is authenticated against the configured link password
- `%LIST`, `%QUERY`, `%HELP`, subscribe, unsubscribe, and rescan commands are
  audited
- generated replies are queued as pending outbound netmail packets and are
  materialized by `net scan`
- `+AREA.TAG !` creates a pending `network_rescan_queue` row for that link and
  area
- `net rescan list/process/cancel` lets the sysop inspect, process, or cancel
  queued rescans
- manual `net areas subscribe` and `net areas unsubscribe` remain available for
  direct operator-side subscription changes
