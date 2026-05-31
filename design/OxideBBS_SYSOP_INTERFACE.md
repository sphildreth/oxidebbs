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

Preferred binary name:

```bash
oxidebbs
```

Alternative binary name if split later:

```bash
oxidebbs-server
oxidebbs-sysop
```

Recommended v1 shape:

```bash
oxidebbs <command> [options]
```

Global options:

```bash
-c, --config <PATH>      Path to config file
--data <PATH>            Override DecentDB data path
--json                   Output machine-readable JSON where supported
--no-color               Disable colored local terminal output
-v, --verbose            Increase local log verbosity
```

## Essential v1 command groups

```text
oxidebbs serve
oxidebbs init
oxidebbs check
oxidebbs status
oxidebbs users ...
oxidebbs nodes ...
oxidebbs messages ...
oxidebbs doors ...
oxidebbs ansi ...
oxidebbs db ...
oxidebbs logs ...
```

## Essential v1 commands

### 1. Start the BBS

```bash
oxidebbs serve --config config/oxidebbs.toml
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
- Start DbWriter service
- Log startup details

Useful options:

```bash
oxidebbs serve --config config/oxidebbs.toml
oxidebbs serve --bind 0.0.0.0:2323
oxidebbs serve --dry-run
```

### 2. Initialize a new board

```bash
oxidebbs init
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
oxidebbs init --board-name "My BBS" --sysop-alias sysop --nodes 4
```

### 3. Validate configuration

```bash
oxidebbs check
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

### 4. Show board status

```bash
oxidebbs status
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

Example output:

```text
OxideBBS Status
Board:        Blackboard BBS
Version:      0.1.0
Database:     ./data/oxidebbs.ddb
Telnet:       0.0.0.0:2323
Nodes:        4 total, 1 active
Doors:        3 enabled
Messages:     5 areas
```

## User commands

### Essential v1

```bash
oxidebbs users list
oxidebbs users show <alias-or-id>
oxidebbs users add
oxidebbs users reset-password <alias-or-id>
oxidebbs users set-level <alias-or-id> <level>
oxidebbs users enable <alias-or-id>
oxidebbs users disable <alias-or-id>
oxidebbs users promote-sysop <alias-or-id>
oxidebbs users demote-sysop <alias-or-id>
```

### Nice for v1.1

```bash
oxidebbs users rename <old-alias> <new-alias>
oxidebbs users audit <alias-or-id>
oxidebbs users sessions <alias-or-id>
oxidebbs users delete <alias-or-id>
```

### Notes

`delete` should probably not be v1 essential. Disabling users is safer than deleting them because messages, door runs, and audit history may reference the user.

## Node and session commands

### Essential v1

```bash
oxidebbs nodes list
oxidebbs nodes watch
oxidebbs nodes show <node-number>
oxidebbs nodes disconnect <node-number>
oxidebbs nodes message <node-number> "Message text"
oxidebbs nodes broadcast "Message text"
```

### Nice for v1.1

```bash
oxidebbs nodes disable <node-number>
oxidebbs nodes enable <node-number>
oxidebbs nodes reset-stale
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

### Notes

`nodes watch` can be simple in v1: refresh every few seconds and print a table. The Ratatui console can replace this later with a real live dashboard.

## Message commands

### Essential v1

```bash
oxidebbs messages areas list
oxidebbs messages areas add <key> --name "General"
oxidebbs messages areas show <key>
oxidebbs messages areas enable <key>
oxidebbs messages areas disable <key>
oxidebbs messages list --area <key>
oxidebbs messages show <message-id>
oxidebbs messages delete <message-id>
```

### Nice for v1.1

```bash
oxidebbs messages areas set-level <key> --read <level> --post <level>
oxidebbs messages move <message-id> --to-area <key>
oxidebbs messages lock <message-id>
oxidebbs messages unlock <message-id>
oxidebbs messages search <query>
```

### Notes

Moderation needs to exist in v1, but it can be minimal. The most important v1 operation is being able to remove a bad local message.

## Door commands

Door commands are essential because DOS door support is a flagship OxideBBS feature.

### Essential v1

```bash
oxidebbs doors list
oxidebbs doors show <door-key>
oxidebbs doors check <door-key>
oxidebbs doors enable <door-key>
oxidebbs doors disable <door-key>
oxidebbs doors test <door-key> --user <alias>
oxidebbs doors dropfile <door-key> --user <alias> --node <number>
```

### Nice for v1.1

```bash
oxidebbs doors add
oxidebbs doors edit <door-key>
oxidebbs doors runs
oxidebbs doors runs show <run-id>
oxidebbs doors cleanup
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
oxidebbs doors test lord --user sysop --dry-run
```

### `doors dropfile`

This is important for troubleshooting. It should print or write the generated drop file for inspection:

```bash
oxidebbs doors dropfile lord --user sysop --node 1 --format door.sys
oxidebbs doors dropfile lord --user sysop --node 1 --format dorinfo1.def
```

## ANSI/screen commands

### Essential v1

```bash
oxidebbs ansi list
oxidebbs ansi show <screen-name>
oxidebbs ansi validate <screen-name>
oxidebbs ansi install-defaults
```

### Nice for v1.1

```bash
oxidebbs ansi preview <screen-name>
oxidebbs ansi convert <input> --from utf8 --to cp437
oxidebbs ansi inspect <screen-name>
```

### Notes

`ansi show` should write the raw ANSI to the local terminal only if the user asks. A safe preview mode should also exist because not every shell will render CP437/ANSI correctly.

## Database commands

### Essential v1

```bash
oxidebbs db init
oxidebbs db doctor
oxidebbs db stats
oxidebbs db backup <output-path>
```

### Nice for v1.1

```bash
oxidebbs db export --format json
oxidebbs db import --format json <path>
oxidebbs db compact
oxidebbs db verify
```

### Notes

Backup/restore should be designed around DecentDB. Do not assume SQLite or PostgreSQL tooling.

## Log and audit commands

### Essential v1

```bash
oxidebbs logs tail
oxidebbs logs recent
oxidebbs audit recent
oxidebbs audit user <alias-or-id>
```

### Nice for v1.1

```bash
oxidebbs logs search <query>
oxidebbs audit node <node-number>
oxidebbs audit door <door-key>
```

## Config commands

### Essential v1

```bash
oxidebbs config show
oxidebbs config check
oxidebbs config paths
```

### Nice for v1.1

```bash
oxidebbs config set <key> <value>
oxidebbs config get <key>
```

### Notes

Editing config through commands is not essential for v1. It is enough to validate and display config.

## Local Ratatui sysop console

The Ratatui console should be v1.x unless implementation momentum is very high.

Launch command:

```bash
oxidebbs sysop
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
oxidebbs init
oxidebbs check
oxidebbs serve
oxidebbs status

oxidebbs users list
oxidebbs users show <alias-or-id>
oxidebbs users add
oxidebbs users reset-password <alias-or-id>
oxidebbs users set-level <alias-or-id> <level>
oxidebbs users disable <alias-or-id>

oxidebbs nodes list
oxidebbs nodes disconnect <node-number>
oxidebbs nodes broadcast "Message text"

oxidebbs messages areas list
oxidebbs messages areas add <key> --name "Name"
oxidebbs messages delete <message-id>

oxidebbs doors list
oxidebbs doors show <door-key>
oxidebbs doors check <door-key>
oxidebbs doors test <door-key> --user <alias> --dry-run
oxidebbs doors dropfile <door-key> --user <alias> --node <number>

oxidebbs ansi list
oxidebbs ansi validate <screen-name>

oxidebbs db doctor
oxidebbs db backup <output-path>

oxidebbs logs tail
oxidebbs audit recent
```

## Commands that can wait

These should not block v1:

```bash
oxidebbs users delete
oxidebbs messages search
oxidebbs doors add
oxidebbs doors edit
oxidebbs ansi convert
oxidebbs db import
oxidebbs db compact
oxidebbs config set
oxidebbs sysop
```

## Implementation recommendation

Implement the CLI using `clap`.

Suggested Rust module structure:

```text
oxidebbs-server/src/cli.rs
oxidebbs-server/src/commands/init.rs
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
