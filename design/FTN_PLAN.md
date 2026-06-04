# FTN Networking Implementation Plan

## Document status

Implementation plan for FTN (FidoNet Technology Network) support in OxideBBS.

## Related documents

- `design/FTN_NETWORKING.md` — early design notes
- `design/MAILER.md` — built-in BinkP mailer, tosser/scanner boundaries, and
  external-mailer directory drop mode
- `design/OXIDENET_PRD.md` — OxideNet-specific product requirements
- `design/adr/0009-ftn-abstraction.md` — ADR: keep FTN packet I/O behind a network boundary
- `design/SPEC.md` — section 13: FTN/OxideNet boundary
- `design/PRD.md` — section 5: FTN-style networking foundation
- `design/DECENTDB_SCHEMA.md` — current and planned DecentDB tables
- `design/ROADMAP.md` — milestone 8 (complete) and future items

## Purpose

This document describes everything needed for OxideBBS to participate in real FTN networks — FidoNet, fsxNet, RetroNet, or any other zone. It covers the shared network model, FTN packet adapter, tosser, scanner, packet format, bundle handling, nodelist processing, netmail routing, AreaFix, and BinkP transport.

This is separate from the OxideNet PRD. OxideNet uses an internal binary packet format and is a self-contained network. This plan enables OxideBBS to interoperate with traditional FTN networks that use the standard `.pkt` format, arcmail bundles, and nodelist files. Both packet formats share the same protocol-neutral network model so queueing, duplicate detection, local-message conversion, and area mapping do not need separate implementations.

The `oxidebbs-network` crate provides the generic network model. The `oxidebbs-ftn` crate is the legacy FTN adapter and engine. OxideNet or any other network profile builds on the shared model and uses its own packet adapter.

## Current state

What exists today:

- `oxidebbs-network` — protocol-neutral `FtnAddress`, `EchoMailAreaMapping`, `NetMailMessage`, `DuplicateDetectionKey`, `PacketBoundary`, `PacketDirection`. All have serde support and unit tests.
- `oxidebbs-core/src/network.rs` — re-exports the protocol-neutral network types during the transition.
- `oxidebbs-core/src/message.rs` — `AreaKind` enum with `Local`, `EchoMail`, `NetMail` variants. `MessageArea` has `network_id: Option<String>`. `Message` has `network_message_id: Option<String>`.
- `oxidebbs-db` schema version 5 — `message_areas.kind` CHECK includes `'echomail'` and `'netmail'`; `messages` has first-class local/network/system author metadata; shared `network_*` tables and repository APIs exist for profiles, links, areas, packets, messages, duplicate logs, poll logs, area subscriptions, and nodelists.
- `oxidebbs-db::DbWriter` — bounded single-writer foundation with ordered execution, transaction rollback, queue backpressure, and shutdown drain tests.
- `oxidebbs-server/src/config.rs` — shared `[network]` config model with profiles, local addresses, links, compression, and transport security. Legacy `[ftn]` remains parseable as a deprecated compatibility alias.
- `config/oxidebbs.example.toml` — disabled `[network]` example with a legacy FTN profile and link.
- No `oxidebbs-ftn`, `oxidebbs-binkp`, or `oxidebbs-oxidenet` crates exist yet.
- No FTN packet reading, writing, tossing, scanning, or transport code exists.

What does not exist yet:

- FTN `.pkt` format parser or writer
- Echomail kludge line parser (AREA, ^MSGID, ^REPLY, INTL, FMPT, TOPT, SEEN-BY, ^PATH, ^Via, ^FLAGS, tear lines, origin lines)
- Tosser (inbound packet processing)
- Scanner (outbound message packing)
- Bundle creation or extraction
- Nodelist parser
- Duplicate detection implementation backed by DecentDB
- Seen-by and PATH propagation
- Netmail routing
- AreaFix
- BinkP client or server
- CLI commands for toss, scan, poll
- FTN adapter runtime code that consumes the shared `network_*` DecentDB tables

## FTN standards reference

OxideBBS must implement or interoperate with these standards:

| Standard | Description | Priority |
|---|---|---|
| FTS-0001 | FidoNet session protocol (original NetMail session layer) | Low (BinkP supersedes) |
| FTS-0005 | Echomail specification (AREA: tag, SEEN-BY, tear line, origin line) | Required |
| FTS-4000 | Nodelist format | Required |
| FSC-0039 | Capability word and Type-2+ packet extensions | Required |
| FSC-0048 | Extension of packets to 4D addressing (point support) | Required |
| FSC-0053 | Type-2.2 packet extension | Medium (for interop with newer systems) |
| FSC-0056 | NetMail attributes | Required |
| FSC-0068 | SEEN-BY and PATH format | Required |
| FSC-0074 | Extended SEEN-BY (including zone) | Medium |
| FSC-0087 | MSGID proposal | Required |
| FSC-0091 | REPLYID kludge | Required |
| FSC-0115 | INTL, FMPT, TOPT kludges | Required |
| FSC-0116 | Typed SEEN-BY and PATH | Medium |
| FidoNet Nodelist | Standard nodelist format with flags | Required |
| FSP-1011 / BinkP | TCP/IP mailer protocol for FTN mail exchange | Required (Phase 10) |

## Architecture

### Crate layout

```text
oxidebbs-network
    Protocol-neutral network model: FTN-style addresses, network message
    envelopes, packet boundaries, queues, duplicate detection keys,
    local-message conversion traits, area mapping traits, and shared state
    enums. It has no dependency on oxidebbs-core, oxidebbs-db, or any
    packet/transport crate.

oxidebbs-ftn
    Legacy FTN adapter and engine: standard .pkt format, echomail/netmail
    kludges, tosser, scanner, nodelist, duplicate detection implementation,
    area mapping repositories, routing, and arcmail bundles. Depends on
    oxidebbs-network.

oxidebbs-binkp
    BinkP-compatible mail transport: outbound polling, inbound listener,
    authentication, file transfer, retry/backoff.

oxidebbs-oxidenet
    OxideNet-specific profile: signup, policy, config generation,
    nodelist generation, admin workflow, and internal binary packet adapter.
    Depends on oxidebbs-network, not on legacy .pkt parsing.
```

### Dependency direction

```text
oxidebbs-server
  -> oxidebbs-core
  -> oxidebbs-network
  -> oxidebbs-ftn
  -> oxidebbs-binkp (future)
  -> oxidebbs-oxidenet (future)
  -> oxidebbs-db

oxidebbs-core
  -> oxidebbs-network (temporary re-exports for current FTN foundation types)

oxidebbs-network
  -> no OxideBBS workspace crates

oxidebbs-ftn
  -> oxidebbs-network (FtnAddress, NetworkMessage, queue and boundary types)
  -> oxidebbs-core (local message conversion only)
  -> oxidebbs-db (DecentDB tables for FTN state)

oxidebbs-binkp
  -> oxidebbs-network

oxidebbs-oxidenet
  -> oxidebbs-network
  -> oxidebbs-db
```

Lower-level crates must not depend on `oxidebbs-server`.

### Shared network types migration

The FTN-style domain types currently in `oxidebbs-core/src/network.rs` will migrate to `oxidebbs-network` once the crate exists. `oxidebbs-core` will re-export them for backward compatibility during the transition. They must not migrate to `oxidebbs-ftn`, because that would create a dependency cycle (`core -> ftn -> core`) and would force OxideNet to depend on legacy `.pkt` details. The migrated types are:

- `FtnAddress`
- `NetworkAddressError`
- `EchoMailAreaMapping`
- `NetMailMessage`
- `DuplicateDetectionKey`
- `PacketDirection`
- `PacketBoundary`

Protocol-neutral types introduced during this work live in `oxidebbs-network` from the start. Legacy packet, kludge, bundle, nodelist, and tosser/scanner implementation types live in `oxidebbs-ftn`.

### Hard constraints

1. Rust only, edition 2024.
2. DecentDB is the only database. No SQLite, Postgres, MySQL, Redis, MongoDB, or ORM.
3. No `unwrap()` or `expect()` in library code. Use `Result<T, E>` with typed errors.
4. Never hold a lock across `.await`.
5. The FTN engine must work with real `.pkt` format (Type-2 and Type-2+).
6. The tosser must handle malformed packets gracefully — quarantine, not crash.
7. The scanner must produce standards-compliant packets and bundles.
8. BinkP transport must use profile-aware transport security: TLS required by default for OxideNet and new private networks, plaintext allowed only by explicit per-link opt-in for legacy FTN interoperability.
9. OxideBBS must be able to participate in multiple FTN networks simultaneously.
10. No new crate additions without justification.

---

## Configuration model

The shared `[network]` section is the v1.2 configuration model for FTN,
OxideNet, and private packet profiles. Legacy `[ftn]` keys remain parseable as
deprecated compatibility input, but new configuration examples and generated
configs must use `[network]`.

Deprecated compatibility alias:

```toml
[ftn]
enabled = false
reserved_network_name = "OxideNet"
```

Shared multi-network form:

```toml
[network]
enabled = true

[network.profiles]

[network.profiles.fidonet]
name = "FidoNet"
enabled = true
adapter = "legacy-ftn"

[network.profiles.fidonet.local_address]
zone = 1
net = 105
node = 42
point = 0

[network.profiles.fidonet.areas]
# Subscribed areas are managed by AreaFix or manual config

[network.profiles.fsxnet]
name = "fsxNet"
enabled = true
adapter = "legacy-ftn"

[network.profiles.fsxnet.local_address]
zone = 21
net = 1
node = 100
point = 0

[network.profiles.oxidenet]
name = "OxideNet"
enabled = true
adapter = "oxidenet"

[network.profiles.oxidenet.local_address]
zone = 42
net = 1
node = 1
point = 0

[network.links]

[network.links.hub_fidonet]
network = "fidonet"
address = "1:1/0"
host = "fidonet.example.net"
binkp_port = 24554
password = "secret-here"
poll_schedule_minutes = 60
transport_security = "plaintext_legacy" # explicit legacy compatibility opt-in

[network.links.hub_fsxnet]
network = "fsxnet"
address = "21:1/100"
host = "fsxnet.example.net"
binkp_port = 24554
password = "secret-there"
poll_schedule_minutes = 120
transport_security = "plaintext_legacy" # explicit legacy compatibility opt-in

[network.paths]
inbound = "/var/lib/oxidebbs/ftn/inbound"
outbound = "/var/lib/oxidebbs/ftn/outbound"
nodelist_dir = "/var/lib/oxidebbs/ftn/nodelist"
temp_inbound = "/var/lib/oxidebbs/ftn/temp"
quarantine = "/var/lib/oxidebbs/ftn/quarantine"
log_dir = "/var/lib/oxidebbs/ftn/log"

[network.ftn.tosser]
max_message_size_bytes = 65536
max_packet_size_bytes = 1048576
quarantine_on_malformed = true
log_toss = true

[network.ftn.scanner]
scan_interval_minutes = 30
add_origin_line = true
origin_template = "OxideBBS ({address})"
```

Configuration validation must reject:
- Zone 0, net 0, or node 0 in any per-network local address
- Missing per-profile local addresses
- Duplicate profile keys or link keys
- Duplicate link addresses within the same network
- Links referencing profiles not defined in `[network.profiles]`
- `adapter = "oxidenet"` on a profile processed by `oxidebbs-ftn`
- `transport_security = "plaintext_legacy"` on OxideNet or non-legacy private profiles
- Paths that do not exist or are not writable at startup (warn, do not fail)

---

## Data model

All shared network state and legacy FTN adapter state is stored in DecentDB.
v1.2 made the final naming decision: shared protocol-neutral state uses
`network_*` tables, implemented in schema version 5 and documented in
`design/DECENTDB_SCHEMA.md`. Reserve `ftn_*` table names only for future
adapter-private FTN state that cannot be shared with OxideNet or private packet
profiles.

### Local message schema adjustment

Imported network messages must be visible in local message areas without creating synthetic local users for every remote sender. Before FTN tossing imports messages, the existing `messages` schema must be migrated to support first-class external authors:

```sql
author_user_id UUID REFERENCES users(id) ON DELETE RESTRICT
author_display_name TEXT NOT NULL DEFAULT ''
author_network_address TEXT
author_kind TEXT NOT NULL DEFAULT 'local'
```

Rules:
- Existing local messages are backfilled with `author_display_name` from the referenced user alias and `author_kind = 'local'`.
- New local posts set `author_user_id`, `author_display_name`, and `author_kind = 'local'`.
- Imported FTN/OxideNet posts set `author_user_id = NULL`, preserve the remote sender in `author_display_name`, set `author_network_address`, and set `author_kind = 'network'`.
- System-generated messages such as AreaFix replies may use `author_kind = 'system'`.
- `author_kind` is one of `local`, `network`, or `system`.

### network_profiles

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
key TEXT NOT NULL UNIQUE
name TEXT NOT NULL
adapter TEXT NOT NULL DEFAULT 'legacy-ftn'
local_zone INT NOT NULL
local_net INT NOT NULL
local_node INT NOT NULL
local_point INT NOT NULL DEFAULT 0
enabled BOOL NOT NULL DEFAULT TRUE
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
```

One row per network profile OxideBBS participates in, such as `fidonet`,
`fsxnet`, or `oxidenet`.

Constraints:
- `adapter` is `legacy-ftn` or `oxidenet`
- `local_zone`, `local_net`, and `local_node` are positive
- `local_point` is `0..65535`

### network_links

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
key TEXT NOT NULL UNIQUE
network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE
address TEXT NOT NULL
host TEXT NOT NULL
binkp_port INT NOT NULL DEFAULT 24554
password TEXT NOT NULL
poll_schedule_minutes INT NOT NULL DEFAULT 60
enabled BOOL NOT NULL DEFAULT TRUE
compression TEXT NOT NULL DEFAULT 'zip'
transport_security TEXT NOT NULL DEFAULT 'tls_required'
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
```

A link is a system we exchange mail with. Each link belongs to a network profile. One link per address within that profile.

Constraints:
- `(network_id, address)` is unique
- `binkp_port` is `1..65535`
- `poll_schedule_minutes` is positive
- `compression` is one of `none`, `zip`, `arj`
- `transport_security` is one of `tls_required`, `tls_opportunistic`, or `plaintext_legacy`

### network_areas

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE
area_tag TEXT NOT NULL
local_area_id UUID NOT NULL REFERENCES message_areas(id) ON DELETE CASCADE
description TEXT NOT NULL DEFAULT ''
read_only BOOL NOT NULL DEFAULT FALSE
subscribed BOOL NOT NULL DEFAULT TRUE
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
```

Maps an FTN echo tag to a local message area. Example: area tag `ALT.BBS`
maps to local area `general`.

Constraints:
- `(network_id, area_tag)` is unique
- `(network_id, local_area_id)` is unique

### network_messages

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE
local_message_id UUID REFERENCES messages(id) ON DELETE SET NULL
message_type TEXT NOT NULL DEFAULT 'echomail'
area_tag TEXT
origin_address TEXT NOT NULL
destination_address TEXT
from_name TEXT NOT NULL
to_name TEXT
subject TEXT NOT NULL
raw_text BLOB NOT NULL
display_body TEXT NOT NULL DEFAULT ''
msgid TEXT
replyid TEXT
created_at TIMESTAMPTZ NOT NULL
imported_at TIMESTAMPTZ
exported_at TIMESTAMPTZ
duplicate_hash TEXT
packet_id UUID REFERENCES network_packets(id) ON DELETE SET NULL
status TEXT NOT NULL DEFAULT 'imported'
```

Stores the network representation of a message alongside its local representation. `raw_text` preserves the exact byte-oriented FTN message text from the packet after the packet terminator is removed. `display_body` is decoded through the configured message encoding for local UI/search convenience and is not used when re-emitting legacy FTN messages. The `duplicate_hash` is computed from the MSGID (or a combination of fields for messages without MSGID).

Constraints:
- `message_type` is `echomail`, `netmail`, or `local`
- `status` is `imported`, `exported`, `quarantined`, or `duplicate`

Indexes:
- `(network_id, area_tag, created_at)`
- `(duplicate_hash)` where `duplicate_hash IS NOT NULL`
- `(msgid)` where `msgid IS NOT NULL`
- `(local_message_id)` where `local_message_id IS NOT NULL`

### Future network_outbound_queue

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE
link_id UUID NOT NULL REFERENCES network_links(id) ON DELETE CASCADE
local_message_id UUID REFERENCES messages(id) ON DELETE SET NULL
network_message_id UUID REFERENCES network_messages(id) ON DELETE SET NULL
packet_id UUID REFERENCES network_packets(id) ON DELETE SET NULL
message_type TEXT NOT NULL DEFAULT 'echomail'
status TEXT NOT NULL DEFAULT 'queued'
attempts INT NOT NULL DEFAULT 0
queued_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
packed_at TIMESTAMPTZ
sent_at TIMESTAMPTZ
last_error TEXT
```

One row per message/link delivery attempt. This table is not part of the P2
shared-schema foundation. Add it in the scanner/export phase if per-link
delivery state cannot be represented cleanly by `network_packets`,
`network_messages`, and `network_area_subscriptions`.

Constraints:
- `message_type` is `echomail` or `netmail`
- `status` is `queued`, `packed`, `sent`, `failed`, `held`, or `skipped`
- `(link_id, local_message_id)` is unique where `local_message_id IS NOT NULL` and the status is not terminal for duplicate prevention

Indexes:
- `(network_id, status, queued_at)`
- `(link_id, status, queued_at)`

### network_packets

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE
direction TEXT NOT NULL
link_id UUID REFERENCES network_links(id) ON DELETE SET NULL
filename TEXT NOT NULL
sha256 TEXT NOT NULL
size_bytes INT NOT NULL
received_at TIMESTAMPTZ
processed_at TIMESTAMPTZ
status TEXT NOT NULL DEFAULT 'pending'
error_message TEXT
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
```

Inbound and outbound packet tracking. Each packet file is recorded before processing.

Constraints:
- `direction` is `inbound` or `outbound`
- `status` is `pending`, `processing`, `processed`, `quarantined`, or `failed`

Indexes:
- `(network_id, direction, status)`
- `(link_id, direction, status)`

### network_seen_by

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
message_id UUID NOT NULL REFERENCES network_messages(id) ON DELETE CASCADE
network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE
zone INT NOT NULL
net INT NOT NULL
node INT NOT NULL
```

Normalized SEEN-BY entries for a message. Used for loop detection during scanning.

Index:
- `(message_id)`
- `(network_id, zone, net, node, message_id)` unique

### network_path

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
message_id UUID NOT NULL REFERENCES network_messages(id) ON DELETE CASCADE
network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE
sequence INT NOT NULL
zone INT NOT NULL
net INT NOT NULL
node INT NOT NULL
```

Normalized PATH entries for a message, preserving order via `sequence`.

Index:
- `(message_id, sequence)` unique

### network_duplicate_log

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE
duplicate_hash TEXT NOT NULL
msgid TEXT
area_tag TEXT
origin_address TEXT NOT NULL
detected_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
action TEXT NOT NULL DEFAULT 'rejected'
```

Records every duplicate detection event. Used for debugging and loop detection.

Constraints:
- `action` is `rejected`, `quarantined`, or `replaced`

Index:
- `(duplicate_hash)`
- `(network_id, detected_at)`

### network_poll_log

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
link_id UUID NOT NULL REFERENCES network_links(id) ON DELETE CASCADE
started_at TIMESTAMPTZ NOT NULL
ended_at TIMESTAMPTZ
direction TEXT NOT NULL
status TEXT NOT NULL DEFAULT 'started'
bytes_in INT NOT NULL DEFAULT 0
bytes_out INT NOT NULL DEFAULT 0
packets_in INT NOT NULL DEFAULT 0
packets_out INT NOT NULL DEFAULT 0
error_message TEXT
```

Every poll attempt is logged.

Constraints:
- `direction` is `inbound`, `outbound`, or `bidirectional`
- `status` is `started`, `success`, `failed`, or `timeout`

Index:
- `(link_id, started_at)`

### network_area_subscriptions

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
area_id UUID NOT NULL REFERENCES network_areas(id) ON DELETE CASCADE
link_id UUID NOT NULL REFERENCES network_links(id) ON DELETE CASCADE
subscribed BOOL NOT NULL DEFAULT TRUE
subscribed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
unsubscribed_at TIMESTAMPTZ
source TEXT NOT NULL DEFAULT 'manual'
```

Tracks which links receive which echomail areas. `source` records whether the subscription was manual, AreaFix, or default.

Constraints:
- `(area_id, link_id)` is unique
- `source` is `manual`, `areafix`, or `default`

---

## Phase 0: Shared network model, schema, and config

### Objectives

Create the shared protocol-neutral network layer and the DecentDB/config foundation needed by all later phases.

### Deliverables

- `oxidebbs-network` crate added to the workspace with explicit justification in its crate docs.
- FTN-style address, duplicate-key, packet-boundary, network-message envelope, area-mapping trait, subscription trait, and queue-state types migrated from `oxidebbs-core` into `oxidebbs-network`.
- `oxidebbs-core` temporarily re-exports migrated types while existing callers are updated.
- `oxidebbs-server` config model updated for per-network local addresses, `adapter`, per-link compression, and `transport_security`.
- DecentDB schema version bumped with a table-rebuild migration where required.
- Local `messages` schema migrated for first-class external authors.
- Shared `network_*` tables and repository APIs implemented in `oxidebbs-db`.

### Schema creation order

The migration must create tables in dependency order:

```text
network_profiles
network_links
network_areas
network_packets
network_messages
network_seen_by
network_path
network_duplicate_log
network_poll_log
network_area_subscriptions
network_nodelist
```

`network_messages` references `network_packets`, so `network_packets` must
exist before `network_messages` even though the logical table descriptions above
discuss messages first.

### Tests required

- Fresh schema initializes with all Phase 0 tables.
- Existing schema migrates without losing local users, areas, messages, sessions, doors, or audit events.
- Existing local messages are backfilled with `author_kind = 'local'` and `author_display_name`.
- Imported-network author rows can be inserted with `author_user_id = NULL`.
- Config accepts multiple networks with different local addresses.
- Config rejects invalid local addresses, unknown link network keys, and plaintext legacy transport on non-legacy packet profiles.

### Definition of done

- [x] `oxidebbs-network` crate exists and has no dependency on `oxidebbs-core`, `oxidebbs-db`, `oxidebbs-ftn`, or `oxidebbs-server`
- [x] Existing network foundation types are re-exported from `oxidebbs-core` without a dependency cycle
- [x] Config supports multiple networks with independent local addresses
- [x] DecentDB migration and repository APIs exist for all Phase 0 tables
- [x] Local message authors support local, network, and system authors
- [x] All tests pass
- [x] `dev-check.sh` passes cleanly

---

## Phase 1: FTN packet format

### Prerequisites

- Phase 0 complete
- `oxidebbs-ftn` crate created in workspace
- Shared network types available from `oxidebbs-network` and re-exported from `oxidebbs-core` during transition

### Objectives

Implement complete Type-2 and Type-2+ (FSC-0039, FSC-0048) packet reading and writing.

### Deliverables

#### `PacketHeader` struct

Represents a Type-2 or Type-2+ packet header. Fields:

- `orig_node: u16`
- `dest_node: u16`
- `year: u16`
- `month: u16`
- `day: u16`
- `hour: u16`
- `minute: u16`
- `second: u16`
- `baud: u16`
- `packet_type: u16` (must be 2)
- `orig_net: u16`
- `dest_net: u16`
- `prod_code_low: u8`
- `prod_rev_low: u8`
- `password: [u8; 8]`
- `orig_zone: u16` (FSC-0048)
- `dest_zone: u16` (FSC-0048)
- `aux_net: u16` (FSC-0039)
- `cw_validation_copy: u16` (FSC-0039)
- `prod_code_high: u8` (FSC-0039)
- `prod_rev_high: u8` (FSC-0039)
- `cap_word: u16` (FSC-0039, 0x0001 for Type-2+)
- `orig_zone_2: u16` (FSC-0048)
- `dest_zone_2: u16` (FSC-0048)
- `orig_point: u16` (FSC-0048)
- `dest_point: u16` (FSC-0048)
- `prod_data: [u8; 4]`

The struct must implement:
- `fn is_type2_plus(&self) -> bool` — returns true if `cap_word & 0x0001 != 0`
- `fn originating_address(&self, context: PacketZoneContext) -> Result<FtnAddress, PacketError>` — constructs address from explicit Type-2+ fields or caller-supplied Type-2 zone context
- `fn destination_address(&self, context: PacketZoneContext) -> Result<FtnAddress, PacketError>` — constructs address from explicit Type-2+ fields or caller-supplied Type-2 zone context

`PacketZoneContext` supplies the network/link zones needed for legacy Type-2 packets that do not carry reliable zone fields.

#### `PacketMessage` struct

Represents a single message within a packet. Fields:

- `message_type: u16` (must be 2)
- `orig_node: u16`
- `dest_node: u16`
- `orig_net: u16`
- `dest_net: u16`
- `attribute: u16`
- `cost: u16`
- `datetime_raw: [u8; 20]` (ASCII bytes, format: `"DD MMM YY  HH:MM:SS"`)
- `to_username_raw: Vec<u8>` (max 36 bytes before NUL)
- `from_username_raw: Vec<u8>` (max 36 bytes before NUL)
- `subject_raw: Vec<u8>` (max 72 bytes before NUL)
- `text_raw: Vec<u8>` (variable length, NUL terminator removed)

Packet I/O is byte-oriented. Decoded `String` accessors may be provided for display and tests, but parsing and writing must preserve the original bytes except where OxideBBS deliberately composes a new outbound packet.

#### `MessageAttribute` bitflags

```rust
bitflags! {
    pub struct MessageAttribute: u16 {
        const PRIVATE        = 0x0001;
        const CRASH          = 0x0002;
        const RECEIVED       = 0x0004;
        const SENT           = 0x0008;
        const FILE_ATTACH    = 0x0010;
        const IN_TRANSIT     = 0x0020;
        const ORPHAN         = 0x0040;
        const KILL_SENT      = 0x0080;
        const LOCAL          = 0x0100;
        const HOLD           = 0x0200;
        const FILE_REQUEST   = 0x0400;
        const RETURN_RECEIPT_REQUEST = 0x0800;
        const IS_RETURN_RECEIPT      = 0x1000;
        const AUDIT_REQUEST  = 0x2000;
        const FILE_UPDATE_REQ = 0x4000;
    }
}
```

#### `PacketReader`

Reads a `.pkt` file from a `Read` trait:

- `fn read_header(reader: &mut impl Read) -> Result<PacketHeader, PacketError>`
- `fn read_message(reader: &mut impl Read) -> Result<Option<PacketMessage>, PacketError>` — returns `Ok(None)` when the termination marker (two zero bytes for message_type) is encountered
- `fn read_all(reader: &mut impl Read) -> Result<(PacketHeader, Vec<PacketMessage>), PacketError>`

#### `PacketWriter`

Writes a `.pkt` file to a `Write` trait:

- `fn write_header(writer: &mut impl Write, header: &PacketHeader) -> Result<(), PacketError>`
- `fn write_message(writer: &mut impl Write, message: &PacketMessage) -> Result<(), PacketError>`
- `fn write_terminator(writer: &mut impl Write) -> Result<(), PacketError>`

The writer must always produce Type-2+ packets with FSC-0048 extensions (orig_point, dest_point, zone copies).

#### `PacketError` enum

```rust
enum PacketError {
    Io(std::io::Error),
    InvalidPacketType(u16),
    InvalidHeaderLength(usize),
    InvalidMessageLength(usize),
    InvalidDatetime(String),
    PasswordMismatch,
    TruncatedHeader,
    TruncatedMessage,
    ZeroTerminator,
}
```

#### OxideBBS product codes

The packet header's `prod_code` and `prod_rev` fields should identify OxideBBS:

- `prod_code`: Register with the FidoNet product code list or use an unassigned code (document the choice).
- `prod_rev`: OxideBBS version encoded as a single byte.

### Technical details

- All numeric fields in the packet header and message header are little-endian (u16 LE).
- Null-terminated fields are byte strings. Header fields are fixed-width and padded with null bytes; message fields are variable-length null-terminated byte strings.
- The packet terminator is a u16 zero value where a message type field would be.
- Passwords are compared case-insensitively as per FTN convention.
- Type-2+ packets MUST include the capability word `0x0001` and its validation copy.
- When reading Type-2 (non-2+) packets, point fields default to 0 and zones are resolved from the link/network context supplied by the tosser or caller. Zones must not be guessed from `orig_net` or `dest_net`.
- Text decoding uses the configured network encoding (CP437 by default for legacy FTN). Packet parsing succeeds even when bytes are not valid UTF-8.

### Tests required

- Parse a known-good Type-2 packet header from raw bytes
- Parse a known-good Type-2+ packet header from raw bytes
- Write a packet header and re-read it, confirming round-trip consistency
- Parse a message with all attribute flags set
- Parse a message with minimal fields
- Write a message and re-read it, confirming round-trip consistency
- Round-trip an entire packet (header + N messages + terminator)
- Reject a packet with `packet_type != 2`
- Handle a packet with empty password field
- Handle a packet with case-insensitive password comparison
- Parse a Type-2+ packet with point addresses
- Parse a Type-2 packet where zone/point fields default
- Write a Type-2+ packet with FSC-0048 extensions
- Handle malformed packets: truncated header, truncated message, missing terminator
- Handle messages with maximum-length fields (36-char username, 72-char subject)
- Handle messages with empty text
- Handle messages with control characters in text (kludges)
- Preserve non-UTF-8 message text bytes through read/write round trips

### Documentation required

- Rustdoc on all public types and functions in `oxidebbs-ftn`
- ADR for packet format choice (Type-2+ as default output, Type-2 and Type-2+ input)
- Reference to FTS-0001, FSC-0039, FSC-0048 in code comments

### Definition of done

- [ ] `oxidebbs-ftn` crate compiles and passes `cargo check --workspace --locked`
- [ ] `PacketHeader`, `PacketMessage`, `MessageAttribute` types are defined and documented
- [ ] `PacketReader` reads Type-2 and Type-2+ packets from `impl Read`
- [ ] `PacketWriter` writes Type-2+ packets to `impl Write`
- [ ] `PacketError` covers all failure modes
- [ ] All tests pass (`cargo test --workspace --locked`)
- [ ] Rustdoc is complete on all public types
- [ ] ADR is written for packet format choice
- [ ] No `unwrap()` or `expect()` in library code
- [ ] `dev-check.sh` passes cleanly

---

## Phase 2: Echomail message model

### Prerequisites

- Phase 1 complete (packet format reading/writing)

### Objectives

Parse and generate the full echomail message format including all kludge lines, control lines, tear lines, and origin lines.

### Deliverables

#### `EchomailKludge` parser

Parses the text body of an echomail message into structured kludge data. FTN echomail messages embed metadata in the message text using control lines:

| Line | Format | Description |
|---|---|---|
| AREA | `AREA:TAGNAME\r` | Echomail area tag (first line) |
| MSGID | `^MSGID: address serial\r` | Unique message identifier |
| REPLY | `^REPLY: serial\r` | References parent MSGID |
| INTL | `^INTL dest orig\r` | Netmail international routing (zone info) |
| FMPT | `^FMPT point\r` | Netmail from-point |
| TOPT | `^TOPT point\r` | Netmail to-point |
| FLAGS | `^FLAGS flags\r` | Message flags (DIR, IMM, K/S, etc.) |
| SEEN-BY | `SEEN-BY: addr addr ...\r` | Echomail loop prevention |
| PATH | `^PATH: addr addr ...\r` | Message routing trail |
| Via | `^Via: address timestamp program\r` | Transport audit trail |
| Tear line | `--- software\r` | Separator before origin |
| Origin | ` * Origin: Board Name (Z:N/N.P)\r` | Originating BBS |

The parser must handle:
- Raw message text bytes from `.pkt` format after the packet reader removes the terminating NUL
- `\r` (CR) as line terminator within `.pkt` messages
- `\n` (LF) or `\r\n` should also be tolerated on input
- Kludge lines starting with `^` (ASCII 0x01) before the blank separator line
- `AREA:` line is NOT preceded by `^` — it is the first line of an echomail message
- SEEN-BY, tear line, origin line appear after the message body text
- ^PATH lines appear after SEEN-BY

#### `FtnParsedMessage` struct

```rust
struct FtnParsedMessage {
    area_tag: Option<String>,
    msgid: Option<String>,
    replyid: Option<String>,
    intl: Option<String>,
    fmpt: Option<u16>,
    topt: Option<u16>,
    flags: Option<String>,
    seen_by: Vec<FtnAddress>,
    path: Vec<FtnAddress>,
    via: Vec<String>,
    body_raw: Vec<u8>,
    body_display_text: String,
    tear_line: Option<String>,
    origin_line: Option<String>,
    origin_address: Option<FtnAddress>,
    unknown_kludges: Vec<Vec<u8>>,
}
```

#### `FtnMessageComposer`

Generates the raw message text (including kludges, body, SEEN-BY, PATH, tear, origin) from structured data. This is the inverse of the kludge parser.

- `fn compose_echomail(params: &EchomailComposeParams) -> Vec<u8>`
- `fn compose_netmail(params: &NetmailComposeParams) -> Vec<u8>`

Output must use `\r` line terminators for `.pkt` format compatibility.

#### `FtnAddressList` parser

Parses SEEN-BY and PATH address lists. These use 2D addresses (net/node) within a zone and 3D addresses (zone:net/node) for cross-zone entries:

```text
SEEN-BY: 105/42 105/100 2:2/320 340/102
```

The parser must handle:
- Space-separated address tokens
- 2D addresses (`net/node`) — zone is implied from the message's originating zone
- 3D addresses (`zone:net/node`) — explicit zone
- Duplicate SEEN-BY addresses (remove silently)
- Out-of-order SEEN-BY addresses (sort for canonical output)
- PATH addresses in their original routing order; do not sort PATH

### Technical details

- The `^` character in kludge lines is the ASCII SOH character (0x01), not the caret symbol. The parser must handle both `0x01MSGID:` and `^MSGID:` in string representation.
- MSGID format: `Z:N/N.P serialnum` where the serial number is a hex string. Example: `1:105/42.0 4a3b2c1d`. The serial must be treated as opaque — do not parse or validate the serial format beyond checking it is non-empty.
- REPLYID format: same as MSGID. References the parent message's MSGID.
- The AREA: line is always the first line of an echomail message. It is NOT a kludge (no `^` prefix).
- Tear line format: `---` followed by an optional software tag. Example: `--- OxideBBS/0.4.0`. The three dashes and a space are mandatory.
- Origin line format: ` * Origin: Display Text (Z:N/N.P)`. The address in parentheses must be parseable as an `FtnAddress`.
- SEEN-BY addresses use short format (net/node) within the same zone and long format (zone:net/node) for cross-zone.
- PATH addresses follow the same address syntax as SEEN-BY but preserve routing order.
- Kludge names and routing addresses are ASCII-compatible control metadata. Message body bytes are decoded only for `body_display_text`; `body_raw` remains authoritative for duplicate hashing and re-export.

### Tests required

- Parse an echomail message with AREA, MSGID, body text, SEEN-BY, PATH, tear, and origin
- Parse a netmail message with INTL, FMPT, TOPT kludges
- Parse a message with no kludges (plain text)
- Parse a message with only AREA tag (no MSGID)
- Compose an echomail message and verify all kludges are present
- Compose a netmail message with INTL, FMPT, TOPT
- Parse SEEN-BY with 2D and 3D addresses
- Parse SEEN-BY with duplicates
- Parse PATH with mixed 2D and 3D addresses
- Compose SEEN-BY and verify canonical output (sorted, deduplicated)
- Compose PATH and verify original routing order is preserved
- Round-trip: compose a message, parse it, verify all fields match
- Handle edge cases: empty body, very long body (> 64 KB), messages with no origin line, messages with no tear line
- Handle malformed kludge lines gracefully (skip and log, do not fail)
- Verify `\r` line terminators in composed output
- Verify MSGID and REPLYID are preserved exactly (no case folding)
- Verify non-UTF-8 body bytes are preserved in `body_raw`

### Documentation required

- Rustdoc on `FtnParsedMessage`, `FtnMessageComposer`, `FtnAddressList`
- ADR for kludge handling strategy (tolerant parsing, strict composition)
- Reference to FTS-0005 (Echomail), FSC-0068 (SEEN-BY/PATH), FSC-0087 (MSGID), FSC-0091 (REPLYID)

### Definition of done

- [ ] `FtnParsedMessage` parses all echomail kludge types (AREA, MSGID, REPLY, INTL, FMPT, TOPT, FLAGS, SEEN-BY, PATH, Via)
- [ ] `FtnMessageComposer` composes echomail and netmail messages with proper line terminators
- [ ] `FtnAddressList` parses and sorts SEEN-BY address lists while preserving PATH order
- [ ] Tear line and origin line parsing and composition work correctly
- [ ] All tests pass
- [ ] Rustdoc complete on all public types
- [ ] ADR written for kludge handling strategy
- [ ] `dev-check.sh` passes cleanly

---

## Phase 3: Duplicate detection

### Prerequisites

- Phase 1 complete
- Phase 2 complete (MSGID parsing)
- Phase 0 complete (DecentDB `network_messages` and `network_duplicate_log` tables implemented in `oxidebbs-db`)

### Objectives

Implement MSGID-based duplicate detection to prevent the same message from being imported twice.

### Deliverables

#### `DuplicateDetector` trait

```rust
trait DuplicateDetector {
    fn check_echomail(
        &self,
        network_key: &str,
        area_tag: &str,
        msgid: Option<&str>,
        origin_address: &FtnAddress,
        created_at: &str,
        subject: &str,
        body_hash: &[u8; 32],
    ) -> Result<DuplicateCheckResult, DuplicateError>;

    fn check_netmail(
        &self,
        network_key: &str,
        msgid: Option<&str>,
        from_address: &FtnAddress,
        to_address: &FtnAddress,
        created_at: &str,
        subject: &str,
        body_hash: &[u8; 32],
    ) -> Result<DuplicateCheckResult, DuplicateError>;

    fn record_message(
        &self,
        message: &FtnMessage,
        duplicate_hash: &str,
    ) -> Result<(), DuplicateError>;
}
```

#### `DuplicateCheckResult` enum

```rust
enum DuplicateCheckResult {
    Unique,
    Duplicate {
        original_message_id: String,
        first_seen_at: String,
    },
}
```

#### `DecentDBDuplicateDetector` implementation

Backed by the `network_messages` and `network_duplicate_log` tables in DecentDB.

#### Duplicate hash computation

When MSGID is present for echomail:

```text
duplicate_hash = sha256(network_key + "\x00" + area_tag + "\x00" + msgid)
```

When MSGID is present for netmail:

```text
duplicate_hash = sha256(network_key + "\x00" + from_address.to_string() + "\x00" + to_address.to_string() + "\x00" + msgid)
```

When MSGID is absent (fallback):

```text
duplicate_hash = sha256(network_key + "\x00" + area_tag + "\x00" + origin_address.to_string() + "\x00" + created_at + "\x00" + subject + "\x00" + hex(body_hash))
```

The fallback hash must tolerate a ±5 minute clock skew window on `created_at` when querying DecentDB.

### Technical details

- MSGID is the primary duplicate key. Two echomail messages with the same MSGID in the same area are duplicates regardless of content. The same MSGID in different echo areas is treated as unique.
- If MSGID is absent, the fallback hash uses multiple fields. A tolerance window of ±5 minutes on `created_at` prevents false negatives from clock drift.
- Body hash uses SHA-256 of the raw message body bytes after stripping trailing whitespace and normalizing line endings to `\r`.
- The duplicate check must happen before a message is imported into a local area. If a duplicate is found, the message is logged to `network_duplicate_log` and not imported.
- Duplicate detection must be fast enough to handle a full inbound packet (potentially hundreds of messages) without blocking the toss for more than a few seconds.

### Tests required

- Unique message passes the duplicate check
- Duplicate MSGID is detected (same MSGID, different packet)
- Fallback hash detects duplicates when MSGID is absent
- Clock skew tolerance: messages with `created_at` within ±5 minutes are compared
- Messages with identical content but different MSGIDs are treated as unique
- Different areas with the same MSGID are unique (area is part of the key)
- `record_message` stores the hash for future duplicate checks
- Querying `network_duplicate_log` shows the rejection event

### Documentation required

- Rustdoc on `DuplicateDetector` trait and `DecentDBDuplicateDetector`
- ADR for duplicate detection strategy (MSGID-primary, hash-fallback)

### Definition of done

- [ ] `DuplicateDetector` trait defined in `oxidebbs-ftn`
- [ ] `DecentDBDuplicateDetector` implemented in `oxidebbs-ftn` (or `oxidebbs-db`)
- [ ] `network_duplicate_log` table created in DecentDB schema migration
- [ ] MSGID-based dedup works with real `.pkt` data
- [ ] Fallback hash works when MSGID is absent
- [ ] Clock skew tolerance is implemented
- [ ] All tests pass
- [ ] Rustdoc complete
- [ ] `dev-check.sh` passes cleanly

---

## Phase 4: Tosser — inbound processing

### Prerequisites

- Phase 1 complete (packet reading)
- Phase 2 complete (kludge parsing)
- Phase 3 complete (duplicate detection)
- Phase 0 complete (DecentDB `network_profiles`, `network_links`, `network_areas`, `network_messages`, `network_packets`, `network_seen_by`, `network_path` tables implemented)

### Objectives

Implement the tosser: receive inbound `.pkt` files, unpack bundles, validate, parse messages, run duplicate detection, route to local areas, update SEEN-BY and PATH records.

### Deliverables

#### `Tosser` struct

```rust
struct Tosser {
    db: Arc<OxideDb>,
    config: FtnConfig,
    networks: HashMap<String, FtnNetworkConfig>,
    areas: HashMap<String, FtnAreaMapping>,
}
```

#### Toss workflow

```text
1. Scan inbound directory for new files.
2. If file is a bundle (.su, .mo, .tu, .we, .th, .fr, .sa extension):
   a. Extract the bundle to temp_inbound.
   b. For each .pkt file in the extracted bundle:
      - Process the .pkt file.
3. If file is a raw .pkt file:
   a. Process the .pkt file directly.
4. For each .pkt file:
   a. Read the packet header.
   b. Validate the packet:
      - Password matches a known link.
      - Originating address matches a known link.
      - Packet is not malformed.
   c. If validation fails, quarantine the packet and log the error.
   d. Read each message from the packet.
   e. For echomail messages:
      - Find the local area mapped to the AREA tag.
      - If no mapping exists, quarantine or skip per configuration.
      - Parse kludge lines (MSGID, REPLY, SEEN-BY, PATH, etc.).
      - Run duplicate detection.
      - If duplicate, log and skip.
      - Decode display text through the configured network encoding.
      - Store the message in the local area via `oxidebbs-db` using `author_kind = 'network'`, `author_display_name`, and `author_network_address`.
      - Record SEEN-BY and PATH entries.
      - Record the message in `network_messages`.
   f. For netmail messages:
      - Parse kludge lines (MSGID, INTL, FMPT, TOPT, FLAGS, Via, etc.).
      - Determine if the netmail is addressed to this system.
      - If addressed to this system, route to the local private mail area.
      - If addressed to another system, route to the outbound queue for forwarding.
      - Run duplicate detection.
      - Decode display text through the configured network encoding.
      - Store or forward as appropriate using the external-author fields for imported messages.
5. Move the processed .pkt to an archive directory.
6. Log the toss results.
```

#### Bundle extraction

A bundle file is a compressed archive containing one or more `.pkt` files. The tosser must support:

- `.zip` bundles (most common)
- `.arj` bundles (legacy, can be deferred)
- Uncompressed `.pkt` files (direct processing)

Bundle extraction:

```rust
fn extract_bundle(bundle_path: &Path, temp_dir: &Path) -> Result<Vec<PathBuf>, TosserError>
```

#### Packet validation

```rust
fn validate_packet(
    header: &PacketHeader,
    known_links: &[FtnLinkConfig],
) -> Result<FtnLinkConfig, TosserError>
```

Validates:
- Password matches a known link (case-insensitive)
- Originating address matches a known link
- Packet type is 2 or 2+
- Header fields are internally consistent

#### Area routing

```rust
fn route_echomail(
    area_tag: &str,
    network_key: &str,
    area_mappings: &HashMap<String, FtnAreaMapping>,
) -> Result<Uuid, TosserError>
```

Looks up the local area ID for an echo tag. If no mapping exists, the message is quarantined or skipped per configuration.

#### Toss result

```rust
struct TossResult {
    packets_processed: usize,
    messages_imported: usize,
    messages_duplicate: usize,
    messages_quarantined: usize,
    messages_skipped: usize,
    errors: Vec<TosserError>,
}
```

### Technical details

- The tosser must not crash on malformed packets. Any parsing error should quarantine the packet and continue with the next file.
- Quarantined packets are moved to the quarantine directory and logged in `network_packets` with `status = 'quarantined'`.
- The tosser must handle packets where the password is wrong — quarantine, do not silently accept.
- Echomail messages with an unknown AREA tag should be handled per configuration: either quarantine, skip with a warning, or create a new local area (if auto-subscribe is enabled — should be disabled by default).
- SEEN-BY entries are stored normalized in `network_seen_by` for loop detection during scanning.
- PATH entries are stored normalized in `network_path` in order.
- The tosser must handle both Type-2 and Type-2+ packets transparently.
- Netmail addressed to this system (matching our address) should be delivered locally.
- Netmail addressed to another system should be placed in the outbound queue for forwarding.
- The tosser is not all-or-nothing per packet. It records packet-level failures and quarantines the original file when needed, but successfully imported messages from earlier in the same packet are retained and recorded. Message-level failures are logged with enough context to avoid repeated silent retries.

### Tests required

- Toss a known-good echomail packet and verify messages appear in local areas
- Toss a packet with a wrong password — verify quarantine and error logging
- Toss a packet with an unknown AREA tag — verify skip/quarantine behavior
- Toss a packet containing a duplicate message — verify detection and skip
- Toss a packet with a malformed header — verify quarantine and error logging
- Toss a bundle (.zip containing .pkt files) — verify extraction and processing
- Toss a netmail message addressed to this system — verify local delivery
- Toss a netmail message addressed to another system — verify outbound queue placement
- Toss a packet with SEEN-BY and PATH entries — verify storage in `network_seen_by` and `network_path`
- Toss an empty packet (no messages) — verify graceful handling
- Toss a packet where messages reference each other via REPLYID — verify parent linking
- Verify `network_packets` records are created with correct status
- Verify `network_messages` records are created with correct `status = 'imported'`
- End-to-end: compose a packet, write it to inbound, run tosser, verify local messages

### Documentation required

- Rustdoc on `Tosser`, `TossResult`, and all public methods
- Toss workflow diagram in `docs/ftn/tosser.md`
- ADR for toss error handling strategy (quarantine vs. skip vs. fail)
- Configuration reference for tosser settings

### Definition of done

- [ ] `Tosser` struct implemented in `oxidebbs-ftn`
- [ ] Bundle extraction supports `.zip` and raw `.pkt`
- [ ] Packet validation checks password, address, and header consistency
- [ ] Echomail routing maps AREA tags to local areas
- [ ] Netmail routing handles local delivery and forwarding
- [ ] Duplicate detection is integrated into the toss workflow
- [ ] SEEN-BY and PATH are stored in DecentDB
- [ ] Quarantine handling works for malformed packets
- [ ] All DecentDB tables (`network_packets`, `network_messages`, `network_seen_by`, `network_path`) are populated correctly
- [ ] All tests pass
- [ ] Rustdoc complete
- [ ] ADR written for toss error handling strategy
- [ ] `dev-check.sh` passes cleanly

---

## Phase 5: Scanner — outbound processing

### Prerequisites

- Phase 4 complete (tosser handles inbound)
- DecentDB `network_outbound_queue` table for per-link outbound tracking

### Objectives

Implement the scanner: detect new echomail and netmail messages in local areas, create outbound `.pkt` files for each link, generate SEEN-BY and PATH lines, compose bundle files.

### Deliverables

#### `Scanner` struct

```rust
struct Scanner {
    db: Arc<OxideDb>,
    config: FtnConfig,
    local_address: FtnAddress,
    networks: HashMap<String, FtnNetworkConfig>,
    links: Vec<FtnLinkConfig>,
    areas: HashMap<String, FtnAreaMapping>,
}
```

#### Scan workflow

```text
1. For each echomail area that has a network mapping:
   a. Query DecentDB for local messages eligible for export.
   b. For each subscribed link, create a `network_outbound_queue` row when no active or successful row already exists for `(link_id, local_message_id)`.
   c. Exclude messages where the link's address already appears in SEEN-BY before queueing (loop prevention).
2. For each link with queued outbound rows:
   a. Load queued messages for that link.
   b. Add the local address to SEEN-BY and PATH metadata.
   c. Compose the full echomail text: AREA tag, kludges, body, tear, origin, SEEN-BY, PATH.
   d. Create a PacketMessage from the composed bytes.
3. For each link:
   a. Create a PacketHeader with our per-network local address as origin and the link address as destination.
   b. Write the messages into a .pkt file.
   c. Record the .pkt in network_packets.
   d. Record each network representation in network_messages with status='exported'.
   e. Update corresponding `network_outbound_queue` rows to `packed` and attach `packet_id`.
4. Optionally bundle .pkt files into compressed arcmail bundles.
5. Place .pkt or bundle files in the outbound directory for the link.
6. Log the scan results.
```

#### `EchomailComposer`

Generates the full echomail message text for outbound:

```text
AREA:TAGNAME\r
^MSGID: Z:N/N.P serial\r
^REPLY: original_msgid\r
\r
Message body text.\r
\r
--- OxideBBS/0.4.0\r
 * Origin: Board Name (Z:N/N.P)\r
SEEN-BY: Z:N/N Z:N/N\r
^PATH: Z:N/N\r
\0
```

#### `NetmailComposer`

Generates the full netmail message text for outbound:

```text
^INTL Z:N/N.P Z:N/N.P\r
^FMPT point\r
^TOPT point\r
^MSGID: Z:N/N.P serial\r
^FLAGS FLAGS\r
\r
Message body text.\r
--- OxideBBS/0.4.0\r
\0
```

#### Seen-by and PATH management

The scanner must:

1. When scanning a message for a link, check if the link's address is already in SEEN-BY. If yes, skip the message for that link (loop prevention).
2. Add our own address to SEEN-BY before exporting.
3. Add our own address to PATH before exporting.
4. Use 2D format (net/node) for same-zone addresses and 3D format (zone:net/node) for cross-zone addresses in SEEN-BY and PATH.
5. Sort SEEN-BY addresses for canonical output.
6. Preserve PATH routing order and append our own address at the end.

#### Scan result

```rust
struct ScanResult {
    messages_scanned: usize,
    packets_created: usize,
    links_processed: usize,
    errors: Vec<ScannerError>,
}
```

### Technical details

- The scanner must produce Type-2+ packets only (with FSC-0048 extensions).
- Packet passwords for outbound packets are set per-link from configuration.
- The MSGID serial number should be a hex string unique per system. Common practice is a hash of the message content or a counter. OxideBBS should use a high-entropy random hex string.
- The origin line must include the BBS name and FTN address in the standard format.
- SEEN-BY and PATH must be added AFTER the body text and origin line, in that order.
- For netmail, the INTL kludge is mandatory for cross-zone messages. FMPT and TOPT are mandatory if point addresses are nonzero.
- The scanner must not create duplicate active queue rows for a `(link_id, local_message_id)` pair.
- A message may be exported to one link and still remain queued or failed for another link.
- Messages from moderated areas that are still in `MessageVisibility::PendingModeration` must not be exported until approved.

### Tests required

- Scan a local area with new messages — verify outbound packets are created
- Scan a local area with no new messages — verify no packets are created
- Verify SEEN-BY includes our address after scanning
- Verify PATH includes our address after scanning
- Verify messages with successful queue rows for a link are not queued for that same link again
- Verify a message exported to one link can still be queued for another subscribed link
- Verify failed queue rows can be retried without duplicating successful deliveries
- Verify messages in a moderated area with `PendingModeration` status are not exported
- Scan for two links — verify each link gets the correct messages based on area subscriptions
- Verify loop prevention: a message with a link's address in SEEN-BY is not sent to that link
- Compose an echomail message and verify AREA, MSGID, body, tear, origin, SEEN-BY, PATH are all present
- Compose a netmail message and verify INTL, FMPT, TOPT, MSGID are present
- Verify outbound packet header has correct origin and destination addresses
- Verify outbound packet password matches link configuration
- Verify MSGID is unique per message
- Verify `\r` line terminators in composed output
- End-to-end: scan, write packet, re-read packet with `PacketReader`, verify all fields match

### Documentation required

- Rustdoc on `Scanner`, `EchomailComposer`, `NetmailComposer`
- Scan workflow diagram in `docs/ftn/scanner.md`
- ADR for outbound MSGID generation strategy (random hex vs. hash vs. counter)
- Configuration reference for scanner settings

### Definition of done

- [ ] `Scanner` struct implemented in `oxidebbs-ftn`
- [ ] `EchomailComposer` generates full echomail text with all kludges
- [ ] `NetmailComposer` generates full netmail text with INTL, FMPT, TOPT
- [ ] SEEN-BY loop prevention works correctly
- [ ] PATH propagation works correctly
- [ ] Messages are grouped by link and only sent to subscribed links
- [ ] Outbound packets are written as Type-2+ with correct addresses
- [ ] Network message representations are tracked in `network_messages` with `status='exported'`
- [ ] Per-link delivery state is tracked in `network_outbound_queue`
- [ ] Moderated messages are not exported until approved
- [ ] All tests pass
- [ ] Rustdoc complete
- [ ] ADR written for MSGID generation
- [ ] `dev-check.sh` passes cleanly

---

## Phase 6: Bundle format

### Prerequisites

- Phase 1 complete (packet reading/writing)
- Phase 5 complete (scanner creates outbound packets)

### Objectives

Implement arcmail bundle naming, creation, and extraction.

### Deliverables

#### `BundleNamer`

Generates arcmail bundle filenames following the standard naming convention:

```text
NMNNNNNN.MO?
```

Where:
- `NM` = lower 2 hex digits of the source net
- `NNNNNN` = implementation-specific address-pair encoding; Phase 6 must choose and document the exact convention before code is written.

The most widely used convention (from hpt/GoldED/ifmail):

```text
bbbbbbbb.dxx
```

Where:
- `bbbbbbbb` = CRC16 or hex encoding of the source and destination addresses
- `dxx` = day-of-week extension (.su .mo .tu .we .th .fr .sa)

For 4D addresses (zone:net/node.point), the naming incorporates zone information when addresses are in different zones:

```text
zNFbbbb.dxx
```

Where:
- `z` = zone of the destination (hex digit)
- `N` = net of the destination (hex digit if different zone, `0` if same zone)
- `F` = flag character for special routing states
- `bbbb` = encoding of source/dest node numbers

OxideBBS should implement the most common convention used by hpt (the Husky Project tosser) for maximum interoperability.

Day-of-week extensions:

| Day | Extension |
|---|---|
| Sunday | .su |
| Monday | .mo |
| Tuesday | .tu |
| Wednesday | .we |
| Thursday | .th |
| Friday | .fr |
| Saturday | .sa |

#### `BundleCreator`

```rust
fn create_bundle(
    packet_path: &Path,
    dest_address: &FtnAddress,
    our_address: &FtnAddress,
    day: Weekday,
    compression: BundleCompression,
    output_dir: &Path,
) -> Result<PathBuf, BundleError>
```

Supported compression formats for v1:

- `none` — raw `.pkt` file without bundling (for local testing and filesystem-based exchange)
- `zip` — ZIP compression (most common modern format)

Future:

- `arj` — ARJ compression (common in FidoNet, deferred)

#### `BundleExtractor`

```rust
fn extract_bundle(
    bundle_path: &Path,
    output_dir: &Path,
) -> Result<Vec<PathBuf>, BundleError>
```

Extracts all `.pkt` files from a compressed bundle. Must handle:

- `.zip` bundles
- Raw `.pkt` files (pass through)
- Unknown formats (return error, do not crash)

#### `BundleCompression` enum

```rust
enum BundleCompression {
    None,
    Zip,
}
```

### Tests required

- Generate a bundle filename for a known address pair and day — verify it matches expected convention
- Create a ZIP bundle from a packet file — verify it can be extracted
- Extract a known-good ZIP bundle — verify .pkt files are recovered
- Extract a raw .pkt file (no compression) — verify pass-through
- Attempt to extract a corrupt bundle — verify error handling
- Verify day-of-week extension is correct for each day
- Verify bundle filenames for same-zone and cross-zone addresses
- Verify bundle creation with `None` compression produces the original .pkt file

### Documentation required

- Rustdoc on `BundleNamer`, `BundleCreator`, `BundleExtractor`
- Reference to FSC-0064 and hpt naming conventions
- Bundle naming convention explanation in `docs/ftn/bundles.md`

### Definition of done

- [ ] `BundleNamer` generates correct arcmail filenames for all address types
- [ ] `BundleCreator` creates ZIP bundles from packet files
- [ ] `BundleExtractor` extracts packets from ZIP bundles and handles raw packets
- [ ] Day-of-week extensions are correct
- [ ] All tests pass
- [ ] Rustdoc complete
- [ ] `dev-check.sh` passes cleanly

---

## Phase 7: Nodelist

### Prerequisites

- Phase 1 complete (packet format)
- DecentDB tables for nodelist storage

### Objectives

Parse standard FidoNet nodelist files, build a lookup index, and provide efficient node-by-address queries.

### Deliverables

#### `NodelistParser`

Parses a standard nodelist file (NODELIST.xxx format). The nodelist format is:

```text
; Comment lines start with semicolon
Zone,1,Worstelen,Horst_Kalverkamp,43-XXXX-XXXX,9600,CM,XA,MO,V34
Region,1,Region_1_Coordinator,...
Host,1/1,BBS_Name,Sysop_Name,Location,Phone,Speed,Flags
Hub,1/1,Hub_Name,Sysop_Name,Location,Phone,Speed,Flags
Pvt,1/2,Private_Node,Sysop_Name,Location,Phone,Speed,Flags
Hold,1/3,On_Hold_Node,Sysop_Name,Location,Phone,Speed,Flags
Down,1/4,Down_Node,Sysop_Name,Location,Phone,Speed,Flags
,1/100,Normal_Node,Sysop_Name,Location,Phone,Speed,Flags
```

The parser must handle:

- Comment lines (starting with `;`)
- Blank lines
- Entry types: Zone, Region, Host, Hub, Pvt, Hold, Down, and untyped (node entries with no keyword)
- Keyword lines: `Zone`, `Region`, `Host`, `Hub`, `Pvt`, `Hold`, `Down`
- Comma-separated fields: keyword, address, name, sysop, location, phone, speed, flags
- Flags: remaining comma-separated tokens after the speed field, including bare flags (CM, XA, MO, V34, etc.) and value-style flags such as `INA:host.example.net`
- The address format in nodelist is `zone:net/node` for Zone entries, `net/node` for others, or just `node` for entries within a known net
- Multi-line entries using continuation lines (a line starting with `,` continues the previous entry)

#### `NodelistEntry` struct

```rust
struct NodelistEntry {
    level: NodelistLevel,
    zone: u16,
    net: u16,
    node: u16,
    point: u16,
    name: String,
    sysop: String,
    location: String,
    phone: String,
    speed: u16,
    flags: HashMap<String, Option<String>>,
    keyword: NodelistKeyword,
}

enum NodelistLevel {
    Zone,
    Region,
    Host,
    Hub,
    Node,
}

enum NodelistKeyword {
    Zone,
    Region,
    Host,
    Hub,
    Pvt,
    Hold,
    Down,
    Normal,
}
```

#### `NodelistIndex`

Stores parsed nodelist entries in DecentDB for efficient lookup.

```rust
trait NodelistIndex {
    fn build_index(&self, entries: &[NodelistEntry]) -> Result<(), NodelistError>;
    fn lookup(&self, address: &FtnAddress) -> Result<Option<NodelistEntry>, NodelistError>;
    fn lookup_by_name(&self, name: &str) -> Result<Vec<NodelistEntry>, NodelistError>;
    fn count_entries(&self) -> Result<usize, NodelistError>;
}
```

#### DecentDB table: `network_nodelist`

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
zone INT NOT NULL
net INT NOT NULL
node INT NOT NULL
point INT NOT NULL DEFAULT 0
keyword TEXT NOT NULL DEFAULT 'normal'
name TEXT NOT NULL
sysop TEXT NOT NULL
location TEXT NOT NULL DEFAULT ''
phone TEXT NOT NULL DEFAULT ''
speed INT NOT NULL DEFAULT 0
flags TEXT NOT NULL DEFAULT ''
nodelist_file TEXT NOT NULL
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
```

Constraints:
- `keyword` is one of `zone`, `region`, `host`, `hub`, `pvt`, `hold`, `down`, `normal`
- `(zone, net, node, point)` is unique per nodelist file

Indexes:
- `(zone, net, node, point)`
- `(name)`

### Technical details

- The nodelist is a text file distributed as a diff series (NODELIST.001, NODELIST.002, ..., NODELIST.xxx where xxx is the day number). For v1, OxideBBS will only support full nodelist files, not incremental diffs.
- Nodelist files may be compressed (e.g., NODELIST.ZIP). The parser should accept a file path, not handle decompression itself — that is the caller's responsibility.
- The phone field may contain `-` characters and non-numeric prefixes. Do not validate phone format strictly.
- The speed field is a baud rate as an integer. Common values: 300, 1200, 2400, 9600, 14400, 28800, 33600, 56000.
- Some nodelist entries use `Pvt` keyword with no phone number — these nodes are private and do not accept direct connections.
- Flags are comma-separated fields after speed. Some flags have values (e.g. `INA:host.example.net`), others are bare (e.g. `CM`, `XA`). Tolerate semicolon-separated flag fragments only as a non-standard compatibility fallback.
- Building the nodelist index should be an idempotent operation — replacing the existing index with a new one.

### Tests required

- Parse a known-good nodelist file with Zone, Host, Hub, and Node entries
- Parse comment lines and blank lines (skip them)
- Parse entries with various keywords (Zone, Region, Host, Hub, Pvt, Hold, Down, Normal)
- Parse entries with flags including key=value pairs and bare flags
- Parse entries with continuation lines
- Build a nodelist index from parsed entries
- Look up a node by address — verify correct entry returned
- Look up a node by name — verify matching entries returned
- Handle a nodelist with malformed entries — verify graceful error handling
- Handle entries with `Pvt` keyword — verify private flag is set
- Handle entries with `Down` keyword — verify down flag is set
- Rebuild the index from a new nodelist — verify idempotent replacement

### Documentation required

- Rustdoc on `NodelistParser`, `NodelistEntry`, `NodelistIndex`
- Reference to FTS-4000 (Nodelist format) in code comments
- Nodelist processing guide in `docs/ftn/nodelist.md`

### Definition of done

- [ ] `NodelistParser` parses standard nodelist files
- [ ] `NodelistEntry` struct captures all nodelist fields including flags
- [ ] `NodelistIndex` trait and DecentDB implementation provide efficient lookup
- [ ] `network_nodelist` table created in DecentDB schema migration
- [ ] All tests pass
- [ ] Rustdoc complete
- [ ] `dev-check.sh` passes cleanly

---

## Phase 8: Netmail routing

### Prerequisites

- Phase 1 complete (FtnAddress parsing)
- Phase 4 complete (tosser processes netmail)
- Phase 5 complete (scanner creates netmail packets)
- Phase 7 complete (nodelist lookup for routing)

### Objectives

Implement netmail routing: determine how to route a netmail message to its destination, including direct routing, hub routing, crash mail, and hold mail.

### Deliverables

#### `NetmailRouter`

```rust
struct NetmailRouter {
    local_address: FtnAddress,
    links: Vec<FtnLinkConfig>,
    nodelist: Arc<dyn NodelistIndex>,
}

impl NetmailRouter {
    fn route(&self, dest: &FtnAddress, attribute: MessageAttribute) -> RoutingDecision;
}
```

#### `RoutingDecision` enum

```rust
enum RoutingDecision {
    Direct {
        link: FtnLinkConfig,
    },
    RoutedViaHub {
        hub: FtnLinkConfig,
        final_destination: FtnAddress,
    },
    Hold {
        link: FtnLinkConfig,
    },
    Crash {
        link: FtnLinkConfig,
    },
    UnknownDestination,
    LocalDelivery,
}
```

### Routing rules

1. If the destination matches our local address, deliver locally (LocalDelivery).
2. If the destination matches a direct link, route directly (Direct).
3. If we have a hub link in the destination's zone, route via hub (RoutedViaHub).
4. If no route is found, return UnknownDestination.
5. If the message has the `Crash` attribute, attempt direct connection (Crash).
6. If the message has the `Hold` attribute, hold for pickup (Hold).
7. If the destination is in the same zone and we have a direct link, route directly.
8. If the destination is in a different zone, route via our zone's hub for that zone.

### Technical details

- Netmail routing is hierarchical: local → direct link → hub → unknown.
- Hub routing means the message is addressed to the hub, with the final destination in the INTL kludge.
- Crash mail bypasses the normal poll schedule and triggers an immediate connection.
- Hold mail is placed in the outbound queue but not sent until the destination polls us.
- The `SENT` attribute is set by the scanner when a netmail message has been successfully sent.
- The `RECEIVED` attribute is set by the tosser when a netmail message has been read by the recipient.

### Tests required

- Route netmail to a known direct link — verify Direct routing
- Route netmail to a hub-routed address — verify RoutedViaHub with correct hub
- Route netmail to our own address — verify LocalDelivery
- Route netmail to an unknown address — verify UnknownDestination
- Route crash netmail — verify Crash routing with immediate connection flag
- Route hold netmail — verify Hold routing
- Route netmail across zones — verify zone-gateway routing
- Verify INTL, FMPT, TOPT kludges are set correctly for hub-routed netmail

### Documentation required

- Rustdoc on `NetmailRouter` and `RoutingDecision`
- ADR for routing strategy (direct vs. hub-routed vs. crash vs. hold)
- Netmail routing guide in `docs/ftn/netmail-routing.md`

### Definition of done

- [ ] `NetmailRouter` implements routing rules
- [ ] `RoutingDecision` covers all routing scenarios
- [ ] Direct, hub-routed, crash, and hold routing work correctly
- [ ] Local delivery detection works
- [ ] Unknown destinations are handled gracefully
- [ ] All tests pass
- [ ] Rustdoc complete
- [ ] ADR written for netmail routing strategy
- [ ] `dev-check.sh` passes cleanly

---

## Phase 9: AreaFix

### Prerequisites

- Phase 4 complete (tosser processes netmail)
- Phase 8 complete (netmail routing)

### Objectives

Implement AreaFix: automated echomail area subscription management via netmail.

### Deliverables

#### AreaFix protocol

AreaFix is a netmail-based robot that processes commands sent to the `AreaFix` or `Areafix` username at our FTN address. Commands are sent as netmail messages with a subject line containing the password.

Supported commands:

| Command | Description |
|---|---|
| `%LIST` | List all available areas |
| `%QUERY` | List areas the sender is subscribed to |
| `%HELP` | Return help text |
| `+AREA.TAG` | Subscribe to an area |
| `-AREA.TAG` | Unsubscribe from an area |
| `+AREA.TAG !` | Subscribe to an area and request rescan |

The subject line of the netmail message is the AreaFix password for the link.

#### `AreaFixProcessor`

```rust
struct AreaFixProcessor {
    db: Arc<OxideDb>,
    local_address: FtnAddress,
    links: Vec<FtnLinkConfig>,
    areas: HashMap<String, FtnAreaConfig>,
}
```

Processes inbound netmail addressed to `AreaFix` and generates reply netmail.

#### AreaFix reply format

Replies are sent as netmail back to the requesting address. The reply body contains:

```text
AreaFix response for [sender address]:

+AREA.TAG - subscribed
-AREA.TAG - unsubscribed

Errors:
Unknown area: AREA.NOTFOUND
Already subscribed: AREA.ALREADY
```

### Technical details

- AreaFix messages must be authenticated by the subject line (password) matching the link's configured password.
- Commands are case-insensitive.
- `%LIST`, `%QUERY`, `%HELP` are management commands that return information.
- `+AREA.TAG` subscribes the link to the specified area.
- `-AREA.TAG` unsubscribes the link.
- `+AREA.TAG !` subscribes and requests a rescan of recent messages (up to a configurable limit).
- Rescan sends all messages from the area that were imported in the last N days (default: 30).
- The `network_area_subscriptions` table tracks which links are subscribed to which areas.
- All AreaFix activity is logged to `network_poll_log` or a separate audit log.

### Tests required

- Process `%LIST` command — verify response lists all available areas
- Process `%QUERY` command — verify response lists only subscribed areas
- Process `+AREA.TAG` command — verify subscription is created in `network_area_subscriptions`
- Process `-AREA.TAG` command — verify subscription is removed
- Process `+AREA.TAG !` command — verify subscription and rescan
- Process command with wrong password — verify rejection
- Process command for unknown area — verify error response
- Process command for already-subscribed area — verify appropriate response
- Verify AreaFix replies are sent as netmail
- Verify AreaFix activity is logged

### Documentation required

- Rustdoc on `AreaFixProcessor`
- AreaFix command reference in `docs/ftn/areafix.md`

### Definition of done

- [ ] `AreaFixProcessor` handles all commands (%LIST, %QUERY, %HELP, +AREA, -AREA, rescan)
- [ ] Password authentication works
- [ ] Subscriptions are created and removed in DecentDB
- [ ] Rescan sends recent messages
- [ ] Reply netmail is generated correctly
- [ ] All tests pass
- [ ] Rustdoc complete
- [ ] `dev-check.sh` passes cleanly

---

## Phase 10: BinkP transport

### Prerequisites

- Phase 5 complete (scanner creates outbound packets)
- Phase 6 complete (bundle format)
- Async runtime available (tokio)

### Objectives

Implement the BinkP protocol for TCP/IP-based network mail exchange. Create the `oxidebbs-binkp` crate.

### Deliverables

#### BinkP protocol implementation

BinkP is a TCP/IP protocol for exchanging packet and bundle files. Legacy FTN
uses it for `.pkt` and arcmail bundles; OxideNet may use the same transport for
its internal packet files. This is the FTN/FidoNet mail transport for v1.2.
Caller file-area transfer protocols such as ZMODEM and XMODEM-CRC live in
`oxidebbs-transfer` and are not used for BinkP polling. YMODEM is outside v1.2
caller-transfer scope. The protocol uses a series of frames:

| Frame | Description |
|---|---|
| M_NUL (0) | Informational text (system name, timezone, etc.) |
| M_ADR (1) | FTN address(es) of the sender |
| M_PWD (2) | Session password |
| M_FILE (3) | File offer (name, size, time) |
| M_OK (4) | Password accepted |
| M_EOB (5) | End of batch |
| M_GOT (6) | File received confirmation |
| M_ERR (7) | Fatal error |
| M_BSY (8) | Busy / try later |
| M_GET (9) | Request/resume file from offset |
| M_SKIP (10) | Non-destructive skip; try in a later session |

Frame format follows FSP-1011:

```text
u16 header | data[header & 0x7fff]
```

The two-octet header is read in network byte order. Its high bit marks the frame type. If the high bit is `0`, the frame is a data frame and the payload is file data. If the high bit is `1`, the frame is a command frame and the first payload byte is the command ID (`0..127`), followed by optional command arguments. The lower 15 bits are the payload size, so one frame carries at most 32767 bytes of data.

#### `BinkpClient` (outbound poller)

```rust
struct BinkpClient {
    config: BinkpClientConfig,
    tls_config: TlsConfig,
}

impl BinkpClient {
    async fn poll(&self, link: &NetworkLinkConfig) -> Result<PollResult, BinkpError>;
    async fn poll_dry_run(&self, link: &NetworkLinkConfig) -> Result<(), BinkpError>;
}
```

Connects to a remote BinkP server, authenticates, sends outbound files, receives inbound files, and disconnects.

#### `BinkpServer` (inbound listener)

```rust
struct BinkpServer {
    config: BinkpServerConfig,
    tls_config: TlsConfig,
    links: Vec<NetworkLinkConfig>,
}
```

Listens for inbound BinkP connections, authenticates callers, receives their files, sends our files, and disconnects.

#### Transport security policy

- OxideNet and new private-network profiles require TLS by default (via `tokio-rustls`).
- Legacy FTN links may use plaintext BinkP only when the link explicitly sets `transport_security = "plaintext_legacy"`.
- Plaintext legacy mode must produce a startup warning and a poll-log warning because reusable BinkP passwords and message content are exposed.
- `transport_security = "tls_opportunistic"` may be added later for deployments that can attempt TLS first and fall back for known legacy peers, but v1 should prefer either strict TLS or explicit plaintext legacy mode.

#### Poll result

```rust
struct PollResult {
    files_sent: usize,
    files_received: usize,
    bytes_sent: u64,
    bytes_received: u64,
    duration: Duration,
    errors: Vec<BinkpError>,
}
```

### Technical details

- The BinkP protocol is stateful and full-duplex. Both sides can send command frames and data frames during the file-transfer stage.
- M_NUL frames carry system information (system name, location, sysop name, timezone) and should be sent before authentication.
- M_ADR carries one or more FTN addresses separated by spaces.
- M_PWD carries the session password. BinkP passwords are case-sensitive at the BinkP layer; legacy packet passwords remain case-insensitive where required by FTN packet convention.
- M_FILE offers a file. The receiver acknowledges with M_GOT when the file has been received completely, or M_SKIP to refuse it.
- M_EOB marks end-of-batch. A session completes after both sides have finished sending files and pending M_GOT/M_SKIP acknowledgements have been processed.
- File transfer within BinkP uses BinkP data frames. After M_FILE, subsequent
  data frames carry bytes for that file until the advertised size is reached;
  there is no separate end-of-file frame. This is not XMODEM, YMODEM, ZMODEM,
  ZedZap, Hydra, or any caller-facing transfer protocol.
- M_GET supports resume from an offset. Basic v1 may reject resume with a clear error, but the frame parser must understand M_GET.
- The client should support retry with exponential backoff on connection failure.
- The client should log all poll activity to `network_poll_log`.
- The server should reject connections from unknown addresses with M_ERR.
- The server should handle concurrent connections (one per link at a time).
- BinkP default port is 24554.

### Tests required

- Client connects to a test server and authenticates successfully
- Client connection fails with wrong password — verify M_ERR response
- Client sends an outbound file and receives an inbound file
- File data is sent and received as BinkP data frames, not unframed bytes
- Session sends and receives M_EOB at end-of-batch
- M_GET is parsed and handled or rejected explicitly
- Server accepts an inbound connection from a known link
- Server rejects an inbound connection from an unknown address
- Server sends outbound files to a connected client
- TLS handshake succeeds with valid certificates
- TLS handshake fails with invalid certificates — verify connection rejection
- Retry with exponential backoff after connection failure
- Concurrent connections from multiple links
- Large file transfer (> 1 MB) completes successfully
- Empty poll (no files to send or receive) completes gracefully
- Session is terminated gracefully after M_EOB and pending acknowledgements complete
- Poll activity is logged to `network_poll_log`
- Dry-run poll connects and disconnects without transferring files

### Documentation required

- Rustdoc on `BinkpClient`, `BinkpServer`, all frame types
- BinkP protocol reference in `docs/ftn/binkp.md`
- ADR for profile-aware BinkP transport security
- Configuration reference for BinkP settings

### Definition of done

- [ ] `oxidebbs-binkp` crate created in workspace
- [ ] BinkP frame parser and writer implemented
- [ ] Command frames and data frames use the FSP-1011 high-bit/15-bit-length header
- [ ] `BinkpClient` connects, authenticates, sends and receives files
- [ ] `BinkpServer` accepts connections, authenticates, sends and receives files
- [ ] TLS is enabled by default for OxideNet/private profiles, plaintext legacy FTN requires explicit per-link opt-in
- [ ] Retry with exponential backoff works
- [ ] Poll activity logged to `network_poll_log`
- [ ] All tests pass
- [ ] Rustdoc complete
- [ ] ADR written for BinkP transport security policy
- [ ] `dev-check.sh` passes cleanly

---

## Phase 11: CLI commands and operational tooling

### Prerequisites

- Phase 4 complete (tosser)
- Phase 5 complete (scanner)
- Phase 7 complete (nodelist)
- Phase 9 complete (AreaFix)
- Phase 10 complete (BinkP)

### Objectives

Provide CLI commands for sysops to manage FTN networking.

### Deliverables

CLI commands added to `oxidebbs-server` using `clap` derive subcommands.

```bash
# Toss inbound packets
oxidebbs net toss [network]

# Scan outbound messages
oxidebbs net scan [network]

# Poll a specific link
oxidebbs net poll <link-name>
oxidebbs net poll --all
oxidebbs net poll --dry-run <link-name>

# Check status
oxidebbs net status [network]
oxidebbs net queue <link-name>

# Nodelist
oxidebbs net nodelist import <file>
oxidebbs net nodelist lookup <address>
oxidebbs net nodelist count

# Area management
oxidebbs net areas list [network]
oxidebbs net areas subscribe <area-tag> <link-name>
oxidebbs net areas unsubscribe <area-tag> <link-name>

# Link management
oxidebbs net links list
oxidebbs net links show <link-name>

# Packets
oxidebbs net packets inbound
oxidebbs net packets outbound
oxidebbs net packets quarantine

# AreaFix
oxidebbs net areafix send <link-name> <command>

# Logs
oxidebbs net logs [link-name] [--limit N]
```

### Tests required

- Each CLI command runs without error
- `net toss` processes inbound packets and logs results
- `net scan` creates outbound packets
- `net poll --dry-run` connects and disconnects without transferring
- `net status` shows network and link status
- `net nodelist import` parses a nodelist file and stores entries
- `net nodelist lookup` returns correct entries
- `net areas list` shows subscribed areas
- Error messages are clear and actionable

### Documentation required

- CLI reference in `docs/ftn/cli.md`
- Sysop guide for daily operations in `docs/ftn/sysop-guide.md`

### Definition of done

- [ ] All CLI commands are implemented and functional
- [ ] Each command has `--help` output
- [ ] Commands integrate with tosser, scanner, nodelist, and BinkP
- [ ] Error messages are clear
- [ ] All tests pass
- [ ] Documentation complete
- [ ] `dev-check.sh` passes cleanly

---

## Phase 12: Integration testing and hardening

### Prerequisites

- All previous phases complete

### Objectives

End-to-end integration testing, stress testing, and operational hardening.

### Deliverables

#### End-to-end test suite

Tests that exercise the complete toss/scan/poll cycle:

1. **Full toss/scan cycle:** Compose an echomail message locally, scan it into an outbound packet, copy the packet to an inbound directory, toss it into a local area, verify the message appears.
2. **Cross-network poll simulation:** Set up two in-process OxideBBS instances, have one poll the other via BinkP over localhost, exchange echomail, verify both sides have the messages.
3. **Netmail round-trip:** Send a netmail message from one system to another, verify delivery.
4. **AreaFix round-trip:** Send AreaFix commands, verify subscriptions change.
5. **Nodelist import and lookup:** Import a full nodelist, verify lookups.
6. **Duplicate detection:** Toss the same packet twice, verify duplicates are rejected.
7. **Malformed packet handling:** Toss a packet with deliberate corruption, verify quarantine.
8. **Netmail routing:** Route netmail through a hub, verify routing decisions.
9. **Bundle creation and extraction:** Create a bundle, extract it, verify the packet contents.
10. **BinkP authentication:** Connect with correct and incorrect passwords.

#### Stress testing

- Process a packet containing 1000 messages
- Process 100 packets in a single toss run
- Handle concurrent inbound and outbound operations
- Handle a nodelist with 50,000 entries

#### Operational hardening

- Quarantine dashboard (CLI and sysop TUI)
- Poll failure logging and alerting
- Packet retention policy (archive after N days, delete after M days)
- Stats collection (messages tossed, messages scanned, polls succeeded, polls failed)
- Configurable log levels for tosser, scanner, and BinkP

### Tests required

- All end-to-end tests pass
- Stress tests complete without memory leaks or panics
- Quarantine dashboard shows quarantined packets
- Retention policy archives and deletes old packets
- Stats are collected and queryable

### Documentation required

- Integration test documentation in `docs/ftn/testing.md`
- Troubleshooting guide in `docs/ftn/troubleshooting.md`
- Performance characteristics in `docs/ftn/performance.md`

### Definition of done

- [ ] End-to-end test suite passes
- [ ] Stress tests pass without leaks or panics
- [ ] Quarantine dashboard works
- [ ] Retention policy is configurable
- [ ] Stats collection is implemented
- [ ] Log levels are configurable
- [ ] All tests pass
- [ ] Documentation complete
- [ ] `dev-check.sh` passes cleanly

---

## Phase 13: Documentation

### Prerequisites

- All previous phases complete

### Objectives

Complete documentation for sysops, developers, and FTN network operators.

### Deliverables

#### Developer documentation

| Document | Purpose |
|---|---|
| `docs/ftn/architecture.md` | Crate layout, module structure, data flow |
| `design/MAILER.md` | Built-in mailer boundaries, BinkP runtime model, spool layout |
| `docs/ftn/packet-format.md` | Type-2/Type-2+ packet format reference |
| `docs/ftn/echomail.md` | Echomail message format, kludge reference |
| `docs/ftn/netmail.md` | Netmail message format, routing |
| `docs/ftn/bundles.md` | Arcmail bundle format, naming convention |
| `docs/ftn/nodelist.md` | Nodelist format, import process, lookup |
| `docs/ftn/binkp.md` | BinkP protocol reference |
| `docs/ftn/tosser.md` | Tosser workflow, configuration |
| `docs/ftn/scanner.md` | Scanner workflow, configuration |
| `docs/ftn/netmail-routing.md` | Netmail routing rules |
| `docs/ftn/areafix.md` | AreaFix command reference |
| `docs/ftn/testing.md` | Integration test documentation |
| `docs/ftn/troubleshooting.md` | Common problems and solutions |
| `docs/ftn/performance.md` | Performance characteristics and tuning |

#### Sysop documentation

| Document | Purpose |
|---|---|
| `docs/ftn/setup.md` | Step-by-step FTN network setup guide |
| `docs/ftn/sysop-guide.md` | Daily operations: toss, scan, poll |
| `docs/ftn/cli.md` | CLI command reference |
| `docs/ftn/configuration.md` | Full configuration reference for `[ftn]` and subsections |
| `docs/ftn/joining-a-network.md` | How to join a real FTN network (FidoNet, fsxNet, etc.) |

#### ADRs

| ADR | Topic |
|---|---|
| FTN packet format choice | Type-2+ as default output, Type-2 and Type-2+ input |
| Kludge handling strategy | Tolerant parsing, strict composition |
| Duplicate detection strategy | MSGID-primary, hash-fallback |
| Outbound MSGID generation | Random hex vs. content hash vs. counter |
| Netmail routing strategy | Direct, hub-routed, crash, hold |
| BinkP transport security | TLS required for OxideNet/private profiles, explicit plaintext legacy mode for real FTN interop |
| Bundle compression | ZIP for v1, ARJ deferred |
| Nodelist differential updates | Full nodelist only for v1, incremental deferred |

### Definition of done

- [ ] All developer documentation is written and reviewed
- [ ] All sysop documentation is written and reviewed
- [ ] All ADRs are written
- [ ] Configuration reference covers all `[ftn]` settings
- [ ] Setup guide walks through joining a real FTN network
- [ ] CLI reference covers all commands

---

## Overall definition of done

The FTN networking implementation is complete when:

1. OxideBBS can toss inbound `.pkt` files (Type-2 and Type-2+) containing echomail and netmail messages into local areas.
2. OxideBBS can scan local echomail areas and create outbound `.pkt` files with proper SEEN-BY, PATH, tear, and origin lines.
3. Duplicate detection prevents the same message from being imported twice.
4. OxideBBS can create and extract arcmail bundles (ZIP format).
5. OxideBBS can parse and index standard nodelist files.
6. Netmail routing handles direct, hub-routed, crash, and hold scenarios.
7. AreaFix processes subscription commands via netmail.
8. BinkP transport works with TLS-required profiles and explicit plaintext legacy FTN links.
9. CLI commands allow the sysop to toss, scan, poll, manage areas, and view status.
10. All tests pass: `cargo test --workspace --locked`.
11. All lints pass: `cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings`.
12. All checks pass: `./scripts/dev-check.sh`.
13. Rustdoc is complete on all public types in `oxidebbs-ftn` and `oxidebbs-binkp`.
14. All ADRs are written and reviewed.
15. All documentation is written.

---

## Phase dependency graph

```text
Phase 0: Shared network model, schema, and config
  |
  +-- Phase 1: Packet format
        |
        +-- Phase 2: Echomail message model
        |     |
        |     +-- Phase 3: Duplicate detection
        |           |
        |           +-- Phase 4: Tosser (inbound)
        |                 |
        |                 +-- Phase 5: Scanner (outbound)
        |                       |
        |                       +-- Phase 6: Bundle format
        |                       +-- Phase 8: Netmail routing
        |                             |
        |                             +-- Phase 9: AreaFix
        |
        +-- Phase 7: Nodelist

Phase 10: BinkP transport (depends on Phase 5, Phase 6)
Phase 11: CLI commands (depends on Phase 4, 5, 7, 9, 10)
Phase 12: Integration testing (depends on all)
Phase 13: Documentation (depends on all)
```

Multiple phases can be worked on in parallel once their prerequisites are complete:

- Phase 7 (Nodelist) can be done in parallel with Phase 2-5.
- Phase 6 (Bundle format) can be done in parallel with Phase 4 (Tosser).
- Phase 8 (Netmail routing) can begin after scanner netmail packet creation and nodelist lookup interfaces exist.
- Phase 9 (AreaFix) can begin after netmail routing exists, because replies are netmail.

---

## FTN standards compliance summary

OxideBBS will implement the following FTN standards to enable participation in real FTN networks:

| Standard | Phase | Notes |
|---|---|---|
| FTS-0001 NetMail session | Phase 10 | Superseded by BinkP for TCP/IP |
| FTS-0005 Echomail | Phase 2, 4, 5 | AREA tag, SEEN-BY, tear, origin |
| FTS-4000 Nodelist | Phase 7 | Full nodelist parsing |
| FSC-0039 Type-2+ packets | Phase 1 | Capability word, extended addressing |
| FSC-0048 4D addressing | Phase 1 | Zone, point fields in packet header |
| FSC-0053 Type-2.2 packets | Phase 1 (read) | Read support, write Type-2+ |
| FSC-0056 NetMail attributes | Phase 4, 5, 8 | Crash, Hold, Sent, Received, etc. |
| FSC-0068 SEEN-BY/PATH | Phase 2, 4, 5 | 2D and 3D address format |
| FSC-0074 Extended SEEN-BY | Phase 2 | Zone in SEEN-BY (read support) |
| FSC-0087 MSGID | Phase 2, 3 | Unique message identification |
| FSC-0091 REPLYID | Phase 2 | Reply threading |
| FSC-0115 INTL/FMPT/TOPT | Phase 2, 5, 8 | Netmail inter-zone routing |
| Arcmail bundle format | Phase 6 | ZIP compression, day-of-week naming |
| BinkP protocol | Phase 10 | TCP/IP mail exchange |


## Reference

Official FidoNet FTSC Documents: http://ftsc.org/docs/
