# DecentDB Schema

OxideBBS uses DecentDB as the only system database. The schema should lean into
DecentDB's PostgreSQL-like type system instead of treating it as a SQLite-style
string store.

Current schema version: `4`

Schema version `4` is still pre-alpha. The initializer now upgrades supported
compatible pre-alpha databases and keeps development upgrades safe:

- schema `2 -> 3` is migratable. The migration adds `message_areas.enabled` with
  default `TRUE`, preserves message rows and reply links, then updates
  `system_config.schema_version` to `3`.
- schema `3 -> 4` is migratable. The migration backfills
  `users.alias_normalized`, rebuilds user-related foreign-key tables so new rows
  reference the v4 `users` table, creates `auth_attempts`, and updates
  `system_config.schema_version` to `4`.
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

- `db import --format json <path>` is a full, whole-database restore. It expects a
  schema `4` payload and fails fast on schema mismatch or malformed foreign-key
  references.
- Restore targets must be schema-only: existing rows are only allowed in
  `system_config` for the `schema_version` marker.
- Restore order is dependency-aware:
  `users -> auth_attempts -> message_areas -> messages -> sessions -> doors -> door_runs -> audit_events`.
- Restores are executed inside one DecentDB transaction; validation is complete before
  any rows are written.
- `db compact` is intentionally unsupported in this phase because DecentDB does not
  expose a safe compaction API contract in this release.

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
author_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT
to_user_id UUID REFERENCES users(id) ON DELETE SET NULL
subject TEXT NOT NULL
body TEXT NOT NULL
created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
reply_to_id UUID REFERENCES messages(id) ON DELETE SET NULL
network_message_id TEXT
visibility TEXT NOT NULL DEFAULT 'normal'
```

Constraints:

- subject must not be blank
- visibility is `normal`, `deleted`, or `hidden`

Indexes:

- `(area_id, created_at)`
- `author_user_id`
- `to_user_id`

Caller message reads use a SQL-side visibility query that joins
`message_areas` and filters disabled areas, area read security, and non-normal
message visibility before rows are returned to the server.

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
```

Constraints:

- key, name, runner, working directory, command, and drop file must not be blank
- time limits must be positive

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

## Planned Tables

These domains are still design-level and should follow the same native-type and
foreign-key rules when implemented:

- `nodes`
- `network_config`
- FTN/OxideNet packet import/export tables
