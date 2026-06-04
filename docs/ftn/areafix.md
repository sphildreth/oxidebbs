# AreaFix

AreaFix is the FTN netmail robot used to manage echomail subscriptions. A linked
system sends netmail to `AreaFix` at the board's FTN address, with the link
password in the subject line and commands in the message body.

OxideBBS v1.2 currently includes the pure `oxidebbs-ftn` command parser. The
parser recognizes every planned command form and normalizes area tags to
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

- the parser does not authenticate the subject-line password
- it does not mutate `network_area_subscriptions`
- it does not generate reply netmail
- it does not enqueue rescans or write activity logs
- manual `net areas subscribe` and `net areas unsubscribe` remain the current
  operator-side subscription mutation commands
