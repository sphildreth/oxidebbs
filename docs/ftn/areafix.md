# AreaFix

AreaFix is the FTN netmail robot used to manage echomail subscriptions. A linked
system sends netmail to `AreaFix` at the board's FTN address, with the link
password in the subject line and commands in the message body.

OxideBBS v1.2 currently includes the pure `oxidebbs-ftn` command parser and a
local sysop-side executor:

```bash
oxidebbs-server net areafix send <link> "<commands>" --password <password> [--network <network>]
```

The executor authenticates the supplied password against the configured link
password, applies subscription commands to DecentDB, audits the activity, and
prints the reply text that would be sent to the link. It is intended for local
operator testing and for proving the AreaFix state transitions before inbound
netmail processing is wired into the tosser.

The parser recognizes every planned command form and normalizes area tags to
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

Current runtime boundaries:

- inbound netmail addressed to `AreaFix` is not processed automatically yet
- generated replies are printed by the CLI but are not queued as netmail yet
- rescan requests are acknowledged but do not enqueue historical messages yet
- manual `net areas subscribe` and `net areas unsubscribe` remain the current
  direct operator-side subscription mutation commands
