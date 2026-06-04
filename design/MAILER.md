# OxideBBS FTN Mailer Design

Document status: v1.2 implementation specification

Created: 2026-06-04

Applies to: `design/RELEASE_v1_2_PLAN.md` P9 through P14,
`design/FTN_PLAN.md`, ADR 0027, ADR 0031

## Purpose

This document defines the OxideBBS mailer plan for v1.2.

The short version is:

- FTN tossing does not require BinkleyTerm, FrontDoor, InterMail, binkd, or any
  other external front-end mailer.
- OxideBBS v1.2 includes its own BinkP mailer for real network exchange.
- The built-in mailer is not a full BinkleyTerm or FrontDoor clone. It provides
  the file transport functions OxideBBS needs: BinkP listener, BinkP poller,
  authentication, file exchange, retry, scheduling, logging, and queue safety.
- Tossing, scanning, and mailing are separate responsibilities. Implementations
  must keep those boundaries clear.

This document exists so coding agents do not need to infer what "mailer" means
from legacy BBS history.

## Terminology

### Packet Engine

The FTN packet engine reads and writes legacy FTN `.pkt` files. In OxideBBS it
lives in `oxidebbs-ftn`.

The packet engine understands:

- Type-2 and Type-2+ packet headers
- FTN message headers
- echomail and netmail body bytes
- FTN kludges such as `AREA`, `MSGID`, `REPLY`, `INTL`, `FMPT`, and `TOPT`
- SEEN-BY and PATH data

The packet engine does not open network sockets and does not decide when to
call another system.

### Tosser

The tosser imports inbound FTN files into the local BBS.

The tosser:

- scans inbound spool directories
- extracts supported bundles
- reads `.pkt` files
- validates packet address and packet password
- detects duplicates
- imports echomail into local message areas
- delivers local netmail
- forwards non-local netmail to the outbound queue when routing allows it
- records packet status in DecentDB
- quarantines malformed, unauthorized, or unsafe files

The tosser can run without any mailer. A sysop can copy `.pkt` or bundle files
into the inbound spool and run `oxidebbs-server net toss`.

### Scanner

The scanner exports local messages into outbound FTN files.

The scanner:

- finds local echomail and netmail eligible for export
- evaluates subscriptions, message visibility, moderation, and routing
- composes outbound FTN messages
- writes Type-2+ packets
- creates configured bundles
- records outbound files in DecentDB
- places ready-to-send files into the outbound spool

The scanner can run without any mailer. A sysop can run
`oxidebbs-server net scan` and move the generated outbound files manually or
with an external transport.

### Mailer

The mailer moves already-created network files between systems.

For v1.2, the OxideBBS mailer means BinkP:

- outbound polling to a configured link
- inbound listening for known links
- BinkP authentication
- BinkP file offers and file data frames
- send and receive of `.pkt`, bundle, and TIC files
- retry and backoff
- one-active-session-per-link guarding
- poll logging

The mailer must not parse FTN message bodies, import messages, or compose local
messages into packets. Those are tosser and scanner responsibilities.

### Front-End Mailer

Classic software such as BinkleyTerm and FrontDoor was usually a front-end
mailer. It answered modem calls, identified mail sessions, exchanged mail
files, called other nodes on schedules, and could pass human callers through to
the BBS.

OxideBBS v1.2 does not implement a classic front-end mailer. The v1.2 system
implements the pieces required for TCP/IP FTN participation:

- BinkP transport
- packet/bundle toss and scan
- nodelist and routing
- AreaFix
- operational CLI and TUI surfaces

FTS-0001 dial-up mailer sessions, EMSI, WaZOO, Hydra, Janus, YooHoo/2U2, BSO
file boxes, and modem mailer call negotiation are outside v1.2 unless a later
ADR explicitly supersedes this document.

## Role Boundaries

Implementations must preserve this boundary:

```text
local message base
  -> scanner
  -> outbound packet or bundle files
  -> mailer
  -> remote system

remote system
  -> mailer
  -> inbound packet or bundle files
  -> tosser
  -> local message base
```

The mailer is transport. The tosser and scanner are FTN content processing.

The BinkP mailer must not depend on caller file-transfer code. ADR 0031 keeps
caller ZMODEM/XMODEM-CRC transfers separate from FTN network mail exchange.
BinkP sends file bytes as BinkP data frames and must not use ZMODEM, XMODEM,
YMODEM, or external `rz`/`sz` programs.

## Crate Responsibilities

### `oxidebbs-network`

Owns protocol-neutral network types:

- FTN-style addresses
- network profiles
- network links
- network message envelopes
- packet boundaries
- duplicate keys
- queue state enums
- conversion traits where they can remain protocol-neutral

This crate must not know about BinkP frames, arcmail bundles, DecentDB, or the
server runtime.

### `oxidebbs-ftn`

Owns legacy FTN behavior:

- packet reader and writer
- echomail and netmail parser/composer
- bundle extraction and creation
- tosser
- scanner
- nodelist parser and index logic
- netmail router
- AreaFix
- DecentDB-backed FTN repositories where they are not protocol-neutral

This crate can depend on `oxidebbs-network`, `oxidebbs-core`, and `oxidebbs-db`
where needed by the existing phase plan. It must not depend on
`oxidebbs-server`.

### `oxidebbs-binkp`

Owns BinkP protocol behavior:

- frame parser and writer
- command and data frame state machines
- BinkP client
- BinkP server
- authentication
- TLS/plaintext transport policy integration points
- retry and backoff primitives
- per-link session guard primitives

This crate must not parse `.pkt` message content. It handles files by name,
size, timestamp, byte stream, and link identity.

### `oxidebbs-server`

Owns runtime orchestration:

- start or stop the BinkP listener
- run the scheduled poller
- run manual `net` commands
- connect config to link records
- connect repositories to the tosser, scanner, and mailer
- record audit and poll logs
- expose status to CLI and TUI

`oxidebbs-server` may compose higher-level workflows such as "scan, poll, then
toss" for the background scheduler, but the underlying operations must remain
callable independently for debugging and external-mailer deployments.

## Runtime Modes

### Integrated Mode

Integrated mode is the default v1.2 design target.

In integrated mode, OxideBBS owns the whole network loop:

1. The scanner creates outbound packets and bundles.
2. The BinkP poller sends queued outbound files and receives inbound files.
3. The tosser imports received packets and bundles.
4. DecentDB records packet status, duplicate status, poll status, and errors.

The background network scheduler may run this full cycle for enabled links.
Manual CLI commands must also exist so sysops can run each step separately.

### Poll-Only Mode

Poll-only mode is useful for debugging.

In this mode, `net poll <link-name>` only transports files already present in
the outbound spool and commits newly received files to the inbound spool. It
does not silently scan new local messages and does not silently toss received
files unless the command explicitly adds a documented option in a later phase.

The planned v1.2 command list currently contains `net scan`, `net poll`, and
`net toss` as separate operations. Coding agents must keep those commands
separate.

### File-Only Mode

File-only mode is required for tests and useful for operators who want to use
an external mailer.

In file-only mode:

- `net scan` writes outbound packet or bundle files.
- an operator or external tool moves outbound files to another system.
- an operator or external tool places inbound packet or bundle files into the
  inbound drop directory.
- `net toss` imports the inbound files.

No BinkP listener or poller is required for file-only mode.

### External-Mailer Mode

External-mailer mode is supported as directory drop interop, not as a promise
to implement every legacy front-end mailer convention.

The operator may disable the built-in BinkP listener and poller, then configure
an external tool to exchange files using the OxideBBS spool directories.

v1.2 external-mailer interop means:

- inbound files dropped into the documented inbound directory are tossed
  normally
- outbound files created by the scanner are ordinary `.pkt`, ZIP bundle, ARJ
  bundle, or TIC files
- packet passwords and FTN addressing are still validated by the tosser
- poll logs for the external tool are not automatically created unless an
  OxideBBS command records them

v1.2 external-mailer interop does not require:

- Binkley-style outbound directory naming
- `.flo`, `.hlo`, `.clo`, `.dlo`, or BSO queue files
- EMSI session negotiation
- modem answering
- passing caller sessions through from a front-end mailer

If first-class BSO or legacy front-end mailer integration is desired later, it
must be added through a separate ADR.

## Configuration

The existing v1.2 foundation already includes:

```toml
[network]
enabled = false

[network.profiles.fidonet]
name = "FidoNet"
enabled = false
adapter = "legacy-ftn"

[network.profiles.fidonet.local_address]
zone = 1
net = 105
node = 42
point = 0

[network.links.fidonet_hub]
network = "fidonet"
address = "1:105/0"
host = "fidonet.example.net"
binkp_port = 24554
password = ""
poll_schedule_minutes = 60
enabled = false
compression = "zip"
transport_security = "plaintext_legacy"
legacy_compatible = true
```

P13 must add the BinkP runtime settings below unless an accepted ADR replaces
them before implementation:

```toml
[network.binkp]
listener_enabled = false
bind = "0.0.0.0:24554"
poller_enabled = true
poll_on_startup = false
max_concurrent_sessions = 8
session_timeout_seconds = 300
connect_timeout_seconds = 30
```

Rules:

- If `network.enabled = false`, no network scanner, tosser scheduler, BinkP
  listener, or BinkP poller starts.
- If `network.binkp.listener_enabled = false`, OxideBBS does not bind a BinkP
  listening socket.
- If `network.binkp.poller_enabled = false`, automatic scheduled polling does
  not run. Manual `net poll` commands still work.
- `poll_on_startup = false` means the scheduler waits until each link's first
  due interval before the first automatic poll. A sysop can run `net poll`
  manually immediately after startup.
- `max_concurrent_sessions` is global. A second session for the same link is
  rejected even if the global limit has capacity.
- `bind` must be parsed as a socket address. Public exposure is an operator
  decision and must be documented with security warnings.

P11 or P13 must add network spool path settings. If omitted, defaults are
derived from `paths.runtime`:

```toml
[network.paths]
spool = "./runtime/network"
inbound = "./runtime/network/inbound"
temp_inbound = "./runtime/network/temp-inbound"
outbound = "./runtime/network/outbound"
archive = "./runtime/network/archive"
quarantine = "./runtime/network/quarantine"
nodelist = "./runtime/network/nodelist"
```

Rules:

- Paths may be relative to the process working directory, matching existing
  OxideBBS path behavior.
- Startup must create missing spool directories when setup/runtime initialization
  owns the data directory.
- Startup must fail with an actionable error if a required enabled network path
  exists but is not a directory or is not writable.
- File paths stored in `network_packets.filename` must be relative to
  `network.paths.spool`, not absolute host paths.
- Caller-facing errors must not expose absolute host paths.

## Spool Layout

The implementation must use this logical layout under `network.paths.spool`:

```text
network/
  inbound/
    drop/
    <network-key>/
      <link-key>/
  temp-inbound/
    <session-id>/
  outbound/
    <link-key>/
      ready/
      busy/
      sent/
      hold/
      temp/
  archive/
    inbound/
    outbound/
  quarantine/
    inbound/
    outbound/
  nodelist/
```

Directory meaning:

- `inbound/drop`: operator and external-mailer drop point. The tosser resolves
  network and link by packet header and configured passwords.
- `inbound/<network-key>/<link-key>`: committed files received by the built-in
  BinkP mailer after link authentication.
- `temp-inbound/<session-id>`: incomplete BinkP receive files. The tosser must
  ignore this directory.
- `outbound/<link-key>/ready`: files available for the BinkP mailer or external
  transport.
- `outbound/<link-key>/busy`: files claimed by an active mailer session.
- `outbound/<link-key>/sent`: files successfully acknowledged by the remote.
- `outbound/<link-key>/hold`: files held by operator action, routing policy, or
  retry policy.
- `outbound/<link-key>/temp`: scanner write-in-progress directory. The mailer
  must ignore this directory.
- `archive`: retention destination for successfully processed files.
- `quarantine`: retention destination for unsafe, malformed, unauthorized, or
  failed files.

All file moves between `temp`, `ready`, `busy`, `sent`, `archive`, and
`quarantine` must use atomic rename when the source and destination are on the
same filesystem. If an atomic rename is not possible, copy, fsync where
available, verify size and hash, then remove the source.

## DecentDB State

P2 already added shared `network_*` tables. The mailer and FTN runtime must use
them consistently.

### `network_packets`

Each inbound or outbound file tracked by OxideBBS must have a
`network_packets` row.

Rules:

- `direction = 'inbound'` for files received from BinkP, dropped by an external
  mailer, or copied manually for tossing.
- `direction = 'outbound'` for files created by the scanner.
- `filename` is the path relative to `network.paths.spool`.
- `sha256` is computed after the file is complete and before the row is marked
  ready for processing.
- `status = 'pending'` means the file is ready for the next phase.
- `status = 'processing'` means a tosser, scanner, mailer, or retention job has
  claimed the file.
- `status = 'processed'` means inbound was tossed successfully or outbound was
  acknowledged by the remote.
- `status = 'quarantined'` means the file was moved to quarantine.
- `status = 'failed'` means an operation failed but the file was not
  quarantined. The operator must be able to inspect and retry or quarantine.

Because the current schema does not have a dedicated `storage_path` column,
coding agents must not store absolute paths in `filename`.

### `network_poll_log`

Each BinkP connection attempt must have a `network_poll_log` row.

Rules:

- Create the row before attempting the connection or before accepting an inbound
  session.
- `direction = 'outbound'` for a poll initiated by OxideBBS.
- `direction = 'inbound'` for a remote BinkP connection accepted by OxideBBS.
- `status` values should be stable strings: `started`, `success`, `failed`,
  `busy`, `auth_failed`, `timeout`, or `canceled`.
- `bytes_in` and `bytes_out` count BinkP file payload bytes, not TCP framing,
  TLS overhead, or BinkP command frame bytes.
- `packets_in` and `packets_out` count committed files, not individual FTN
  messages.
- Plaintext legacy sessions must include a warning in logs and operational
  status surfaces.

If the implementation needs more structured status values than the existing
schema allows, add a checked schema migration in the same phase.

## BinkP Session Lifecycle

The BinkP implementation must follow `design/FTN_PLAN.md` for frame format and
commands. This section defines how that protocol maps into OxideBBS runtime
state.

### Outbound Poll

Outbound poll steps:

1. Resolve the enabled link by key.
2. Reject the poll if the profile or link is disabled.
3. Acquire the per-link session guard.
4. Create `network_poll_log` with `direction = 'outbound'`.
5. Apply the link's transport security policy:
   - `tls_required`
   - `tls_opportunistic`
   - `plaintext_legacy`
6. Connect to `host:binkp_port`.
7. Exchange BinkP greeting and address frames.
8. Authenticate with `M_PWD`.
9. Move outbound files from `ready` to `busy` only after the session is
   authenticated.
10. Offer each file with `M_FILE`.
11. Send file bytes as BinkP data frames.
12. Move acknowledged files to `sent` and mark their `network_packets` rows as
   `processed`.
13. Move skipped files back to `ready` or `hold` according to retry policy.
14. Receive remote file offers and commit received files through
   `temp-inbound`.
15. Exchange `M_EOB` and finish after both sides have no pending files or
   acknowledgements.
16. Update `network_poll_log`.
17. Release the per-link session guard.

Outbound poll must not delete an outbound file merely because a TCP connection
was made. Deletion or archival is allowed only after a positive BinkP
acknowledgement such as `M_GOT` for that file.

### Inbound Session

Inbound session steps:

1. Accept TCP connection if the listener is enabled and capacity exists.
2. Create `network_poll_log` with `direction = 'inbound'` after the link is
   identified. If identification fails before link identity is known, write an
   operational log entry without a `network_poll_log` row.
3. Apply listener transport security.
4. Read remote `M_ADR`.
5. Match the remote address to one enabled configured link.
6. Reject unknown, disabled, or duplicate active links with `M_ERR` or `M_BSY`.
7. Authenticate `M_PWD`.
8. Acquire the per-link session guard.
9. Receive offered files into `temp-inbound/<session-id>`.
10. Validate advertised file names using the same path-safety rules as the
    tosser. BinkP filenames are untrusted metadata.
11. Commit completed files into `inbound/<network-key>/<link-key>`.
12. Create inbound `network_packets` rows after the file is complete and hashed.
13. Offer outbound ready files for that link.
14. Finish with `M_EOB`, update poll log, and release the session guard.

Inbound BinkP must never let the peer choose an absolute path or write outside
the network spool.

## Scheduler Behavior

The background scheduler is an `oxidebbs-server` concern.

Rules:

- The scheduler starts only when `network.enabled = true` and
  `network.binkp.poller_enabled = true`.
- It considers only enabled links whose profiles are also enabled.
- It uses each link's `poll_schedule_minutes`.
- It must not hold locks across `.await`.
- It must enforce one active session per link.
- It must use jitter of up to 10 percent of the interval to avoid all links
  polling at the same instant after restart.
- It must not poll immediately at startup when `poll_on_startup = false`.
- It must expose next-due and last-result data to CLI and TUI.

The recommended automatic cycle is:

```text
scan enabled networks
poll due links
toss newly received inbound files
record summary
```

Manual CLI commands remain independent. `net scan`, `net poll`, and `net toss`
must each be usable alone.

## Security

BinkP credentials and message contents are sensitive.

Rules:

- OxideNet and non-legacy private profiles default to `tls_required`.
- Legacy FTN links may use `plaintext_legacy` only when the link explicitly opts
  in and the profile adapter is `legacy-ftn`.
- `tls_opportunistic` must attempt TLS first and may fall back to plaintext only
  for legacy-compatible links.
- Startup and poll logs must warn for plaintext legacy links.
- BinkP passwords must not be printed in logs, TUI screens, CLI output, panic
  messages, or test snapshots.
- Packet passwords inside legacy `.pkt` files follow FTN packet rules and are
  separate from BinkP session passwords.
- File names from remote systems must be sanitized to basenames.
- Received file size limits must be enforced before writing unbounded data.
- Unknown links must not be allowed to deposit files into the trusted inbound
  directory.
- Repeated authentication failures should be rate limited or temporarily refused
  at the server layer.

## Error Handling

Error handling must be observable and retryable.

Rules:

- Malformed BinkP frames end the BinkP session and mark the poll failed.
- Malformed `.pkt` content is handled by the tosser quarantine policy, not by
  the mailer.
- Connection failure leaves outbound files in `ready`.
- Failure after a file is moved to `busy` must move that file back to `ready`
  unless the retry policy moves it to `hold`.
- Received incomplete files stay in `temp-inbound` only until cleanup moves them
  to quarantine or deletes them according to retention policy.
- Wrong BinkP password records `auth_failed`.
- Remote busy records `busy` and schedules retry.
- Operator cancellation records `canceled`.
- Timeouts record `timeout`.

No failed mailer operation may silently discard a packet or bundle.

## CLI And TUI Surfaces

P14 must expose mailer state through the planned `net` CLI commands.

Required mailer-related commands:

- `net poll <link-name>`
- `net poll --all`
- `net poll --dry-run <link-name>`
- `net status [network]`
- `net queue <link-name>`
- `net links list`
- `net links show <link-name>`
- `net packets inbound`
- `net packets outbound`
- `net packets quarantine`
- `net logs [link-name] [--limit N]`

Required status fields:

- link enabled state
- profile enabled state
- link address
- host and port
- transport security mode
- plaintext warning when applicable
- last poll result
- next scheduled poll
- active session state
- outbound ready count
- inbound pending count
- quarantined count
- bytes in and out
- packets in and out
- last error summary

The TUI may display the same data later, but the CLI is the required P14
operational surface.

## Testing Requirements

Implementation must include tests for each layer.

### File-Only Tests

- Copy known-good `.pkt` into `inbound/drop`, run tosser, verify local messages.
- Run scanner on local echomail, verify outbound ready file and
  `network_packets` row.
- Drop malformed packet, verify quarantine.
- Drop wrong-password packet, verify quarantine.

### BinkP Protocol Tests

- Parse command frame header and payload.
- Parse data frame header and payload.
- Reject frame larger than 32767 bytes.
- Authenticate with correct password.
- Reject wrong password.
- Transfer one file from client to server.
- Transfer one file from server to client.
- Complete empty poll.
- Handle `M_SKIP`.
- Parse and explicitly handle or reject `M_GET`.
- Complete `M_EOB` from both sides.

### Runtime Tests

- Outbound poll sends ready files and marks them processed only after
  acknowledgement.
- Failed connection keeps files ready.
- Interrupted transfer does not create pending inbound packet row.
- Inbound session commits completed file and creates inbound packet row.
- Per-link session guard rejects concurrent sessions for the same link.
- Two different links may poll concurrently up to the global limit.
- Plaintext legacy mode emits startup and poll warnings.
- TLS required succeeds with valid certificates and fails with invalid
  certificates.
- Opportunistic TLS attempts TLS before allowed plaintext fallback.

### End-To-End Tests

- Two in-process OxideBBS network runtimes exchange echomail over localhost
  BinkP.
- Node A scans local message, polls Node B, Node B tosses inbound message, and
  the message appears in Node B's local area.
- Node B replies, scans, polls Node A, Node A tosses inbound reply, and the
  reply appears in Node A's local area with reply metadata preserved.
- Duplicate packet retry is detected and skipped without duplicate local posts.

## Out Of Scope For v1.2

The following are not part of the v1.2 mailer unless a later ADR supersedes
this document:

- BinkleyTerm-compatible front-end mailer UI
- FrontDoor-compatible front-end mailer UI
- FTS-0001 dial-up mailer sessions
- modem mailer negotiation
- EMSI
- WaZOO
- YooHoo/2U2
- Hydra
- Janus
- SEAlink mailer sessions
- BSO outbound queue format
- `.flo` file generation
- external mailer process supervision
- caller pass-through from a front-end mailer into OxideBBS

## Implementation Checklist

- [ ] Add `network.binkp` runtime config.
- [ ] Add or derive `network.paths` spool config.
- [ ] Create the spool directories during setup/runtime initialization.
- [ ] Implement `oxidebbs-binkp`.
- [ ] Keep `oxidebbs-binkp` independent from `oxidebbs-transfer`.
- [ ] Keep BinkP independent from FTN packet parsing.
- [ ] Implement outbound poll lifecycle.
- [ ] Implement inbound listener lifecycle.
- [ ] Implement per-link session guard.
- [ ] Implement scheduled polling.
- [ ] Implement poll logging.
- [ ] Implement plaintext legacy warnings.
- [ ] Implement file-only inbound and outbound workflows.
- [ ] Document external-mailer directory drop mode.
- [ ] Add CLI status and queue views.
- [ ] Add sysop docs in `docs/ftn/architecture.md` and `docs/ftn/binkp.md`.
- [ ] Pass `./scripts/dev-check.sh`.
