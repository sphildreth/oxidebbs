# FTN Architecture

OxideBBS v1.2 is planned to include built-in FTN networking. That means a sysop
should not need BinkleyTerm, FrontDoor, InterMail, binkd, or another external
mailer just to exchange echomail and netmail.

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
For v1.2, the built-in mailer is BinkP over TCP/IP.

## Built-In BinkP Mailer

BinkP is the TCP/IP protocol commonly used by modern FTN systems to exchange
mail files.

The OxideBBS BinkP mailer is planned to provide:

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

The planned full v1.2 CLI keeps each operation available separately:

```bash
oxidebbs-server net scan fidonet
oxidebbs-server net poll fidonet_hub
oxidebbs-server net toss fidonet
```

Use `scan` when local messages need to be packed for links.

Use `poll` when files should be exchanged with a link over BinkP.

Use `toss` when inbound files should be imported into the BBS.

The background scheduler may run the full cycle automatically for enabled
links, but the separate commands remain important for setup and debugging.

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

v1.2 does not require Binkley-style `.flo` files, BSO directories, EMSI, modem
answering, or caller pass-through from a front-end mailer.

## Spool Directories

The planned spool layout is under the network runtime directory:

```text
runtime/network/
  inbound/
    drop/
  temp-inbound/
  outbound/
    <link-key>/
      ready/
      busy/
      sent/
      hold/
      temp/
  archive/
  quarantine/
  nodelist/
```

Important directories:

- `inbound/drop`: manual or external-mailer drop point.
- `outbound/<link-key>/ready`: files ready for BinkP or external transport.
- `outbound/<link-key>/busy`: files claimed by an active session.
- `outbound/<link-key>/sent`: files acknowledged by the remote.
- `quarantine`: files that were malformed, unauthorized, unsafe, or failed
  validation.

Sysops should not place files in `temp-inbound`, `busy`, or `temp` directories.
Those are implementation working areas.

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

`net status`, `net links list`, `net areas list`, `net logs`, and nodelist
import/list/lookup commands already expose DecentDB network state. `net queue`,
`net packets`, toss/scan/poll execution, and AreaFix remain planned.

## Current Implementation Status

The v1.2 foundation currently includes shared network configuration, DecentDB
network tables, protocol-neutral network types, FTN packet/kludge/duplicate
primitives, BinkP frame I/O, and nodelist import/lookup.

The following pieces are still planned:

- tosser
- scanner
- bundle handling
- differential nodelist updates and routing
- AreaFix
- BinkP client and server
- operational toss/scan/poll `net` CLI commands

See `design/MAILER.md` and `design/FTN_PLAN.md` in the repository for the
implementation specifications.
