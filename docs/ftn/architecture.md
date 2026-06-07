# FTN Architecture

OxideBBS includes built-in FTN networking. A sysop does not need
BinkleyTerm, FrontDoor, InterMail, binkd, or another external mailer just to
exchange echomail and netmail.

Those tools are still useful historical references, but OxideBBS splits the
work into clearer parts:

| Part | What it does |
| --- | --- |
| Scanner | Exports local messages into outbound FTN packet or bundle files. |
| Mailer | Moves packet or bundle files between systems. OxideBBS uses BinkP for this. |
| Tosser | Imports inbound packet or bundle files into local message areas. |
| Nodelist and routing | Decides where netmail should go and which links receive areas. |
| AreaFix | Lets linked systems request area lists and subscriptions by netmail. |

## Do You Need A Classic Mailer?

No. FTN tossing itself only needs files.

If a `.pkt` or bundle file is already in the inbound directory, OxideBBS can
toss it into the message base without opening a network connection. Likewise,
OxideBBS can scan local messages into outbound files without calling another
system.

The mailer is only needed to move those files between systems automatically.
The built-in mailer is BinkP over TCP/IP.

## Built-In BinkP Mailer

BinkP is the TCP/IP protocol commonly used by modern FTN systems to exchange
mail files.

The OxideBBS BinkP mailer provides:

- outbound polling of configured links
- optional inbound listener for known links
- BinkP password authentication
- TLS-required mode for OxideNet and private networks
- explicit plaintext legacy mode for traditional FTN links
- opportunistic TLS for legacy-compatible links
- retry and backoff
- one active session per link
- poll logs and status for sysops

BinkP does not use ZMODEM, XMODEM, or YMODEM. Those are caller file-transfer
protocols. FTN network mail exchange uses BinkP data frames.

## Normal Message Flow

Outbound echomail or netmail follows this path:

```text
local message area
  -> scanner
  -> outbound packet or bundle
  -> BinkP poller
  -> remote system
```

Inbound echomail or netmail follows this path:

```text
remote system
  -> BinkP listener or poll response
  -> inbound packet or bundle
  -> tosser
  -> local message area
```

The separation is intentional. It makes troubleshooting easier because a sysop
can tell whether a problem is packet creation, transport, or packet import.

## Manual Operation

The CLI keeps each operation available separately:

```bash
oxidebbs-server net scan fidonet
oxidebbs-server net poll fidonet_hub
oxidebbs-server net toss fidonet
```

Use `scan` when local messages need to be packed for links.

Use `poll` when files should be exchanged with a link over BinkP.

Use `toss` when inbound files should be imported into the BBS.

`poll_schedule_minutes` is stored with each link for operations visibility and
future automation, but the current runtime does not start a background mail
scheduler. Run `net scan`, `net poll`, and `net toss` manually or from external
host scheduling such as systemd timers or cron.

## File-Only Operation

OxideBBS can also operate without the built-in BinkP listener or poller.

In that mode:

1. `net scan` creates outbound files.
2. A sysop or external mailer sends those files.
3. A sysop or external mailer drops received files into the inbound drop
   directory.
4. `net toss` imports the inbound files.

This is useful for testing, manual recovery, or sites that already have an
external transport workflow.

OxideBBS does not require Binkley-style `.flo` files, BSO directories, EMSI, modem
answering, or caller pass-through from a front-end mailer.

## Spool Directories

The active spool layout is profile-scoped under the runtime directory:

```text
runtime/network/
  <profile-key>/
    inbound/
      drop/
      archive/
      quarantine/
    temp-inbound/
    outbound/
      <link-key>/
        ready/
        bundled/
    nodelist/
```

Important directories:

- `inbound/drop`: manual or external-mailer drop point. `net toss <profile>`
  scans this directory.
- `inbound/archive`: packet and bundle files that were accepted by the tosser.
- `outbound/<link-key>/ready`: packet files ready for BinkP or external
  transport.
- `outbound/<link-key>/bundled`: ZIP arcmail bundles created for links using
  ZIP compression.
- `inbound/quarantine`: files that were malformed, unauthorized, unsafe, or failed
  validation.

Sysops should not place files in `temp-inbound`. It is an implementation
working area.

## Security Model

Traditional FTN links often use plaintext BinkP. OxideBBS allows that only when
the link explicitly opts in with legacy compatibility.

For OxideNet and new private networks, TLS is required by default.

Plaintext legacy mode should be treated like telnet exposure: credentials and
message contents can be observed on the network. OxideBBS must warn at startup
and in poll logs when a link uses plaintext legacy mode.

## Troubleshooting Boundaries

When network mail fails, identify which boundary failed:

| Symptom | Likely area |
| --- | --- |
| Local messages never create outbound files | Scanner, area subscription, message visibility, or routing. |
| Outbound files exist but remote never receives them | BinkP mailer, host, port, password, TLS, or link scheduling. |
| Poll receives files but messages do not appear | Tosser, packet password, AREA mapping, duplicate detection, or quarantine. |
| Netmail does not route onward | Nodelist, link routing, hold/crash policy, or unknown destination. |
| Repeated duplicate posts are skipped | Duplicate detector is working; inspect duplicate logs for source details. |

`net status`, `net toss`, `net scan`, `net poll`, `net links list/show`, `net
areas list/subscribe/unsubscribe`, `net queue`, `net packets`, `net logs`, `net
rescan`, `net poll --dry-run`, `net areafix send`, and nodelist
import/apply-diff/list/lookup/count commands expose, import, export, transport,
or update DecentDB network state. TLS-capable poll sessions, inbound listener
execution, AreaFix netmail processing, reply netmail, rescan queueing, packet
retention, and operations stats are implemented.

## Operations Scope

The FTN implementation includes shared network configuration, DecentDB
network tables, protocol-neutral network types, FTN packet/kludge/duplicate
primitives, inbound raw/ZIP/ARJ packet tossing, outbound packet scanning and ZIP
bundle creation, TLS/plaintext BinkP client polling, inbound BinkP listener
execution, netmail delivery/forwarding, AreaFix command processing, BinkP frame
I/O, transport-security policy, nodelist import/apply-diff/list/lookup/count,
packet retention cleanup, and operations stats.

The following pieces are not part of day-to-day sysop operation:

- nodelist-driven runtime routing integration
- outbound ARJ bundle creation

Developer planning notes live under `design/` in the repository; day-to-day
sysop operation should use the `net` commands documented here.
