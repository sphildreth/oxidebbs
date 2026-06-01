# DecentDB Schema

OxideBBS uses DecentDB as the only system database. The schema should lean into
DecentDB's PostgreSQL-like type system instead of treating it as a SQLite-style
string store.

Current schema version: `2`

Schema version `2` is still pre-alpha. The initializer refuses to open a
database with an older OxideBBS schema marker instead of silently running against
stale tables. Until migrations exist, recreate development databases when the
schema version changes.

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
- security levels are `0..255`
- counters cannot be negative
- status is `active`, `locked`, or `disabled`

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
