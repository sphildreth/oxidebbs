# DecentDB Schema

OxideBBS uses DecentDB as the only system database. The schema should lean into
DecentDB's PostgreSQL-like type system instead of treating it as a SQLite-style
string store.

Current schema version: `8`

Schema version `8` is the current v1.2 development schema. The P2 foundation
landed in schema `5`; later v1.2 phases added caller door-security storage in
schema `6`, file-transfer storage tables in schema `7`, and OxideNet registry
tables in schema `8`. The initializer
upgrades supported older development schemas and keeps development upgrades safe:

- schema `2 -> 3` is migratable. The migration adds `message_areas.enabled` with
  default `TRUE`, preserves message rows and reply links, then updates
  `system_config.schema_version` to `3`.
- schema `3 -> 4` is migratable. The migration backfills
  `users.alias_normalized`, rebuilds user-related foreign-key tables so new rows
  reference the v4 `users` table, creates `auth_attempts`, and updates
  `system_config.schema_version` to `4`.
- schema `4 -> 5` is migratable. The migration rebuilds `messages` so
  `author_user_id` is nullable and external author metadata is first-class,
  backfills existing local rows with `author_kind = 'local'` and
  `author_display_name` from the referenced user alias, creates the shared
  `network_*` tables, and updates `system_config.schema_version` to `5`.
- schema `5 -> 6` is migratable. The migration rebuilds `doors` and
  `door_runs`, adds `doors.min_security_level`, backfills existing doors with
  `0`, and updates `system_config.schema_version` to `6`.
- schema `6 -> 7` is migratable. The migration creates `file_areas`,
  `file_entries`, and `file_transfers`, then updates
  `system_config.schema_version` to `7`.
- schema `7 -> 8` is migratable. The migration creates
  `network_applications`, `network_nodes`, and `network_credentials` for the
  OxideNet application/node registry, then updates `system_config.schema_version`
  to `8`.
- the pinned DecentDB rejects direct `ALTER TABLE ... ADD COLUMN` on checked
  tables, so migrations use table-rebuild strategies. Renamed pre-upgrade
  tables are retained under `oxidebbs_schema*_` archive names where DecentDB
  cannot safely drop self-referencing or foreign-key tables.
- if the schema marker is missing, malformed, or absent on an existing database,
  OxideBBS reports a clear error.
- newer schema versions are rejected until this software understands them.

Open and startup flow:

- `db init` and `serve` use the same schema opener and will attempt migration
  only from known previous versions.
- migration is blocked on missing or malformed existing schema markers, forcing
  explicit operator intervention.
- schema migrations do table rebuilds for DecentDB compatibility and record the
  new marker only after successful validation of rebuilt tables.
- `serve` performs a startup health check after opening DecentDB and before
  binding telnet: it reads the schema marker and the core user, auth, message,
  session, door, door-run, and audit tables, then requires the startup audit
  events to be writable. Failure in those checks blocks startup.

## Restore and Compaction Semantics

- `db import --format json <path>` is a full restore for the tables currently
  represented in the JSON import/export payload. It expects a schema `8` payload
  and fails fast on schema mismatch or malformed foreign-key references.
- Restore targets must be schema-only: existing rows are only allowed in
  `system_config` for the `schema_version` marker.
- Restore order is dependency-aware for the covered tables:
  `users -> auth_attempts -> message_areas -> messages -> network_profiles -> network_links -> network_areas -> network_packets -> network_messages -> network_seen_by -> network_path -> network_duplicate_log -> network_poll_log -> network_area_subscriptions -> network_nodelist -> network_applications -> network_nodes -> network_credentials -> sessions -> doors -> door_runs -> audit_events`.
- Schema `7` file-transfer tables are initialized and verified by database
  diagnostics, but transfer import/export rows remain part of the file-transfer
  implementation work rather than P2 schema/config foundation.
- Schema `8` OxideNet registry tables are exported/imported by the database
  backup JSON path. Credentials persist only `secret_hash`, never plaintext
  session passwords or invite tokens.
- Restores are executed inside one DecentDB transaction; validation is complete before
  any rows are written.
- `db compact --output <path> [--overwrite]` checkpoints the active DecentDB,
  saves a compacted copy to a separate output file, evicts the output shared WAL,
  and verifies the compacted database before reporting success. It refuses the
  active database path so replacement remains an offline operator action.

## Type Rules

- Entity identifiers use `UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()`.
- Lifecycle timestamps use `TIMESTAMPTZ`, not ad hoc text.
- Session peer IPs use `IPADDR`; the display endpoint is retained as text
  because it includes a port.
- Boolean flags use `BOOL`.
- Counters, levels, node numbers, ports, and exit codes use `INT` with `CHECK`
  constraints where the domain is bounded.
- External identifiers, display names, paths, commands, and ANSI/BBS text remain
  `TEXT`.
- Menu/status-like fields currently use `TEXT CHECK (...)` instead of DecentDB
  `ENUM(...)` so the repository API can continue to return stable labels without
  exposing internal enum label ids.

## Tables

### system_config

```sql
key TEXT PRIMARY KEY
value TEXT NOT NULL
updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
```

### users

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
alias TEXT NOT NULL UNIQUE
alias_normalized TEXT NOT NULL UNIQUE
real_name TEXT NOT NULL
email TEXT
password_hash TEXT NOT NULL
security_level INT NOT NULL DEFAULT 10
is_sysop BOOL NOT NULL DEFAULT FALSE
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
last_login_at TIMESTAMPTZ
total_calls INT NOT NULL DEFAULT 0
time_bank_minutes INT NOT NULL DEFAULT 0
status TEXT NOT NULL DEFAULT 'active'
```

Constraints:

- aliases and real names must not be blank
- `alias_normalized` is `LOWER(TRIM(alias))` and enforces case-insensitive
  uniqueness
- security levels are `0..255`
- counters cannot be negative
- status is `active`, `locked`, or `disabled`

### auth_attempts

```sql
scope TEXT NOT NULL
scope_key TEXT NOT NULL
failed_count INT NOT NULL DEFAULT 0
first_failed_at TIMESTAMPTZ
last_failed_at TIMESTAMPTZ
locked_until TIMESTAMPTZ
PRIMARY KEY (scope, scope_key)
```

Constraints:

- scope is `ip` or `alias`
- scope keys must not be blank
- failed counts cannot be negative

### audit_events

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
event_type TEXT NOT NULL
user_id UUID REFERENCES users(id) ON DELETE SET NULL
node_number INT
details TEXT NOT NULL DEFAULT ''
```

Indexes:

- `created_at`
- `user_id`

Runtime audit inserts generate `id` and `created_at` inside the `INSERT`
statement with DecentDB functions. Import/restore paths use a separate preserving
insert so backup JSON can restore original audit identifiers and timestamps.
Audit retention is not enforced on every insert; the repository exposes a
retention purge helper for scheduled maintenance based on `[audit].retention_days`
(`365` by default).

### message_areas

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
key TEXT NOT NULL UNIQUE
name TEXT NOT NULL
description TEXT NOT NULL DEFAULT ''
kind TEXT NOT NULL DEFAULT 'local'
network_id TEXT
read_security_level INT NOT NULL DEFAULT 0
post_security_level INT NOT NULL DEFAULT 10
moderated BOOL NOT NULL DEFAULT FALSE
enabled BOOL NOT NULL DEFAULT TRUE
```

Constraints:

- key and name must not be blank
- kind is `local`, `echomail`, or `netmail`
- security levels are `0..255`

### messages

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
area_id UUID NOT NULL REFERENCES message_areas(id) ON DELETE CASCADE
author_user_id UUID REFERENCES users(id) ON DELETE RESTRICT
to_user_id UUID REFERENCES users(id) ON DELETE SET NULL
subject TEXT NOT NULL
body TEXT NOT NULL
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
reply_to_id UUID REFERENCES messages(id) ON DELETE SET NULL
network_message_id TEXT
author_kind TEXT NOT NULL DEFAULT 'local'
author_display_name TEXT NOT NULL DEFAULT ''
author_network_address TEXT
visibility TEXT NOT NULL DEFAULT 'normal'
```

Constraints:

- subject must not be blank
- author kind is `local`, `network`, or `system`
- local rows use `author_user_id`; network rows may leave it `NULL` and use
  `author_display_name` plus `author_network_address`
- visibility is `normal`, `deleted`, or `hidden`

Indexes:

- `(area_id, created_at)`
- `author_user_id`
- `author_kind`
- `to_user_id`

Caller message reads use a SQL-side visibility query that joins
`message_areas` and filters disabled areas, area read security, and non-normal
message visibility before rows are returned to the server.

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

Constraints:

- key and name must not be blank
- adapter is `legacy-ftn` or `oxidenet`
- local zone, net, and node are positive
- local point cannot be negative

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
compression TEXT NOT NULL DEFAULT 'zip'
transport_security TEXT NOT NULL DEFAULT 'tls_required'
enabled BOOL NOT NULL DEFAULT TRUE
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
```

Constraints:

- key, address, and host must not be blank
- BinkP ports are `1..65535`
- poll intervals are positive
- compression is `none`, `zip`, or `arj`
- transport security is `tls_required`, `tls_opportunistic`, or
  `plaintext_legacy`

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
UNIQUE (network_id, area_tag)
UNIQUE (network_id, local_area_id)
```

Constraints:

- area tags must not be blank
- one network profile maps a tag to one local message area

### network_packets

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE
direction TEXT NOT NULL
link_id UUID REFERENCES network_links(id) ON DELETE SET NULL
filename TEXT NOT NULL
sha256 TEXT NOT NULL
size_bytes INT NOT NULL DEFAULT 0
status TEXT NOT NULL DEFAULT 'pending'
error_message TEXT
received_at TIMESTAMPTZ
processed_at TIMESTAMPTZ
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
```

Constraints:

- direction is `inbound` or `outbound`
- filenames and SHA-256 values must not be blank
- byte sizes cannot be negative
- status is `pending`, `processing`, `processed`, `quarantined`, or `failed`

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

Constraints:

- message type is `echomail`, `netmail`, or `local`
- origin address, sender name, and subject must not be blank
- raw network text is stored as bytes in `raw_text`; decoded UI/search text is
  stored separately in `display_body`
- status is `imported`, `exported`, `quarantined`, or `duplicate`

### network_seen_by

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
message_id UUID NOT NULL REFERENCES network_messages(id) ON DELETE CASCADE
network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE
zone INT NOT NULL
net INT NOT NULL
node INT NOT NULL
```

Constraints:

- zone, net, and node are positive

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

Constraints:

- path sequence cannot be negative
- zone, net, and node are positive

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

Constraints:

- duplicate hashes must not be blank
- action is `rejected`, `quarantined`, or `replaced`

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

Constraints:

- direction is `inbound`, `outbound`, or `bidirectional`
- status is `started`, `success`, `failed`, or `timeout`
- byte and packet counters cannot be negative

### network_area_subscriptions

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
area_id UUID NOT NULL REFERENCES network_areas(id) ON DELETE CASCADE
link_id UUID NOT NULL REFERENCES network_links(id) ON DELETE CASCADE
subscribed BOOL NOT NULL DEFAULT TRUE
subscribed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
unsubscribed_at TIMESTAMPTZ
source TEXT NOT NULL DEFAULT 'manual'
UNIQUE (area_id, link_id)
```

Constraints:

- source is `manual`, `areafix`, or `default`

### network_nodelist

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE
zone INT NOT NULL
net INT NOT NULL
node INT NOT NULL
point INT NOT NULL DEFAULT 0
parsed_name TEXT
raw_entry TEXT NOT NULL
updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
UNIQUE (network_id, zone, net, node, point)
```

Constraints:

- zone, net, and node are positive
- point cannot be negative
- raw nodelist entries must not be blank

### network_applications

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
submitted_at TIMESTAMPTZ
reviewed_at TIMESTAMPTZ
status TEXT NOT NULL DEFAULT 'submitted'
applicant_user_id UUID REFERENCES users(id) ON DELETE SET NULL
board_name TEXT NOT NULL
sysop_alias TEXT NOT NULL
contact_email TEXT NOT NULL
host TEXT NOT NULL
binkp_port INT NOT NULL DEFAULT 24554
telnet_host TEXT
telnet_port INT
software TEXT NOT NULL DEFAULT 'OxideBBS'
software_version TEXT NOT NULL DEFAULT ''
timezone TEXT NOT NULL DEFAULT 'UTC'
region TEXT NOT NULL DEFAULT ''
description TEXT NOT NULL DEFAULT ''
reason TEXT NOT NULL DEFAULT ''
policy_version TEXT NOT NULL DEFAULT ''
policy_accepted_at TIMESTAMPTZ
admin_notes TEXT NOT NULL DEFAULT ''
reviewed_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL
assigned_address TEXT UNIQUE
```

Constraints:

- blank board names, sysop aliases, contact emails, hosts, software names, and
  timezones are rejected
- ports must be `1..65535`
- status is one of the OxideNet application lifecycle labels

### network_nodes

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
application_id UUID REFERENCES network_applications(id) ON DELETE SET NULL
network_key TEXT NOT NULL DEFAULT 'oxidenet'
address TEXT NOT NULL UNIQUE
zone INT NOT NULL
net INT NOT NULL
node INT NOT NULL
point INT NOT NULL DEFAULT 0
hub_address TEXT NOT NULL
board_name TEXT NOT NULL
sysop_alias TEXT NOT NULL
contact_email TEXT NOT NULL
host TEXT NOT NULL
binkp_port INT NOT NULL DEFAULT 24554
telnet_host TEXT
telnet_port INT
software TEXT NOT NULL DEFAULT 'OxideBBS'
software_version TEXT NOT NULL DEFAULT ''
status TEXT NOT NULL DEFAULT 'first-poll-pending'
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
activated_at TIMESTAMPTZ
suspended_at TIMESTAMPTZ
retired_at TIMESTAMPTZ
last_poll_at TIMESTAMPTZ
last_successful_poll_at TIMESTAMPTZ
flags TEXT NOT NULL DEFAULT ''
```

Constraints:

- addresses and human-facing identity fields must not be blank
- address zone, net, and node are positive; point cannot be negative
- ports must be `1..65535`
- status is `config-generated`, `first-poll-pending`, `active`, `probation`,
  `suspended`, or `retired`

### network_credentials

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
node_id UUID NOT NULL REFERENCES network_nodes(id) ON DELETE CASCADE
credential_kind TEXT NOT NULL
secret_hash TEXT NOT NULL
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
rotated_at TIMESTAMPTZ
expires_at TIMESTAMPTZ
status TEXT NOT NULL DEFAULT 'active'
```

Constraints:

- credentials are scoped to a node and cascade when that node is removed
- `credential_kind` is `binkp_session` or `invite_token`
- `secret_hash` must not be blank; plaintext secrets are not stored
- status is `active`, `revoked`, or `expired`

### sessions

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
node_number INT NOT NULL
user_id UUID REFERENCES users(id) ON DELETE SET NULL
transport TEXT NOT NULL
remote_address TEXT NOT NULL DEFAULT ''
remote_ip IPADDR
remote_port INT
started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
ended_at TIMESTAMPTZ
disconnect_reason TEXT
```

Constraints:

- node numbers are positive
- v1 transport is `telnet`
- remote ports are `0..65535` when present

Indexes:

- `user_id`
- `started_at`

### doors

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
key TEXT NOT NULL UNIQUE
name TEXT NOT NULL
runner TEXT NOT NULL
working_dir TEXT NOT NULL
command TEXT NOT NULL
drop_file TEXT NOT NULL
exclusive BOOL NOT NULL DEFAULT FALSE
time_limit_minutes INT NOT NULL DEFAULT 30
enabled BOOL NOT NULL DEFAULT TRUE
min_security_level INT NOT NULL DEFAULT 0
```

Constraints:

- key, name, runner, working directory, command, and drop file must not be blank
- time limits must be positive
- minimum security levels are `0..255`

### door_runs

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
door_id UUID NOT NULL REFERENCES doors(id) ON DELETE RESTRICT
user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT
node_number INT NOT NULL
started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
ended_at TIMESTAMPTZ
exit_code INT
timed_out BOOL NOT NULL DEFAULT FALSE
disconnect_forced BOOL NOT NULL DEFAULT FALSE
bytes_in INT NOT NULL DEFAULT 0
bytes_out INT NOT NULL DEFAULT 0
```

Constraints:

- node numbers are positive
- byte counters cannot be negative

Indexes:

- `door_id`
- `user_id`
- `started_at`

### file_areas

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
key TEXT NOT NULL UNIQUE
name TEXT NOT NULL
description TEXT NOT NULL DEFAULT ''
root_path TEXT NOT NULL
read_security_level INT NOT NULL DEFAULT 0
download_security_level INT NOT NULL DEFAULT 10
upload_security_level INT NOT NULL DEFAULT 0
max_upload_bytes INT
enabled BOOL NOT NULL DEFAULT TRUE
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
```

Constraints:

- key, name, and root path must not be blank
- security levels are `0..255`
- max upload bytes cannot be negative when present

### file_entries

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
area_id UUID NOT NULL REFERENCES file_areas(id) ON DELETE CASCADE
storage_name TEXT NOT NULL
display_name TEXT NOT NULL
original_name TEXT
size_bytes INT NOT NULL DEFAULT 0
content_crc32 TEXT
description TEXT NOT NULL DEFAULT ''
uploader_user_id UUID REFERENCES users(id) ON DELETE SET NULL
download_count INT NOT NULL DEFAULT 0
approved BOOL NOT NULL DEFAULT FALSE
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
```

Constraints:

- storage and display names must not be blank
- size and download counters cannot be negative

Indexes:

- `area_id`
- `approved`

### file_transfers

```sql
id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID()
node_number INT NOT NULL
user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT
area_id UUID REFERENCES file_areas(id) ON DELETE SET NULL
file_entry_id UUID REFERENCES file_entries(id) ON DELETE SET NULL
direction TEXT NOT NULL
protocol TEXT NOT NULL
requested_name TEXT
storage_name TEXT
declared_size_bytes INT
transferred_payload_bytes INT NOT NULL DEFAULT 0
committed_size_bytes INT
started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
ended_at TIMESTAMPTZ
duration_ms INT
outcome TEXT NOT NULL DEFAULT 'started'
error_code TEXT
error_message TEXT
retry_count INT NOT NULL DEFAULT 0
```

Constraints:

- node numbers are positive
- direction is `download` or `upload`
- protocol is `zmodem` or `xmodem_crc`
- byte counts, durations, and retry counters cannot be negative
- outcome is `started`, `success`, `cancelled`, or `failed`

Indexes:

- `user_id`
- `started_at`

## Planned Tables

These domains are still design-level and should follow the same native-type and
foreign-key rules when implemented:

- `nodes`
