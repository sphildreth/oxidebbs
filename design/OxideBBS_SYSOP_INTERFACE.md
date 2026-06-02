# OxideBBS Sysop Interface

## Purpose

The sysop interface is the local administrative surface for running, inspecting, and maintaining an OxideBBS instance.

For v1, the sysop interface should be **CLI-first**.

A local Ratatui-based TUI is desirable, but it should come after the core server, telnet sessions, users, local messages, DecentDB persistence, and basic door launching are stable.

## Design goals

1. Make common sysop tasks obvious.
2. Keep v1 operationally simple.
3. Avoid requiring a web admin panel.
4. Preserve the retro BBS feel without making administration painful.
5. Make door troubleshooting easy.
6. Make DecentDB health and backups easy to reason about.
7. Make the future Ratatui console a wrapper around the same command/service layer, not a separate admin system.

## Interface layers

OxideBBS should have three admin layers over time:

```text
v1      CLI admin commands
v1.x    Local Ratatui sysop console
v2+     Optional read-only status web dashboard, if desired
```

The CLI should be the source of truth. The TUI should call the same underlying services.

## Command shape

V1 binary name:

```bash
oxidebbs-server
```

Separate helper crate/binary area:

```bash
oxidebbs-sysop
```

Recommended v1 shape:

```bash
oxidebbs-server <command> [options]
```

Global options:

```bash
-c, --config <PATH>      Path to config file
--data <PATH>            Override DecentDB data path
--json                   Output machine-readable JSON where supported
--no-color               Disable colored local terminal output
-v, --verbose            Increase local log verbosity
```

`--json` outputs are intentionally stable objects for automation. The hardening
phase normalizes top-level JSON responses for `status`, `users list`, `nodes
list`, `messages areas list`, `doors list`, and `db stats`.

## Essential v1 command groups

```text
oxidebbs-server serve
oxidebbs-server setup
oxidebbs-server check
oxidebbs-server status
oxidebbs-server users ...
oxidebbs-server nodes ...
oxidebbs-server messages ...
oxidebbs-server doors ...
oxidebbs-server ansi ...
oxidebbs-server db ...
oxidebbs-server logs ...
```

Canonical top-level order is currently:

```text
ansi
audit
check
config
db
doors
logs
messages
nodes
serve
setup
status
sysop
users
```
with Clap help appended after the command list.

## Essential v1 commands

### 1. Start the BBS

```bash
oxidebbs-server serve --config config/oxidebbs.toml
```

Starts the telnet server and runs the board.

Required behavior:

- Load config
- Open DecentDB
- Initialize runtime directories
- Load ANSI assets
- Load door definitions
- Start telnet listener
- Start session/node manager
- Open DecentDB repository services for the v1 direct-write model; a dedicated
  DbWriter remains deferred unless write contention emerges
- Log startup details

Useful options:

```bash
oxidebbs-server serve --config config/oxidebbs.toml
oxidebbs-server serve --bind 0.0.0.0:2323  # explicit plaintext public bind
oxidebbs-server serve --dry-run
```

### 2. Setup a new board

```bash
oxidebbs-server setup
```

Creates a new local OxideBBS instance.

Required behavior:

- Create directory structure
- Create example config
- Create DecentDB file
- Create initial sysop account
- Install default ANSI screens
- Create default message areas
- Create default node configuration

Suggested interactive flow:

```text
Board name:
Sysop alias:
Sysop password:
Telnet port [2323]:
Node count [4]:
Create sample ANSI screens? [Y/n]:
```

Non-interactive option:

```bash
oxidebbs-server setup --board-name "My BBS" --sysop-alias sysop --nodes 4
```

### 3. Validate configuration

```bash
oxidebbs-server check
```

Required behavior:

- Parse config
- Validate paths
- Validate telnet bind address
- Validate DecentDB path
- Validate node count
- Validate ANSI asset paths
- Validate door definitions
- Validate runtime directory permissions
- Report warnings and errors clearly

This should be one of the most polished v1 commands.

`oxidebbs-server check` is expected to return no errors for
`config/oxidebbs.example.toml`.

### 4. Show board status

```bash
oxidebbs-server status
```

Required behavior:

- Show board name
- Show version
- Show DecentDB path
- Show telnet bind
- Show node count
- Show active sessions
- Show uptime if server is running
- Show enabled doors
- Show message area count

`status --json` returns a stable top-level object with `board`, `version`,
`database`, `telnet`, `nodes`, `doors`, and `messages`.

Example output:

```text
OxideBBS Status
Board:        Blackboard BBS
Version:      1.0.0
Database:     ./data/oxidebbs.ddb
Telnet:       127.0.0.1:2323
Nodes:        4 total, 1 active
Doors:        3 enabled
Messages:     5 areas
```

## User commands

### Essential v1

```bash
oxidebbs-server users list
oxidebbs-server users show <alias-or-id>
oxidebbs-server users add
oxidebbs-server users reset-password <alias-or-id>
oxidebbs-server users set-level <alias-or-id> <level>
oxidebbs-server users enable <alias-or-id>
oxidebbs-server users disable <alias-or-id>
oxidebbs-server users promote-sysop <alias-or-id>
oxidebbs-server users demote-sysop <alias-or-id>
```

### Nice for v1.1

```bash
oxidebbs-server users rename <old-alias> <new-alias>
oxidebbs-server users audit <alias-or-id>
oxidebbs-server users sessions <alias-or-id>
oxidebbs-server users delete <alias-or-id>
```

### Notes

`delete` should probably not be v1 essential. Disabling users is safer than deleting them because messages, door runs, and audit history may reference the user.

## Node and session commands

### Essential v1

```bash
oxidebbs-server nodes list
oxidebbs-server nodes watch
oxidebbs-server nodes show <node-number>
oxidebbs-server nodes disconnect <node-number>
oxidebbs-server nodes message <node-number> "Message text"
oxidebbs-server nodes broadcast "Message text"
```

### Nice for v1.1

```bash
oxidebbs-server nodes disable <node-number>
oxidebbs-server nodes enable <node-number>
oxidebbs-server nodes reset-stale
```

### Required node states

```text
available
connecting
login
main_menu
reading_messages
posting_message
in_door
disconnecting
offline
stale
```

### Local control channel (v1)

The node commands are backed by a local Unix control socket at:

```text
<runtime>/oxidebbs-control.sock
```

The control socket is local-only and only accepts callers whose Unix peer UID matches
the server process effective UID. Run CLI commands under the same OS user account
as the server process (for example:
`sudo -u oxidebbs oxidebbs-server nodes list`); other users typically fail with
a peer-UID mismatch before commands are processed.
It also uses local filesystem permissions for
additional isolation.

`nodes list`, `nodes show`, `nodes watch`, `nodes disconnect`, `nodes message`,
`nodes broadcast`, and `nodes reset-stale` attempt live runtime state first.
When the socket is unavailable they fall back to DecentDB session rows (for status)
or audit-recorded intent for actions that require delivery.

`nodes reset-stale` marks stale live nodes as disconnecting and requests runtime
cleanup only when control is connected.

### Notes

`nodes watch` can be simple in v1: refresh every few seconds and print a table.
The Ratatui console can replace this later with a real live dashboard. When the
server is running, these states come from the local runtime registry and include
heartbeat age for stale-session diagnosis. `nodes reset-stale` should use the
live control channel when available and fall back to audited intent when the
server is unreachable.

Future web-based admin interfaces are not in v1 and, if introduced, must not
proceed until CSRF and replay protections are in place.

## Message commands

### Essential v1

```bash
oxidebbs-server messages areas list
oxidebbs-server messages areas add <key> --name "General"
oxidebbs-server messages areas show <key>
oxidebbs-server messages areas enable <key>
oxidebbs-server messages areas disable <key>
oxidebbs-server messages list --area <key>
oxidebbs-server messages show <message-id>
oxidebbs-server messages delete <message-id>
```

### Nice for v1.1

```bash
oxidebbs-server messages areas set-level <key> --read <level> --post <level>
oxidebbs-server messages move <message-id> --to-area <key>
oxidebbs-server messages lock <message-id>
oxidebbs-server messages unlock <message-id>
oxidebbs-server messages search <query>
```

### Notes

Moderation needs to exist in v1, but it can be minimal. The most important v1 operation is being able to remove a bad local message.

## Door commands

Door commands are essential because DOS door support is a flagship OxideBBS feature.

### Essential v1

```bash
oxidebbs-server doors list
oxidebbs-server doors show <door-key>
oxidebbs-server doors check <door-key>
oxidebbs-server doors enable <door-key>
oxidebbs-server doors disable <door-key>
oxidebbs-server doors test <door-key> --user <alias>
oxidebbs-server doors dropfile <door-key> --user <alias> --node <number>
```

### Nice for v1.1

```bash
oxidebbs-server doors add
oxidebbs-server doors edit <door-key>
oxidebbs-server doors runs
oxidebbs-server doors runs show <run-id>
oxidebbs-server doors cleanup
```

### `doors check`

Should validate:

- Door working directory exists
- Command exists
- Runner exists
- Drop-file format is supported
- Runtime directory is writable
- Door is not configured as both exclusive and multi-node incorrectly
- Time limit is valid

### `doors test`

Should run a door in a controlled sysop test mode.

It should be possible to test drop-file generation without launching the door:

```bash
oxidebbs-server doors test lord --user sysop --dry-run
```

### `doors dropfile`

This is important for troubleshooting. It should print or write the generated drop file for inspection:

```bash
oxidebbs-server doors dropfile lord --user sysop --node 1 --format door.sys
oxidebbs-server doors dropfile lord --user sysop --node 1 --format dorinfo1.def
```

## ANSI/screen commands

### Essential v1

```bash
oxidebbs-server ansi list
oxidebbs-server ansi show <screen-name>
oxidebbs-server ansi validate <screen-name>
oxidebbs-server ansi install-defaults
```

### Nice for v1.1

```bash
oxidebbs-server ansi preview <screen-name>
oxidebbs-server ansi convert <input> --from utf8 --to cp437
oxidebbs-server ansi inspect <screen-name>
```

### Notes

`ansi show` should write the raw ANSI to the local terminal only if the user asks. A safe preview mode should also exist because not every shell will render CP437/ANSI correctly.
`ansi install-defaults` installs the bundled terminal and screen assets into the
configured ANSI/screen paths without overwriting customized files.

## Database commands

### Essential v1

```bash
oxidebbs-server db init
oxidebbs-server db doctor
oxidebbs-server db stats
oxidebbs-server db backup <output-path>
```

### Nice for v1.1

```bash
oxidebbs-server db export --format json
oxidebbs-server db import --format json <path>
oxidebbs-server db compact
oxidebbs-server db verify
```

### Notes

Backup/restore should be designed around DecentDB. `db import --format json <path>`
is the v1 restore boundary; it is safe only for schema-only targets and runs
transactionally.
`db compact` is explicit but unsupported in this release because DecentDB does not
expose a production-safe compaction API.

`db stats --json` is a stable object contract with counts for schema version,
users, message areas, messages, sessions, live active sessions, open session
rows, doors, door runs, and audit events. `active_sessions` reflects live
runtime state from the control socket and is `0` when the server is unreachable;
`open_sessions` reports database rows whose `ended_at` value is still null.

## Log and audit commands

### Essential v1

```bash
oxidebbs-server logs tail
oxidebbs-server logs recent
oxidebbs-server audit recent
oxidebbs-server audit user <alias-or-id>
```

### Nice for v1.1

```bash
oxidebbs-server logs search <query>
oxidebbs-server audit node <node-number>
oxidebbs-server audit door <door-key>
```

## Config commands

### Essential v1

```bash
oxidebbs-server config show
oxidebbs-server config check
oxidebbs-server config paths
```

### Nice for v1.1

```bash
oxidebbs-server config set <key> <value>
oxidebbs-server config get <key>
```

### Notes

Editing config through commands is not essential for v1. It is enough to validate and display config.

## Local Ratatui sysop console

The Ratatui console should be v1.x unless implementation momentum is very high.

Launch command:

```bash
oxidebbs-server sysop
```

Recommended first screen:

```text
┌────────────────────────── OxideBBS Sysop ──────────────────────────┐
│ Board: Blackboard BBS        Uptime: 03:12:44        Nodes: 1 / 4   │
├───────────────┬────────────────────────────────────────────────────┤
│ Nodes         │ Recent Events                                       │
│ 1 GUEST       │ 22:14 caller_connected 192.168.1.50                 │
│ 2 Available   │ 22:15 login_success steven                          │
│ 3 Available   │ 22:18 door_started lord node=1                      │
│ 4 Available   │                                                    │
├───────────────┼────────────────────────────────────────────────────┤
│ Doors         │ Commands                                            │
│ LORD enabled  │ F1 Help  U Users  D Doors  M Messages  L Logs  Q Quit │
└───────────────┴────────────────────────────────────────────────────┘
```

### TUI v1.x screens

- Dashboard
- Nodes
- Users
- Doors
- Message areas
- Logs
- Config
- Database status

### TUI rule

The TUI should not duplicate business logic. It should call the same application services used by the CLI.

## V1 essential command shortlist

If v1 has to be ruthless, these are the must-have commands:

```bash
oxidebbs-server setup
oxidebbs-server check
oxidebbs-server serve
oxidebbs-server status

oxidebbs-server users list
oxidebbs-server users show <alias-or-id>
oxidebbs-server users add
oxidebbs-server users reset-password <alias-or-id>
oxidebbs-server users set-level <alias-or-id> <level>
oxidebbs-server users disable <alias-or-id>

oxidebbs-server nodes list
oxidebbs-server nodes disconnect <node-number>
oxidebbs-server nodes broadcast "Message text"

oxidebbs-server messages areas list
oxidebbs-server messages areas add <key> --name "Name"
oxidebbs-server messages delete <message-id>

oxidebbs-server doors list
oxidebbs-server doors show <door-key>
oxidebbs-server doors check <door-key>
oxidebbs-server doors test <door-key> --user <alias> --dry-run
oxidebbs-server doors dropfile <door-key> --user <alias> --node <number>

oxidebbs-server ansi list
oxidebbs-server ansi validate <screen-name>

oxidebbs-server db doctor
oxidebbs-server db backup <output-path>

oxidebbs-server logs tail
oxidebbs-server audit recent
```

## Commands that can wait

These should not block v1:

```bash
oxidebbs-server users delete
oxidebbs-server messages search
oxidebbs-server doors add
oxidebbs-server doors edit
oxidebbs-server ansi convert
oxidebbs-server config set
oxidebbs-server sysop
```

## Implementation recommendation

Implement the CLI using `clap`.

Suggested Rust module structure:

```text
oxidebbs-server/src/cli.rs
oxidebbs-server/src/commands/setup.rs
oxidebbs-server/src/commands/check.rs
oxidebbs-server/src/commands/serve.rs
oxidebbs-server/src/commands/status.rs
oxidebbs-server/src/commands/users.rs
oxidebbs-server/src/commands/nodes.rs
oxidebbs-server/src/commands/messages.rs
oxidebbs-server/src/commands/doors.rs
oxidebbs-server/src/commands/ansi.rs
oxidebbs-server/src/commands/db.rs
oxidebbs-server/src/commands/logs.rs
```

## Final recommendation

For v1, build the sysop interface as:

```text
CLI first.
Ratatui console later.
No web admin.
No remote sysop UI until local administration is solid.
```

The v1 CLI should make these tasks painless:

- start the board
- validate the board
- create/reset users
- watch nodes
- disconnect stuck callers
- validate ANSI screens
- test doors
- generate/inspect drop files
- back up DecentDB
- tail logs
