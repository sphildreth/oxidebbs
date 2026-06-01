# OxideNet PRD

# Product Requirements Document: OxideNet

## Document status

Draft PRD for planning and implementation.

## Related projects

- **OxideBBS** — the Rust-based BBS software.
- **OxideNet** — a future FTN/FidoNet-style message network built around OxideBBS.
- **Blackboard BBS** — the initial home BBS, registrar, hub, and onboarding board for OxideNet.

## Summary

OxideNet is a modern, BBS-native, FTN-style message network intended to connect OxideBBS systems through store-and-forward message exchange.

The core product idea is intentionally retro:

> A sysop connects to the home BBS, applies from inside the BBS, receives their OxideNet address and configuration, imports that configuration into their own OxideBBS instance, and then messages begin flowing through FTN-style echomail and netmail.

OxideNet should feel like a classic message network, but with fewer sharp edges. The signup and onboarding flow should happen through a BBS-native module rather than a web-first form. The network should use modern automation to generate configuration, credentials, message-area mappings, nodelists, and welcome messages.

The first OxideNet hub will be **Blackboard BBS**, running OxideBBS. Blackboard BBS acts as:

- The home board
- The primary hub
- The registrar
- The policy authority
- The first nodelist generator
- The first onboarding experience
- The first network-admin console

OxideNet should be implemented as an extension/profile on top of generic FTN support in OxideBBS, not hard-coded directly into the core BBS runtime.

## Product positioning

OxideNet is not intended to be a centralized web forum.

OxideNet should be a real message network with:

- Local BBS message bases
- Store-and-forward message flow
- Echomail-style public areas
- Netmail-style private messages
- Node addresses
- Network policy
- Area subscriptions
- A hub/polling model
- A nodelist-like directory
- Packet import/export
- Future BinkP-compatible transport

The user experience should be easier than classic FTN, but the spirit should remain recognizable.

## Non-goals

OxideNet is not:

- A web forum
- A Discord replacement
- A Mastodon/ActivityPub clone
- A centralized database shared by all BBSes
- A real FidoNet zone
- A replacement for existing FTN networks such as FidoNet or fsxNet
- A requirement for running OxideBBS
- A v1 requirement for OxideBBS itself

OxideBBS should be useful without OxideNet.

OxideNet should be a later milestone built on top of stable OxideBBS primitives.

## Core idea

The primary onboarding experience:

```text
1. A sysop installs OxideBBS.
2. The sysop connects to Blackboard BBS over telnet.
3. The sysop enters the OxideNet application module.
4. The sysop applies to join OxideNet.
5. Blackboard BBS stores the application.
6. The OxideNet admin reviews the application.
7. If approved, the system assigns an OxideNet address.
8. Blackboard BBS generates an onboarding/config package.
9. The sysop imports the package into their own OxideBBS.
10. Their BBS polls the hub.
11. Welcome netmail and echomail begin flowing.
```

This is intentionally more charming than a normal web signup flow.

The signup process itself becomes part of the BBS culture.

## Recommended architecture

OxideNet should be built using layered crates/modules.

```text
oxidebbs-ftn
    Generic FTN-style message model and packet handling.

oxidebbs-binkp
    Future BinkP-compatible transport/poller/listener.

oxidebbs-oxidenet
    First-party OxideNet profile: signup, policy, config generation,
    nodelist generation, default area set, and admin workflow.
```

The purpose of this split is to keep OxideBBS flexible.

OxideBBS should eventually be able to support:

- OxideNet
- FidoNet
- fsxNet
- Other FTN-style networks
- Private sysop-created message networks

OxideNet should be an opinionated network profile, not the only possible network.

## Crate responsibilities

## `oxidebbs-ftn`

Generic FTN-style networking primitives.

Responsibilities:

- FTN address model
- Network identity model
- Echomail message model
- Netmail message model
- Message metadata preservation
- Packet import boundary
- Packet export boundary
- Duplicate detection
- Area mapping
- Seen-by/path metadata model
- Inbound queue
- Outbound queue
- Local-message-to-network-message conversion
- Network-message-to-local-message conversion
- Basic validation rules

This crate should not know anything about Blackboard BBS specifically.

It should not hard-code OxideNet branding.

It should provide the general FTN engine OxideNet uses.

## `oxidebbs-binkp`

Future BinkP-compatible mail transport.

Responsibilities:

- Outbound polling
- Inbound mailer listener
- Session authentication
- File transfer for packets/bundles
- Retry/backoff
- Connection logging
- Last-poll state
- Failed-poll state
- Per-node transport credentials
- Mail queue send/receive lifecycle

This can be delayed until after the FTN data model and signup workflow are designed.

Early development can use local filesystem-based inbound/outbound directories for testing.

## `oxidebbs-oxidenet`

The OxideNet-specific product module.

Responsibilities:

- OxideNet application form
- Application state machine
- Network policy acceptance
- Manual approval workflow
- Address assignment
- Node registry
- Config package generation
- Area subscription defaults
- Welcome netmail generation
- Default echomail area definitions
- Nodelist generation
- Hub dashboard
- Network admin commands
- Applicant status screen
- Member self-service commands

This crate can depend on `oxidebbs-ftn`.

It should not contain generic packet parsing logic that belongs in `oxidebbs-ftn`.

## Addressing model

OxideNet should use FTN-style addressing:

```text
Zone:Net/Node.Point
```

OxideNet should use **42** as the default zone.

Rationale:

- 42 is culturally familiar as “the meaning of life.”
- It is playful without religious baggage.
- It is short and memorable.
- It does not imply membership in real FidoNet.
- It gives OxideNet a distinct identity.

Initial addressing recommendation:

```text
42:1/1       Blackboard BBS, primary hub and registrar
42:1/2       Reserved backup hub
42:1/10-99   Reserved infrastructure / future hubs
42:1/100+    Member BBSes
42:1/900+    Test/lab nodes
```

Possible future expansion:

```text
42:2/*       Future second net
42:3/*       Future third net
42:100/*     Experimental/private area
```

Early implementation can start even simpler:

```text
42:1/1       Blackboard BBS
42:1/100     First member BBS
42:1/101     Second member BBS
```

## Address assignment rules

Initial assignment should be manual or semi-automatic.

Recommended rules:

1. Blackboard BBS is always `42:1/1`.
2. `42:1/2` is reserved for a future backup hub.
3. `42:1/10-99` are reserved for infrastructure/hub roles.
4. Member boards start at `42:1/100`.
5. Addresses are never reused immediately after retirement.
6. Retired addresses remain in a tombstone/retired-node list.
7. Test nodes use a reserved range such as `42:1/900+`.

## Home BBS concept

The “home BBS” is the entry point for joining OxideNet.

Initial home BBS:

```text
Blackboard BBS
Address: 42:1/1
Role: Primary hub, registrar, policy authority
Software: OxideBBS
```

The home BBS should expose a BBS-native OxideNet area.

Main menu example:

```text
[O] OxideNet
```

OxideNet menu:

```text
╔══════════════════════════════════════╗
║              OxideNet                ║
║      Message Network Services        ║
╚══════════════════════════════════════╝

[A] Apply for OxideNet
[S] Check Application Status
[P] Read Network Policy
[N] View Nodelist
[E] View Echomail Areas
[C] Download Config Pack
[T] Test Poll / Connectivity
[H] Help Setting Up OxideBBS
[Q] Return to Main Menu
```

For approved members:

```text
[M] Manage My Node
[R] Rotate BinkP Password
[D] Download Latest Config
[L] View Last Poll Logs
[Q] Request Area Subscription
[X] Retire My Node
```

For network administrators:

```text
[A] Pending Applications
[V] View Application
[P] Approve Application
[R] Reject Application
[I] Request More Information
[N] Assign / Edit Node Address
[S] Suspend Node
[G] Generate Nodelist
[B] Broadcast Network Notice
[L] View Poll Failures
```

## Signup flow

## Phase 1: Applicant connects

The sysop connects to Blackboard BBS over telnet.

They may be:

- A new caller
- An existing caller
- A known sysop
- A returning applicant

They enter the OxideNet module from the BBS menu.

## Phase 2: Application intro

The module explains:

- What OxideNet is
- What kind of systems are welcome
- That the network is human-approved
- That the sysop must run a reachable BBS
- That network policy applies
- That spam/abuse/malicious systems are not allowed
- That the network is experimental/early if applicable

## Phase 3: Application form

Fields:

```text
Board name
Sysop alias
Sysop real name, optional depending on policy
Sysop contact email
BBS hostname or IP
BinkP port, future
Telnet hostname/port, optional public listing
BBS software
OxideBBS version
Operating system
Time zone
City/region, optional
Short board description
Expected topics / community
Why do you want to join OxideNet?
Will you accept the network policy?
```

For OxideBBS-based systems, the module can eventually accept a machine-generated “capability block” from the applicant’s BBS:

```text
software = "OxideBBS"
version = "0.4.0"
supports = ["echomail", "netmail", "binkp", "areafix"]
host = "mybbs.example.net"
binkp_port = 24554
telnet_port = 2323
```

## Phase 4: Validation

The application module should validate:

- Required fields present
- Board name not already taken
- Sysop account not already linked to an active node
- Hostname format valid
- Port numbers valid
- Policy accepted
- Software/version recorded
- Contact method provided

Future validation:

- Test DNS resolution
- Test BinkP reachability
- Test telnet reachability
- Verify OxideBBS version/capabilities
- Confirm inbound mailer authentication

## Phase 5: Submit application

Application state becomes:

```text
submitted
```

Applicant receives:

- Application ID
- Current status
- Next steps
- Expected manual review note
- How to check status

Example:

```text
Your OxideNet application has been submitted.

Application ID: OXNET-2026-0007
Status: Pending Review

Return to Blackboard BBS and choose:
OxideNet -> Check Application Status
```

## Phase 6: Review

Network admin reviews the application from Blackboard BBS.

Admin can choose:

```text
approve
reject
needs-info
hold
```

If more information is needed, the applicant sees the request when they check status.

## Phase 7: Approval and node assignment

On approval:

1. Assign address.
2. Create node record.
3. Generate network credentials.
4. Generate config pack.
5. Create default subscriptions.
6. Generate welcome netmail.
7. Add node to nodelist.
8. Mark application approved.

Example assignment:

```text
Board: Retro Cavern BBS
Sysop: Night Owl
Address: 42:1/100
Hub: Blackboard BBS, 42:1/1
```

## Phase 8: Config package

The applicant receives a config package.

For OxideBBS, this can be a downloadable/importable bundle:

```text
oxidenet-42-1-100.zip
  oxidenet.toml
  areas.toml
  policy.txt
  nodelist.txt
  credentials.toml
  README.md
```

Alternative token-based import:

```bash
oxidebbs net join --invite OXNET-ABCD-1234-EFGH
```

The invite token could fetch or import the generated package.

For v1 of OxideNet, a static generated config file is enough. Token-based join can come later.

## Phase 9: First poll

The member sysop installs/imports the config.

Their BBS polls Blackboard BBS.

First successful poll should:

- Authenticate
- Receive welcome netmail
- Receive default echomail area packets
- Record last successful poll
- Confirm active network status

## Phase 10: Activation

After first successful poll, node state becomes:

```text
active
```

The member appears in the active nodelist.

## Application lifecycle

Recommended states:

```text
draft
submitted
needs-info
approved
config-generated
first-poll-pending
active
probation
suspended
retired
rejected
withdrawn
```

State meanings:

| State | Meaning |
|---|---|
| draft | Applicant started but has not submitted |
| submitted | Application awaiting review |
| needs-info | Admin asked applicant for more detail |
| approved | Admin approved application |
| config-generated | Address/credentials/config have been generated |
| first-poll-pending | Waiting for the applicant BBS to poll successfully |
| active | Node is active and participating |
| probation | Node is active but under watch |
| suspended | Node may not send/receive network traffic |
| retired | Node voluntarily or administratively retired |
| rejected | Application was rejected |
| withdrawn | Applicant withdrew |

## Network member roles

## Applicant

A sysop who has started or submitted an application.

## Member sysop

An approved sysop running a node.

## Hub sysop

A sysop running a hub that routes network traffic.

Initially, only Blackboard BBS is a hub.

## Network admin

A user with permission to approve/reject applications, assign addresses, suspend nodes, manage areas, and generate nodelists.

## Policy authority

Initially the same as the network admin.

Later, this could become a small group.

## Area moderator

A user responsible for one or more echomail areas.

## Message model

OxideNet should support two primary message types.

## Echomail

Public messages distributed to all subscribed nodes for a given area.

Examples:

```text
OXIDE.GENERAL
OXIDE.SYSOP
OXIDE.DOORS
OXIDE.ANSI
OXIDE.DECENTDB
OXIDE.DEVELOPMENT
```

## Netmail

Private node-to-node mail.

Example:

```text
From: 42:1/100
To:   42:1/1
Subject: Poll test
```

## Message metadata

Imported messages should preserve network metadata.

Required metadata:

```text
network_key
origin_address
origin_board
origin_sysop
area_tag
message_id
reply_to_id
created_at
imported_at
path
seen_by
duplicate_hash
packet_id
```

The local OxideBBS message base should be able to show user-friendly message content while preserving enough metadata to debug routing and duplicates.

## Default echomail areas

Initial recommended areas:

```text
OXIDE.GENERAL      General discussion
OXIDE.SYSOP        Sysop-only operations and support
OXIDE.DOORS        Door games and compatibility
OXIDE.ANSI         ANSI art, screens, themes
OXIDE.DEV          OxideBBS development
OXIDE.DECENTDB     DecentDB usage and feedback
OXIDE.NETWORK      OxideNet operations and announcements
OXIDE.TEST         Test messages
```

Recommended defaults for new nodes:

```text
Subscribe by default:
  OXIDE.GENERAL
  OXIDE.SYSOP
  OXIDE.NETWORK
  OXIDE.TEST

Optional:
  OXIDE.DOORS
  OXIDE.ANSI
  OXIDE.DEV
  OXIDE.DECENTDB
```

## Area subscription management

At first, area subscriptions can be admin-managed.

Future:

- AreaFix-like commands via netmail
- BBS-native subscription screen from Blackboard BBS
- Self-service subscription changes for approved nodes

Recommended first implementation:

```text
Network admin assigns default area set during approval.
Member can request additional areas from Blackboard BBS.
Admin approves/denies.
```

## Store-and-forward flow

OxideNet should use a store-and-forward model.

Basic hub-and-spoke flow:

```text
Member BBS
  scans local messages
  creates outbound packets
  polls Blackboard hub
  uploads outbound packets
  downloads inbound packets
  tosses inbound packets into local areas

Blackboard BBS
  receives member packets
  validates node credentials
  imports/tosses packets
  scans outbound for subscribed nodes
  generates packets for other nodes
  makes them available for polling
```

## Topology

Initial topology should be hub-and-spoke.

```text
42:1/1 Blackboard BBS
  ├── 42:1/100 Member BBS
  ├── 42:1/101 Member BBS
  └── 42:1/102 Member BBS
```

Do not begin with full mesh routing.

Reasons:

- Simpler debugging
- Simpler policy enforcement
- Simpler duplicate detection
- Simpler moderation
- Easier member onboarding
- Easier initial nodelist generation
- Easier hub backups

Future topology:

```text
42:1/1 Primary hub
42:1/2 Backup hub
42:2/1 Regional or thematic hub
```

## Nodelist

OxideNet should generate a nodelist-like directory.

Required fields:

```text
address
board_name
sysop_alias
host
binkp_port
telnet_host_optional
telnet_port_optional
software
version
status
last_seen
flags
```

Example logical entry:

```text
42:1/100 Retro Cavern BBS NightOwl retrocavern.example.net 24554 OxideBBS 0.4.0 active
```

The first implementation can use a simple machine-readable format.

Recommended files:

```text
nodelist.toml
nodelist.json
nodelist.txt
```

A traditional FTN-style nodelist can come later if desired.

## Config package contents

An approved sysop should receive:

```text
oxidenet.toml
areas.toml
nodelist.toml
policy.txt
README.md
credentials.toml
```

Example `oxidenet.toml`:

```toml
[network]
key = "oxidenet"
name = "OxideNet"
address = "42:1/100"
hub_address = "42:1/1"

[network.local]
board_name = "Retro Cavern BBS"
sysop_alias = "Night Owl"

[network.hub]
name = "Blackboard BBS"
host = "blackboard.example.net"
binkp_port = 24554
poll_interval_minutes = 60

[network.auth]
session_password = "generated-secret"

[network.policy]
accepted_policy_version = "1.0"
accepted_at = "2026-06-01T00:00:00Z"
```

Example `areas.toml`:

```toml
[[areas]]
tag = "OXIDE.GENERAL"
local_key = "ox-general"
name = "OxideNet General"
subscribed = true

[[areas]]
tag = "OXIDE.SYSOP"
local_key = "ox-sysop"
name = "OxideNet Sysop"
subscribed = true

[[areas]]
tag = "OXIDE.TEST"
local_key = "ox-test"
name = "OxideNet Test"
subscribed = true
```

## CLI commands for member BBSes

OxideBBS should provide local commands for managing network membership.

Recommended commands:

```bash
oxidebbs net list
oxidebbs net show oxidenet
oxidebbs net join --config oxidenet.toml
oxidebbs net join --invite OXNET-ABCD-1234
oxidebbs net check oxidenet
oxidebbs net poll oxidenet
oxidebbs net poll oxidenet --dry-run
oxidebbs net areas oxidenet
oxidebbs net status oxidenet
oxidebbs net queue oxidenet
oxidebbs net toss oxidenet
oxidebbs net scan oxidenet
```

Early implementation can start with:

```bash
oxidebbs net join --config oxidenet.toml
oxidebbs net check oxidenet
oxidebbs net poll oxidenet --dry-run
oxidebbs net status oxidenet
```

## CLI commands for home BBS / hub

Blackboard BBS needs additional commands.

```bash
oxidebbs oxidenet applications list
oxidebbs oxidenet applications show <application-id>
oxidebbs oxidenet applications approve <application-id>
oxidebbs oxidenet applications reject <application-id>
oxidebbs oxidenet applications needs-info <application-id>

oxidebbs oxidenet nodes list
oxidebbs oxidenet nodes show <address>
oxidebbs oxidenet nodes suspend <address>
oxidebbs oxidenet nodes retire <address>
oxidebbs oxidenet nodes rotate-password <address>

oxidebbs oxidenet nodelist generate
oxidebbs oxidenet nodelist publish

oxidebbs oxidenet areas list
oxidebbs oxidenet areas add <tag>
oxidebbs oxidenet areas subscribe <address> <tag>
oxidebbs oxidenet areas unsubscribe <address> <tag>

oxidebbs oxidenet packets inbound
oxidebbs oxidenet packets outbound
oxidebbs oxidenet packets quarantine
```

## BBS-native admin screens

Blackboard BBS should expose OxideNet administration inside the BBS.

Admin menu:

```text
╔══════════════════════════════════════╗
║          OxideNet Admin              ║
╚══════════════════════════════════════╝

[A] Applications
[N] Nodes
[E] Echomail Areas
[P] Packets / Queues
[L] Poll Logs
[G] Generate Nodelist
[B] Broadcast Notice
[S] Suspensions
[Q] Quit
```

Application review screen:

```text
Application: OXNET-2026-0007
Board:       Retro Cavern BBS
Sysop:       Night Owl
Host:        retrocavern.example.net
Software:    OxideBBS 0.4.0
Status:      Submitted

[A] Approve
[R] Reject
[I] Request Info
[H] Hold
[Q] Back
```

## BBS-native applicant screens

Applicant status screen:

```text
╔══════════════════════════════════════╗
║       OxideNet Application Status    ║
╚══════════════════════════════════════╝

Application ID: OXNET-2026-0007
Board:          Retro Cavern BBS
Status:         Pending Review

Next step:
A network admin will review your application.
Return later and choose OxideNet -> Status.
```

Approved screen:

```text
Congratulations! Your BBS has been approved for OxideNet.

Assigned address: 42:1/100
Hub:              Blackboard BBS, 42:1/1

[D] Download Config Pack
[I] View Import Instructions
[T] Test First Poll
```

## Security and abuse prevention

OxideNet should assume that open signups will eventually attract junk.

Required controls:

- Manual application approval
- Unique board names
- Unique node addresses
- Policy acceptance
- Per-node session password
- Ability to suspend a node
- Ability to quarantine malformed packets
- Duplicate message detection
- Message size limits
- Packet size limits
- Poll rate limits
- Audit logs
- Last-seen tracking
- Admin broadcast area
- Node retirement process

Strongly recommended later:

- Reachability checks
- Hostname verification
- Version/capability checks
- Admin notes per node
- Moderation flags
- Per-area posting permissions
- Per-node write throttling
- Spam/loop detection
- Packet signature or stronger authentication model

## Policy requirements

OxideNet should have a short network policy.

The policy should cover:

- No spam
- No malicious content
- No illegal content
- No harassment
- No deliberate message loops
- No impersonation
- No unauthorized gateways
- Sysop responsibility for their users
- Moderation and suspension process
- Network experimental status
- Policy versioning
- How to retire a node

Each applicant must accept a specific policy version.

Record:

```text
policy_version
accepted_by_user_id
accepted_at
application_id
node_address
```

## Observability

Hub logs should capture:

- Application submitted
- Application approved/rejected
- Node assigned
- Config package generated
- Node first poll
- Poll success
- Poll failure
- Packet received
- Packet tossed
- Packet quarantined
- Duplicate detected
- Message exported
- Node suspended
- Node retired
- Nodelist generated

Member logs should capture:

- Network config imported
- Poll started
- Poll authenticated
- Inbound packets received
- Outbound packets sent
- Toss started
- Toss completed
- Scan started
- Scan completed
- Duplicate detected
- Packet quarantined

## Data model sketch

## `network_applications`

```text
id
created_at
updated_at
submitted_at
reviewed_at
status
applicant_user_id
board_name
sysop_alias
contact_email
host
binkp_port
telnet_host
telnet_port
software
software_version
timezone
region
description
reason
policy_version
policy_accepted_at
admin_notes
reviewed_by_user_id
assigned_address
```

## `network_nodes`

```text
id
network_key
address
zone
net
node
point
board_name
sysop_alias
contact_email
host
binkp_port
telnet_host
telnet_port
software
software_version
status
created_at
activated_at
suspended_at
retired_at
last_poll_at
last_successful_poll_at
flags
```

## `network_credentials`

```text
id
node_id
credential_type
secret_hash
created_at
rotated_at
expires_at
status
```

Do not store plain-text secrets after config generation unless deliberately required. Prefer storing hashes and allowing rotation/regeneration.

## `network_areas`

```text
id
network_key
tag
name
description
status
default_subscribe
sysop_only
moderated
created_at
```

## `network_area_subscriptions`

```text
id
node_id
area_id
status
subscribed_at
unsubscribed_at
```

## `network_messages`

```text
id
network_key
message_type
area_tag
origin_address
destination_address
from_name
to_name
subject
body
created_at
imported_at
message_id
reply_to_id
duplicate_hash
path
seen_by
packet_id
status
```

## `network_packets`

```text
id
network_key
direction
node_id
packet_id
filename
sha256
size_bytes
received_at
processed_at
status
error_message
```

## `network_poll_logs`

```text
id
network_key
node_id
started_at
ended_at
direction
status
bytes_in
bytes_out
packets_in
packets_out
error_message
```

## Duplicate detection

OxideNet needs duplicate detection early.

Minimum duplicate hash input:

```text
network_key
area_tag
origin_address
message_id
created_at
subject
body_hash
```

Better if FTN-compatible message IDs are available:

```text
area_tag + origin_address + ftn_msgid
```

On duplicate:

- Do not import the message twice.
- Record duplicate event.
- Optionally count duplicates for loop detection.

## Packet quarantine

Packets should be quarantined if:

- Sender is unknown
- Sender is suspended
- Authentication fails
- Packet is malformed
- Packet exceeds size limits
- Packet contains messages for unknown areas
- Packet creates suspicious loop behavior
- Packet cannot be decoded safely

Quarantined packets should be visible to the network admin.

## Hub-and-spoke v1 design

The first working version should avoid routing complexity.

Initial rule:

```text
All member nodes poll 42:1/1.
42:1/1 distributes all echomail.
```

Do not support:

- Node-to-node mesh routing
- Regional routing
- Multiple hubs
- Complex routing tables
- Automatic hub promotion

Those can wait.

## Implementation phases

## Phase 0: Design foundation

Deliverables:

- OxideNet PRD
- FTN architecture ADR
- Addressing ADR using zone 42
- OxideNet policy draft
- Data model sketch
- Config package format

## Phase 1: Local FTN data model

Deliverables:

- `oxidebbs-ftn` crate
- FTN address parser/formatter
- Network message model
- Area mapping model
- Duplicate hash strategy
- Unit tests

Example test cases:

```text
42:1/1
42:1/100
42:1/100.1
invalid zone
invalid net
invalid node
```

## Phase 2: Home BBS application module

Deliverables:

- `oxidebbs-oxidenet` crate
- Application form
- Application status screen
- Admin review screen
- Application lifecycle
- Manual approval
- Address assignment
- Config package generation

This phase can work before actual packet exchange exists.

## Phase 3: Local import/export simulation

Deliverables:

- Filesystem inbound/outbound queues
- Packet-like JSON or TOML test format
- Scan local messages to outbound
- Toss inbound to local message areas
- Duplicate detection
- Quarantine

This lets the system work before BinkP is implemented.

## Phase 4: First hub/member flow

Deliverables:

- Blackboard BBS as hub
- One test member BBS
- Manual config import
- Poll simulation or filesystem exchange
- Welcome netmail
- Default echomail areas
- Nodelist generation

Goal:

```text
A test member can apply, be approved, import config, and exchange test messages.
```

## Phase 5: BinkP-compatible transport

Deliverables:

- `oxidebbs-binkp` crate
- Outbound poller
- Hub listener
- Authentication
- Packet transfer
- Retry/backoff
- Poll logs

## Phase 6: Operational hardening

Deliverables:

- Packet quarantine UI
- Poll failure dashboard
- Node suspension
- Password rotation
- Area subscription requests
- Policy version updates
- Backup/restore notes

## Phase 7: Public experimental OxideNet

Deliverables:

- Public signup through Blackboard BBS
- Public docs
- First real member nodes
- Network policy v1.0
- Published nodelist
- Network announcements area
- Sysop support area

## Acceptance criteria

OxideNet MVP is successful when:

1. Blackboard BBS can accept an OxideNet application.
2. A network admin can approve the application.
3. The system assigns a `42:1/N` node address.
4. The system generates an OxideBBS-compatible config package.
5. A member OxideBBS can import the config.
6. A member can poll the hub.
7. Welcome netmail is delivered.
8. At least one echomail area flows both directions.
9. Duplicate messages are not imported twice.
10. Suspended nodes cannot exchange mail.
11. Poll attempts are logged.
12. The nodelist can be generated.
13. The entire flow works without using a web signup form.

## Recommended initial defaults

```text
Network name:       OxideNet
Zone:               42
Primary hub:        42:1/1
Primary hub board:  Blackboard BBS
Initial areas:
  OXIDE.GENERAL
  OXIDE.SYSOP
  OXIDE.NETWORK
  OXIDE.TEST
Default transport:  filesystem simulation first, BinkP later
Signup method:      BBS-native application module
Approval:           manual
Topology:           hub-and-spoke
```

## Open questions

1. Should OxideNet allow non-OxideBBS systems eventually?
2. Should OxideNet support traditional FTN packet formats from day one, or start with an internal packet format?
3. Should `42:1/100+` be the member range, or should members start at `42:1/10`?
4. Should sysop real names be required, optional, or forbidden?
5. Should the nodelist be public?
6. Should telnet addresses be listed publicly?
7. Should there be an application fee? Current recommendation: no.
8. Should OxideNet include a private sysop-only area from day one? Current recommendation: yes.
9. Should the first version support netmail, or only echomail? Current recommendation: include netmail foundation, but make echomail the first visible feature.
10. Should OxideNet eventually bridge to FidoNet/fsxNet? Current recommendation: not until the native network is stable.

## Strong recommendations

1. Use **42** as the OxideNet zone.
2. Make Blackboard BBS `42:1/1`.
3. Keep signup BBS-native.
4. Make approval manual.
5. Start hub-and-spoke.
6. Build generic FTN support separately from the OxideNet profile.
7. Do not make OxideNet required for OxideBBS.
8. Do not start with a web signup form.
9. Generate config packages automatically.
10. Use DecentDB for all local OxideBBS/OxideNet state.
11. Preserve enough metadata to debug routing and duplicates.
12. Treat packet quarantine and suspension as v1 requirements for the network.
13. Make the first “magic moment” be simple: apply, approve, import config, poll, receive welcome netmail.

## Example user story: applying sysop

As a sysop running OxideBBS, I want to connect to Blackboard BBS and apply for OxideNet from inside the BBS, so that I can join the network without manually emailing a coordinator or filling out a web form.

Acceptance criteria:

- I can choose OxideNet from the Blackboard BBS menu.
- I can read the network policy.
- I can submit board and sysop details.
- I receive an application ID.
- I can return later and check status.
- If approved, I can download or copy my config package.
- I can import the config into my BBS.

## Example user story: network admin

As the OxideNet admin, I want to review applications from inside Blackboard BBS, so that I can preserve a classic sysop workflow and control network quality.

Acceptance criteria:

- I can view pending applications.
- I can inspect all submitted details.
- I can approve, reject, or request more information.
- Approval assigns an address.
- Approval generates credentials.
- Approval generates a config package.
- The approved node appears in the nodelist.

## Example user story: first poll

As an approved member sysop, I want my BBS to poll Blackboard BBS and receive welcome messages, so that I know my OxideNet setup works.

Acceptance criteria:

- My BBS authenticates to the hub.
- My BBS receives welcome netmail.
- My BBS receives the default test echomail area.
- Poll status is recorded locally and on the hub.
- Errors are understandable.

## Example user story: suspend node

As the network admin, I want to suspend a node, so that I can stop abuse or broken routing before it affects the rest of the network.

Acceptance criteria:

- I can mark a node suspended.
- Suspended nodes cannot upload new packets.
- Suspended nodes stop receiving new outbound packets.
- Suspension is logged.
- The node status appears in admin views.

## Documentation deliverables

OxideNet should eventually include:

```text
docs/oxidenet/PRD.md
docs/oxidenet/POLICY.md
docs/oxidenet/SETUP_MEMBER.md
docs/oxidenet/HUB_ADMIN.md
docs/oxidenet/ADDRESSING.md
docs/oxidenet/AREAS.md
docs/oxidenet/CONFIG_PACKAGE.md
docs/oxidenet/TROUBLESHOOTING.md
```

## Final product vision

OxideNet should feel like a newly born retro message network that somehow got the benefit of modern software engineering.

The ideal story:

```text
Install OxideBBS.
Call Blackboard BBS.
Apply for OxideNet.
Get your 42:1/N address.
Import your config.
Poll the hub.
Read your welcome netmail.
Post in OXIDE.GENERAL.
Messages flow.
```

That is the heart of OxideNet.
