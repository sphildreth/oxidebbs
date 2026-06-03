# OxideBBS Sysop Interface and TUI Master Specification

## Document Purpose

This single consolidated document combines the OxideBBS sysop/admin interface planning into one reference:

1. **Sysop CLI and administration requirements**
2. **Feature-rich local Sysop TUI mockups**
3. **Node display/scaling requirements for 8, 16, 32+ nodes**
4. **Implementation guidance for a Rust/Ratatui-based sysop console**

The intent is to give a coding agent or contributor one authoritative starting document for building the OxideBBS sysop administration experience.

## Project Context

OxideBBS is a Rust-based BBS software project with:

- Telnet-first v1
- ANSI/CP437-first caller experience
- DecentDB as the system database
- DOS door support as a flagship feature
- Future OxideNet / FTN-style message networking
- A local sysop TUI inspired by classic BBS software such as Telegard, VBBS, Wildcat!, Renegade, WWIV, and similar systems

The sysop TUI should feel like a **classic BBS control center rebuilt as a modern Rust terminal application**.

## Consolidated Design Decisions

### CLI-first, TUI second

The CLI is the first reliable administration surface. The TUI should use the same underlying services and should not duplicate business logic.

### Ratatui for local sysop UI only

Ratatui is appropriate for the local sysop/admin console. It should **not** be used for the remote caller UI. Remote callers should receive a byte-oriented ANSI/CP437 BBS experience.

### Optimized for 8 nodes, scalable beyond that

The first real Blackboard/OxideBBS instance is expected to run **8 nodes**, so the default dashboard should make 8 nodes feel natural. The implementation must also scale cleanly to 16, 32, and larger configurations.

### Doors are a first-class sysop concern

Door checking, drop-file generation, dry-run testing, active door monitoring, and failed-run troubleshooting are must-have TUI capabilities.

### OxideNet should have a future admin surface

When OxideNet exists, the sysop TUI should include network applications, node registry, packet queues, area subscriptions, poll logs, quarantine, and nodelist generation.

## Table of Contents

1. Sysop Interface Requirements
2. Essential v1 CLI Commands
3. Local Ratatui Sysop TUI
4. TUI Layout and Navigation
5. Dashboard Mockups
6. Node Scaling Requirements
7. Nodes Screen Mockups
8. Users Screen Mockups
9. Messages Screen Mockups
10. Door Management Mockups
11. OxideNet Mockups
12. ANSI Screen Management
13. Config, Database, Logs, and Audit
14. Command Palette and Help
15. Modal Patterns
16. Milestones and Implementation Plan
17. Engineering Requirements

---

## OxideBBS Sysop Interface

### Purpose

The sysop interface is the local administrative surface for running, inspecting, and maintaining an OxideBBS instance.

For v1, the sysop interface was **CLI-first**. A local Ratatui-based TUI shipped in **v1.1** once the core server, telnet sessions, users, local messages, DecentDB persistence, and basic door launching were stable.

### Design goals

1. Make common sysop tasks obvious.
2. Keep v1 operationally simple.
3. Avoid requiring a web admin panel.
4. Preserve the retro BBS feel without making administration painful.
5. Make door troubleshooting easy.
6. Make DecentDB health and backups easy to reason about.
7. Make the future Ratatui console a wrapper around the same command/service layer, not a separate admin system.

### Interface layers

OxideBBS should have three admin layers over time:

```text
v1      CLI admin commands
v1.1    Local Ratatui sysop console
v2+     Optional read-only status web dashboard, if desired
```

The CLI should be the source of truth. The TUI should call the same underlying services.

### Command shape

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

### Essential v1 command groups

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

### Essential v1 commands

#### 1. Start the BBS

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

#### 2. Initialize a new board

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

#### 3. Validate configuration

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

#### 4. Show board status

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

### User commands

#### Essential v1

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

#### Nice for v1.1

```bash
oxidebbs users rename <old-alias> <new-alias>
oxidebbs users audit <alias-or-id>
oxidebbs users sessions <alias-or-id>
oxidebbs users delete <alias-or-id>
```

#### Notes

`delete` should probably not be v1 essential. Disabling users is safer than deleting them because messages, door runs, and audit history may reference the user.

### Node and session commands

#### Essential v1

```bash
oxidebbs nodes list
oxidebbs nodes watch
oxidebbs nodes show <node-number>
oxidebbs nodes disconnect <node-number>
oxidebbs nodes message <node-number> "Message text"
oxidebbs nodes broadcast "Message text"
```

#### Nice for v1.1

```bash
oxidebbs nodes disable <node-number>
oxidebbs nodes enable <node-number>
oxidebbs nodes reset-stale
```

#### Required node states

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

#### Notes

`nodes watch` can be simple in v1: refresh every few seconds and print a table. The Ratatui console can replace this later with a real live dashboard.

### Message commands

#### Essential v1

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

#### Nice for v1.1

```bash
oxidebbs messages areas set-level <key> --read <level> --post <level>
oxidebbs messages move <message-id> --to-area <key>
oxidebbs messages lock <message-id>
oxidebbs messages unlock <message-id>
oxidebbs messages search <query>
```

#### Notes

Moderation needs to exist in v1, but it can be minimal. The most important v1 operation is being able to remove a bad local message.

### Door commands

Door commands are essential because DOS door support is a flagship OxideBBS feature.

#### Essential v1

```bash
oxidebbs doors list
oxidebbs doors show <door-key>
oxidebbs doors check <door-key>
oxidebbs doors enable <door-key>
oxidebbs doors disable <door-key>
oxidebbs doors test <door-key> --user <alias>
oxidebbs doors dropfile <door-key> --user <alias> --node <number>
```

#### Nice for v1.1

```bash
oxidebbs doors add
oxidebbs doors edit <door-key>
oxidebbs doors runs
oxidebbs doors runs show <run-id>
oxidebbs doors cleanup
```

#### `doors check`

Should validate:

- Door working directory exists
- Command exists
- Runner exists
- Drop-file format is supported
- Runtime directory is writable
- Door is not configured as both exclusive and multi-node incorrectly
- Time limit is valid

#### `doors test`

Should run a door in a controlled sysop test mode.

It should be possible to test drop-file generation without launching the door:

```bash
oxidebbs doors test lord --user sysop --dry-run
```

#### `doors dropfile`

This is important for troubleshooting. It should print or write the generated drop file for inspection:

```bash
oxidebbs doors dropfile lord --user sysop --node 1 --format door.sys
oxidebbs doors dropfile lord --user sysop --node 1 --format dorinfo1.def
```

### ANSI/screen commands

#### Essential v1

```bash
oxidebbs ansi list
oxidebbs ansi show <screen-name>
oxidebbs ansi validate <screen-name>
oxidebbs ansi install-defaults
```

#### Nice for v1.1

```bash
oxidebbs ansi preview <screen-name>
oxidebbs ansi convert <input> --from utf8 --to cp437
oxidebbs ansi inspect <screen-name>
```

#### Notes

`ansi show` should write the raw ANSI to the local terminal only if the user asks. A safe preview mode should also exist because not every shell will render CP437/ANSI correctly.

### Database commands

#### Essential v1

```bash
oxidebbs db init
oxidebbs db doctor
oxidebbs db stats
oxidebbs db backup <output-path>
```

#### Nice for v1.1

```bash
oxidebbs db export --format json
oxidebbs db import --format json <path>
oxidebbs db compact
oxidebbs db verify
```

#### Notes

Backup/restore should be designed around DecentDB. Do not assume SQLite or PostgreSQL tooling.

### Log and audit commands

#### Essential v1

```bash
oxidebbs logs tail
oxidebbs logs recent
oxidebbs audit recent
oxidebbs audit user <alias-or-id>
```

#### Nice for v1.1

```bash
oxidebbs logs search <query>
oxidebbs audit node <node-number>
oxidebbs audit door <door-key>
```

### Config commands

#### Essential v1

```bash
oxidebbs config show
oxidebbs config check
oxidebbs config paths
```

#### Nice for v1.1

```bash
oxidebbs config set <key> <value>
oxidebbs config get <key>
```

#### Notes

Editing config through commands is not essential for v1. It is enough to validate and display config.

### Local Ratatui sysop console

The Ratatui console shipped in v1.1.

Launch command:

```bash
oxidebbs-server sysop
```

The current `oxidebbs-server` binary launches the full local TUI from `sysop`
by default. `--tui` remains as a compatibility flag, and `--readonly` disables
destructive TUI actions.

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

#### TUI v1.1 screens

- Dashboard
- Nodes
- Users
- Doors
- Message areas
- Logs
- Config
- Database status

#### TUI rule

The TUI should not duplicate business logic. It should call the same application services used by the CLI.

### V1 essential command shortlist

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

### Commands that can wait

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
```

### Implementation recommendation

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

### Final recommendation

For v1, the sysop interface was built as:

```text
CLI first.
Ratatui console in v1.1.
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

---

## OxideBBS Sysop TUI Mockups and Requirements


> v2 update: This version adds explicit handling for more than 4 nodes, optimizes the default dashboard for an 8-node Blackboard/OxideBBS instance, and requires scalable node display patterns for 16, 32, and larger systems.


### Purpose

This document defines the design direction, screen mockups, functional requirements, and implementation requirements for the **OxideBBS Sysop TUI**.

The goal is a feature-rich local sysop console inspired by classic BBS packages such as **Telegard**, **VBBS / Virtual Advanced**, **Wildcat!**, **Renegade**, **WWIV**, and similar sysop-friendly systems, but implemented with a modern Rust/Ratatui architecture.

The desired feel:

```text
Classic sysop control center + modern operational dashboard.
```

It should be dense, keyboard-driven, useful, fast, and charming.

### Product philosophy

OxideBBS should have two different terminal personalities:

```text
Remote caller UI:
  ANSI/CP437 byte-oriented BBS experience.

Local sysop TUI:
  Ratatui-powered modern terminal application with retro visual language.
```

The remote BBS UI should remain authentic ANSI/CP437. The local TUI can use modern terminal layout techniques, live tables, filtering, modals, tabs, command palettes, and structured status displays.

### Main goals

1. Give the sysop full visibility into the running board.
2. Make common administration tasks fast and keyboard-first.
3. Provide excellent door-game troubleshooting support.
4. Provide live node/session control.
5. Provide user and message management.
6. Provide OxideNet/FTN administration when that module exists.
7. Provide database health, backup, and audit visibility.
8. Preserve a retro BBS aesthetic without sacrificing usability.
9. Avoid requiring a web admin panel.
10. Build on the same service layer as the CLI.

### Non-goals

For the initial TUI:

- No web UI.
- No mouse-required workflows.
- No editing raw DecentDB internals.
- No replacing the remote caller menu system.
- No direct bundling of DOS door binaries.
- No file-transfer administration until file areas become a real feature.
- No full ANSI art editor in the first version.

### Recommended command

```bash
oxidebbs-server sysop
```

Optional flags:

```bash
oxidebbs-server --config config/oxidebbs.toml sysop
oxidebbs-server sysop --readonly
```

### Implementation stack

Recommended stack:

```text
Rust
Ratatui
Crossterm
Tokio
tracing
clap
DecentDB-backed application services
```

The TUI should use Ratatui for layout and widgets. It should **not** use Ratatui for the remote caller UI.


### Node-count design requirements

The sysop TUI must not assume a small four-node board.

The first real Blackboard/OxideBBS instance is expected to run **8 nodes**, so the default dashboard mockups and screenshots should be optimized around an 8-node layout. However, the implementation must scale cleanly beyond that.

#### Required node-count behavior

| Configured Nodes | Dashboard Behavior | Dedicated Nodes Screen Behavior |
|---:|---|---|
| 1–4 | Full detail rows may fit directly on dashboard | Table view, no scrolling required |
| 5–8 | Default target layout; show all nodes in a compact 2-row node map | Table view, no scrolling required on normal terminal sizes |
| 9–16 | Use compact grid on dashboard; avoid full-width detail rows | Scrollable table, grid view, active-only filter |
| 17–32 | Dashboard shows compact/paged summary; prioritize active/problem nodes | Scrollable/paged table and grid views |
| 33+ | Dashboard shows aggregate counts plus active/problem nodes only | Paged views, search, filters, grouping required |

#### Hard requirement

```text
The TUI must handle 1..N configured nodes.
No dashboard, table, grid, command, or node-detail workflow may assume exactly 4 nodes.
```

#### Default design target

```text
Default mockup target: 8 nodes
Minimum scaling target: 16 nodes
Future-friendly target: 32+ nodes
```

#### Node activity codes

Compact node maps should use short activity codes so the sysop can scan the board quickly.

Recommended codes:

| Code | Meaning |
|---|---|
| `FREE` | Node available |
| `CONN` | Caller connecting |
| `LOGN` | Login/new-user flow |
| `MENU` | Main menu |
| `MSGS` | Reading messages |
| `POST` | Posting message |
| `MAIL` | Private mail |
| `DOOR` | In a generic door |
| `LORD` | In a known named door |
| `CHAT` | Sysop chat |
| `IDLE` | Idle too long |
| `DISC` | Disconnecting |
| `STALE` | Stale/crashed session |
| `DOWN` | Node disabled/offline |

Known doors may show their short key, such as `LORD`, `BRE`, `OOII`, or `TRIV`, when that fits the available width.

#### Node colors/status semantics

The TUI should use color and text together. Do not rely only on color.

| Status | Suggested Color | Text |
|---|---|---|
| Available | Muted gray | `FREE` |
| Online active | Terminal green | `MENU`, `MSGS`, etc. |
| In door | Oxide orange or cyan | Door key |
| Idle warning | Amber | `IDLE` |
| Disconnecting | Amber | `DISC` |
| Stale/error | Red | `STALE` |
| Disabled | Dark gray | `DOWN` |

#### Required node views

The Nodes screen should support multiple views:

```text
Table View       Detailed rows for administration
Grid View        Compact node map for 8, 16, 32+ nodes
Active View      Hide free/disabled nodes
Door View        Show only users currently in doors
Problem View     Show stale, idle, failed, disconnecting, or errored nodes
```

Recommended shortcuts:

| Key | Action |
|---|---|
| `v` | Cycle view |
| `a` | Active-only view |
| `g` | Grid view |
| `t` | Table view |
| `p` | Problem nodes |
| `d` | Door nodes |
| `/` | Search nodes |
| `f` | Filter |
| `PageUp/PageDown` | Page through nodes |
| `Home/End` | Jump to first/last node |

### Dashboard layout for 8 nodes

The default dashboard should be optimized for the user's expected 8-node Blackboard/OxideBBS instance.

```text
┌──────────────────────────────────────── Dashboard ──────────────────────────────────────────────┐
│ Blackboard BBS │ OxideBBS 0.1.0 │ Up 03:12:44 │ Nodes 3/8 │ Doors 1 │ Alerts 0 │ 23:59:14      │
├─────────────────────────────────────── Node Map ────────────────────────────────────────────────┤
│ 01 steven    MSGS   │ 02 nightowl  LORD   │ 03 guest     MENU   │ 04 -         FREE             │
│ 05 -         FREE   │ 06 cactus    POST   │ 07 -         FREE   │ 08 -         FREE             │
├────────────────────────────────────── Recent Events ────────────────────────────────────────────┤
│ 22:18:03 node=2 door_started lord user=nightowl                                                 │
│ 22:20:44 node=6 message_posted area=general user=cactus                                         │
│ 22:22:01 system db_backup_started                                                               │
├───────────────────────────────┬────────────────────────────────────────────────────────────────┤
│ Health                        │ Alerts                                                         │
│ DB:        OK                 │ No active alerts                                                │
│ Telnet:    0.0.0.0:2323       │                                                                │
│ Doors:     3 enabled          │                                                                │
│ OxideNet:  disabled           │                                                                │
└───────────────────────────────┴────────────────────────────────────────────────────────────────┘
```

#### 8-node dashboard requirements

- Show all 8 nodes without scrolling.
- Use 2 rows × 4 columns where terminal width allows.
- Show node number, alias or `-`, and compact activity code.
- Highlight active, door, idle, and problem states.
- Selecting a node from the dashboard should open Node Detail.
- If terminal width is too narrow, degrade to 2 columns × 4 rows.

### Dashboard layout for 16 nodes

For 16 configured nodes, the dashboard should avoid full detail rows and instead show a compact node radar/map.

```text
┌──────────────────────────────────────── Dashboard ──────────────────────────────────────────────┐
│ Blackboard BBS │ OxideBBS 0.1.0 │ Up 03:12:44 │ Nodes 5/16 │ Doors 2 │ Alerts 1 │ 23:59:14     │
├─────────────────────────────────────── Node Map ────────────────────────────────────────────────┤
│ 01 steven    MSGS   │ 02 nightowl  LORD   │ 03 guest     MENU   │ 04 -         FREE             │
│ 05 -         FREE   │ 06 cactus    POST   │ 07 -         FREE   │ 08 -         FREE             │
│ 09 -         FREE   │ 10 sysop2    CHAT   │ 11 -         FREE   │ 12 -         FREE             │
│ 13 -         FREE   │ 14 -         FREE   │ 15 -         FREE   │ 16 -         FREE             │
├────────────────────────────────────── Recent Events ────────────────────────────────────────────┤
│ 22:18:03 node=2 door_started lord user=nightowl                                                 │
│ 22:20:44 node=6 message_posted area=general user=cactus                                         │
│ 22:21:17 node=10 sysop_chat_started user=sysop2                                                 │
│ 22:22:01 system db_backup_started                                                               │
├───────────────────────────────┬────────────────────────────────────────────────────────────────┤
│ Health                        │ Alerts                                                         │
│ DB:        OK                 │ Node 11 stale 04:12                                             │
│ Telnet:    0.0.0.0:2323       │                                                                │
│ Doors:     3 enabled          │                                                                │
│ OxideNet:  disabled           │                                                                │
└───────────────────────────────┴────────────────────────────────────────────────────────────────┘
```

#### 16-node dashboard requirements

- Prefer 4 columns × 4 rows on wide terminals.
- Collapse aliases if needed before hiding nodes.
- Show active/problem nodes with stronger visual priority.
- On smaller terminals, switch to summary + active/problem subset.
- The dedicated Nodes screen must always provide access to every node.

### Dashboard layout for 32+ nodes

For 32 or more nodes, the dashboard should not attempt to show all nodes at full label width unless the terminal is very large.

Recommended compact summary:

```text
┌──────────────────────────────────────── Dashboard ──────────────────────────────────────────────┐
│ Blackboard BBS │ Up 03:12:44 │ Nodes 11/32 │ Doors 4 │ Idle 2 │ Problems 1 │ Alerts 1          │
├────────────────────────────────────── Node Summary ─────────────────────────────────────────────┤
│ Active: 01 steven MSGS │ 02 nightowl LORD │ 06 cactus POST │ 10 sysop2 CHAT │ 18 raven MAIL     │
│ Doors:  02 LORD │ 12 BRE │ 19 TRIV │ 21 OOII                                                     │
│ Issues: 11 STALE 04:12 │ 24 IDLE 22:44                                                           │
├────────────────────────────────────── Recent Events ────────────────────────────────────────────┤
│ 22:18:03 node=2 door_started lord user=nightowl                                                 │
│ 22:20:44 node=6 message_posted area=general user=cactus                                         │
│ 22:21:17 node=11 stale_session_detected                                                         │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 32+ node dashboard requirements

- Show aggregate counts.
- Show active nodes first.
- Show problem nodes prominently.
- Show door nodes separately when useful.
- Provide a quick key to open full Nodes screen.
- Do not overcrowd the dashboard.

### Dedicated Nodes screen for 8 nodes

```text
┌────────────────────────────────────────── Nodes ────────────────────────────────────────────────┐
│ Nodes: 3/8 active      Filter: all      Sort: node      View: table      Auto-refresh: 2s       │
├────┬──────────────┬────────────────────┬──────────┬────────┬─────────────────┬────────────────┤
│ #  │ User         │ Activity           │ Time On  │ Idle   │ Remote          │ Status         │
├────┼──────────────┼────────────────────┼──────────┼────────┼─────────────────┼────────────────┤
│ 1  │ steven       │ Reading Messages   │ 00:24:12 │ 00:01  │ 192.168.1.50    │ Online         │
│ 2  │ nightowl     │ Door: LORD         │ 00:12:44 │ 00:00  │ 10.0.0.44       │ In Door        │
│ 3  │ guest        │ Main Menu          │ 00:02:10 │ 00:30  │ 192.168.1.80    │ Online         │
│ 4  │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ 5  │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ 6  │ cactus       │ Posting Message    │ 00:08:19 │ 00:00  │ 192.168.1.99    │ Online         │
│ 7  │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ 8  │ -            │ Available          │ --       │ --     │ --              │ Available      │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Move │ Enter Detail │ M Msg │ C Chat │ D Disconnect │ K Kill Door │ B Broadcast │ F Filter    │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Dedicated Nodes screen for 16 nodes

The 16-node table should scroll on smaller terminals and show all rows on larger terminals.

```text
┌────────────────────────────────────────── Nodes ────────────────────────────────────────────────┐
│ Nodes: 5/16 active     Filter: all     Sort: node     View: table     Auto-refresh: 2s          │
├────┬──────────────┬────────────────────┬──────────┬────────┬─────────────────┬────────────────┤
│ #  │ User         │ Activity           │ Time On  │ Idle   │ Remote          │ Status         │
├────┼──────────────┼────────────────────┼──────────┼────────┼─────────────────┼────────────────┤
│ 1  │ steven       │ Reading Messages   │ 00:24:12 │ 00:01  │ 192.168.1.50    │ Online         │
│ 2  │ nightowl     │ Door: LORD         │ 00:12:44 │ 00:00  │ 10.0.0.44       │ In Door        │
│ 3  │ guest        │ Main Menu          │ 00:02:10 │ 00:30  │ 192.168.1.80    │ Online         │
│ 4  │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ 5  │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ 6  │ cactus       │ Posting Message    │ 00:08:19 │ 00:00  │ 192.168.1.99    │ Online         │
│ 7  │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ 8  │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ 9  │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ 10 │ sysop2       │ Sysop Chat         │ 00:04:18 │ 00:00  │ 192.168.1.77    │ Chat           │
│ 11 │ -            │ Stale Session      │ --       │ 04:12  │ --              │ Stale          │
│ 12 │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ ... more rows if terminal is small ...                                                         │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ Move │ PgUp/PgDn │ Enter Detail │ M Msg │ D Disconnect │ K Kill Door │ V View │ F Filter     │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Nodes grid view

Grid view is preferred when the sysop wants to watch the whole board at a glance.

#### 8-node grid

```text
┌──────────────────────────────────────── Node Grid ──────────────────────────────────────────────┐
│ 01 steven    MSGS   │ 02 nightowl  LORD   │ 03 guest     MENU   │ 04 -         FREE             │
│ 05 -         FREE   │ 06 cactus    POST   │ 07 -         FREE   │ 08 -         FREE             │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Detail │ T Table │ A Active │ P Problems │ D Doors │ M Message │ B Broadcast │ Esc Back    │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 16-node grid

```text
┌──────────────────────────────────────── Node Grid ──────────────────────────────────────────────┐
│ 01 steven    MSGS   │ 02 nightowl  LORD   │ 03 guest     MENU   │ 04 -         FREE             │
│ 05 -         FREE   │ 06 cactus    POST   │ 07 -         FREE   │ 08 -         FREE             │
│ 09 -         FREE   │ 10 sysop2    CHAT   │ 11 -         STALE  │ 12 -         FREE             │
│ 13 -         FREE   │ 14 -         FREE   │ 15 -         FREE   │ 16 -         FREE             │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Detail │ T Table │ A Active │ P Problems │ D Doors │ M Message │ B Broadcast │ Esc Back    │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Dense 32-node grid

```text
┌──────────────────────────────────────── Node Grid 1-32 ─────────────────────────────────────────┐
│ 01 MSGS │ 02 LORD │ 03 MENU │ 04 FREE │ 05 FREE │ 06 POST │ 07 FREE │ 08 FREE                  │
│ 09 FREE │ 10 CHAT │ 11 STAL │ 12 BRE  │ 13 FREE │ 14 FREE │ 15 FREE │ 16 FREE                  │
│ 17 MAIL │ 18 MENU │ 19 TRIV │ 20 FREE │ 21 OOII │ 22 FREE │ 23 FREE │ 24 IDLE                  │
│ 25 FREE │ 26 FREE │ 27 FREE │ 28 FREE │ 29 FREE │ 30 FREE │ 31 FREE │ 32 FREE                  │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ←→ Move │ Enter Detail │ T Table │ A Active │ P Problems │ PgDn Next Page │ Esc Back             │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Active-only view

For larger boards, active-only view helps the sysop ignore empty nodes.

```text
┌────────────────────────────────────── Active Nodes ─────────────────────────────────────────────┐
│ Active: 5/16       Hidden free nodes: 11       Sort: activity                                   │
├────┬──────────────┬────────────────────┬──────────┬────────┬─────────────────┬────────────────┤
│ #  │ User         │ Activity           │ Time On  │ Idle   │ Remote          │ Status         │
├────┼──────────────┼────────────────────┼──────────┼────────┼─────────────────┼────────────────┤
│ 2  │ nightowl     │ Door: LORD         │ 00:12:44 │ 00:00  │ 10.0.0.44       │ In Door        │
│ 1  │ steven       │ Reading Messages   │ 00:24:12 │ 00:01  │ 192.168.1.50    │ Online         │
│ 6  │ cactus       │ Posting Message    │ 00:08:19 │ 00:00  │ 192.168.1.99    │ Online         │
│ 10 │ sysop2       │ Sysop Chat         │ 00:04:18 │ 00:00  │ 192.168.1.77    │ Chat           │
│ 3  │ guest        │ Main Menu          │ 00:02:10 │ 00:30  │ 192.168.1.80    │ Online         │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Problem-node view

Problem view is essential for 16+ node systems.

```text
┌────────────────────────────────────── Problem Nodes ────────────────────────────────────────────┐
│ Problems: 2      Idle threshold: 15m      Stale threshold: 2m                                    │
├────┬──────────────┬────────────────────┬──────────┬────────┬─────────────────┬────────────────┤
│ #  │ User         │ Problem            │ Time On  │ Idle   │ Remote          │ Action         │
├────┼──────────────┼────────────────────┼──────────┼────────┼─────────────────┼────────────────┤
│ 11 │ -            │ Stale Session      │ --       │ 04:12  │ --              │ Reset Node     │
│ 24 │ raven        │ Idle Too Long      │ 01:12:40 │ 22:44  │ 192.168.1.88    │ Message/Kick   │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Detail │ R Reset Stale │ M Message │ D Disconnect │ I Ignore │ Esc Back                  │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Updated node management requirements

#### Functional requirements

- The dashboard must support displaying all 8 configured nodes on the default Blackboard BBS instance.
- The dashboard must support at least 16 configured nodes through compact grid layout.
- The Nodes screen must support arbitrary configured node counts with scrolling or paging.
- The Nodes screen must include table, grid, active-only, door-only, and problem views.
- The node model must not hard-code node count.
- Node status summaries must be generated dynamically from configured nodes.
- Node keyboard navigation must work across grids and tables.
- Node filters must work across all configured nodes, not only visible rows.
- Node detail must be available for every node, active or inactive.
- Free/available nodes must remain visible in table/grid views unless filtered out.

#### Layout requirements

- 1–8 nodes: dashboard should show all nodes.
- 9–16 nodes: dashboard should show all nodes when terminal size allows.
- 17+ nodes: dashboard may show summary, active nodes, problem nodes, and paged grid.
- On narrow terminals, dashboard node map should reduce columns before hiding information.
- On very small terminals, dashboard should show aggregate node counts and a prompt to open Nodes screen.

#### Data requirements

Each node card/cell should be able to display:

```text
node_number
user_alias_or_blank
activity_code
status
idle_time_optional
door_key_optional
problem_flag_optional
```

Each node detail should display:

```text
node_number
status
current_user
activity
connected_at
time_on
idle_time
remote_address
terminal_type
encoding
current_door_optional
runtime_dir_optional
recent_events
```

#### Testing requirements

Add test cases for:

- 1 node
- 4 nodes
- 8 nodes
- 16 nodes
- 32 nodes
- 64 nodes
- no active nodes
- all active nodes
- one stale node
- multiple door nodes
- narrow terminal layout
- wide terminal layout


### Overall navigation model

The TUI should have three navigation concepts:

1. **Section navigation**
   - Dashboard
   - Nodes
   - Users
   - Messages
   - Doors
   - OxideNet
   - ANSI
   - Config
   - Database
   - Logs
   - Audit
   - Help

2. **Context actions**
   - Actions available for the selected item.
   - Example: disconnect node, reset password, test door.

3. **Command palette**
   - A fast fuzzy command launcher.

### Global keyboard shortcuts

| Key | Action |
|---|---|
| `F1` | Help |
| `F2` | Command palette |
| `F3` | Search/filter |
| `F5` | Refresh |
| `Tab` | Next panel |
| `Shift+Tab` | Previous panel |
| `Enter` | Open/select |
| `Esc` | Back/close modal |
| `q` | Quit current screen or app |
| `?` | Context help |
| `/` | Search current list |
| `a` | Add/create where applicable |
| `e` | Edit selected item |
| `d` | Disable/delete/disconnect depending on context; always confirm |
| `r` | Refresh/retry/reload depending on context |
| `Ctrl+b` | Database backup |
| `Ctrl+l` | Open logs |
| `Ctrl+n` | Nodes screen |
| `Ctrl+u` | Users screen |
| `Ctrl+d` | Doors screen |
| `Ctrl+m` | Messages screen |
| `Ctrl+o` | OxideNet screen |

### Global layout

This is the default app frame.

```text
┌──────────────────────────────────────── OxideBBS Sysop ────────────────────────────────────────┐
│ Blackboard BBS │ OxideBBS 0.1.0 │ Up 03:12:44 │ Nodes 1/4 │ Users 128 │ Alerts 0 │ 23:59:14   │
├───────────────┬────────────────────────────────────────────────────────────────────────────────┤
│ NAV           │ MAIN CONTENT                                                                   │
│ ▸ Dashboard   │                                                                                │
│   Nodes       │                                                                                │
│   Users       │                                                                                │
│   Messages    │                                                                                │
│   Doors       │                                                                                │
│   OxideNet    │                                                                                │
│   ANSI        │                                                                                │
│   Config      │                                                                                │
│   Database    │                                                                                │
│   Logs        │                                                                                │
│   Audit       │                                                                                │
│   Help        │                                                                                │
├───────────────┴────────────────────────────────────────────────────────────────────────────────┤
│ F1 Help │ F2 Command │ F3 Search │ F5 Refresh │ Tab Panel │ Enter Open │ Esc Back │ Q Quit       │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Design language

#### Theme: Oxide Classic

Visual inspiration:

- Black/charcoal background
- Oxide orange headings and accents
- Terminal green for active/online/success
- Amber/yellow for warnings
- Red for danger/failure
- Gray and off-white for neutral text
- Box drawing with selective heavy borders
- Minimal blinking; use sparingly and only for urgent alerts

Suggested semantic colors:

| Meaning | Color idea |
|---|---|
| Primary accent | Oxide orange |
| Active/online/success | Terminal green |
| Warning | Amber |
| Error/destructive | Red |
| Muted/inactive | Dark gray |
| Neutral text | Off-white |
| Data labels | Steel gray |
| Selection | Orange border or reversed row |

### Screen 1: Dashboard

#### Purpose

The dashboard is the first screen the sysop sees. It should answer:

- Is the board healthy?
- Who is online?
- What just happened?
- Are doors or networks failing?
- Do I need to act?

#### Mockup

```text
┌──────────────────────────────────────── Dashboard ──────────────────────────────────────────────┐
│ Board: Blackboard BBS                         Mode: Live                         Profile: prod  │
├────────────────────┬────────────────────┬────────────────────┬─────────────────────────────────┤
│ Nodes              │ Calls Today         │ Msgs Today          │ Door Runs Today                 │
│ 1 / 4 active       │ 12                  │ 34                  │ 8                               │
├────────────────────┴────────────────────┴────────────────────┴─────────────────────────────────┤
│ Active Nodes                                                                                   │
│ #  User/Alias       Activity             Time On   Idle   Remote           Flags                │
│ 1  GUEST            Main Menu            00:03:12  00:00  192.168.1.50     ANSI                 │
│ 2  -                Available            --        --     --               --                   │
│ 3  -                Available            --        --     --               --                   │
│ 4  -                Available            --        --     --               --                   │
├──────────────────────────────────────── Recent Events ──────────────────────────────────────────┤
│ 22:14:01  node=1  caller_connected        192.168.1.50                                         │
│ 22:14:12  node=1  login_guest             GUEST                                                │
│ 22:15:44  system  ansi_loaded             welcome.ans                                          │
│ 22:18:03  door    door_finished           lord exit=0 duration=00:12:10                        │
├───────────────────────────────┬────────────────────────────────────────────────────────────────┤
│ Health                        │ Alerts                                                         │
│ DB:        OK                 │ No active alerts                                                │
│ Telnet:    0.0.0.0:2323       │                                                                │
│ Runtime:   OK                 │                                                                │
│ Doors:     3 enabled          │                                                                │
│ OxideNet:  disabled           │                                                                │
└───────────────────────────────┴────────────────────────────────────────────────────────────────┘
```

#### Requirements

- Show board name.
- Show uptime.
- Show active nodes and total nodes.
- Show calls today.
- Show messages posted today.
- Show door runs today.
- Show recent events.
- Show high-level health.
- Show active alerts.
- Refresh automatically.
- Allow manual refresh with `F5`.
- Allow jumping to selected node with `Enter`.
- Allow opening logs with `Ctrl+l`.

#### V1 minimum

- Active node list.
- Recent events.
- Health summary.
- Static counts from DecentDB.

#### V1.x enhancements

- Live event stream.
- Alert drawer.
- Time-series mini charts.
- Poll/network status.
- Door failure counters.
- Database backup age warning.

### Screen 2: Nodes

#### Purpose

The Nodes screen is the classic sysop “who is online and what are they doing?” view.

#### Node list mockup

```text
┌────────────────────────────────────────── Nodes ────────────────────────────────────────────────┐
│ Filter: all                         Sort: node                         Auto-refresh: 2s         │
├────┬──────────────┬────────────────────┬──────────┬────────┬─────────────────┬────────────────┤
│ #  │ User         │ Activity           │ Time On  │ Idle   │ Remote          │ Status         │
├────┼──────────────┼────────────────────┼──────────┼────────┼─────────────────┼────────────────┤
│ 1  │ steven       │ Reading Messages   │ 00:24:12 │ 00:01  │ 192.168.1.50    │ Online         │
│ 2  │ nightowl     │ Door: LORD         │ 00:12:44 │ 00:00  │ 10.0.0.44       │ In Door        │
│ 3  │ -            │ Available          │ --       │ --     │ --              │ Available      │
│ 4  │ -            │ Available          │ --       │ --     │ --              │ Available      │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Open │ M Message │ C Chat │ D Disconnect │ K Kill Door │ W Watch │ R Refresh             │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Node detail mockup

```text
┌──────────────────────────────────────── Node 2 Detail ──────────────────────────────────────────┐
│ User: nightowl                  Alias: Night Owl                  Security: 50                 │
│ Activity: Door: LORD            Connected: 00:12:44               Idle: 00:00:03               │
│ Remote: 10.0.0.44               Terminal: SyncTERM                Encoding: CP437              │
├──────────────────────────────────────── Door Session ───────────────────────────────────────────┤
│ Door: Legend of the Red Dragon                                                                 │
│ Runner: DOSBox                                                                                 │
│ Runtime Dir: ./runtime/nodes/002/lord                                                          │
│ Drop File: DORINFO1.DEF                                                                        │
│ Started: 22:18:03                                                                              │
│ Time Limit: 30 min                                                                             │
├──────────────────────────────────────── Recent Node Events ─────────────────────────────────────┤
│ 22:17:55 menu_command D                                                                         │
│ 22:18:03 door_started lord                                                                      │
│ 22:18:04 dropfile_written DORINFO1.DEF                                                         │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ M Message │ C Chat │ D Disconnect │ K Kill Door │ T Tail I/O │ A Audit │ Esc Back              │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements

- List all nodes.
- Show current user.
- Show current activity.
- Show connection time.
- Show idle time.
- Show remote address.
- Show status.
- Show whether the user is in a door.
- Show terminal/client if known.
- Allow disconnecting a node.
- Allow sending a message to a node.
- Allow broadcasting to all nodes.
- Allow viewing node detail.
- Allow viewing node audit events.
- Disconnect must require confirmation.
- Killing an active door must require confirmation.
- Sysop message should be logged.
- Broadcast should be logged.

### Screen 3: Users

#### User list mockup

```text
┌────────────────────────────────────────── Users ────────────────────────────────────────────────┐
│ Search: ste                         Filter: active                         Users: 128           │
├──────────┬──────────────┬──────────────┬───────┬────────────┬─────────────┬────────────────────┤
│ ID       │ Alias        │ Real Name    │ Sec   │ Calls      │ Last Login  │ Status             │
├──────────┼──────────────┼──────────────┼───────┼────────────┼─────────────┼────────────────────┤
│ 000001   │ steven       │ Steven H.    │ 100   │ 42         │ Today       │ Sysop              │
│ 000014   │ nightowl     │ -            │ 50    │ 12         │ Yesterday   │ Active             │
│ 000031   │ guest        │ -            │ 10    │ 2          │ 2026-05-29  │ Limited            │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Open │ A Add │ E Edit │ P Password │ L Level │ D Disable │ / Search │ F Filter           │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### User detail/editor mockup

```text
┌──────────────────────────────────────── User: steven ───────────────────────────────────────────┐
│ Alias:        steven                         Status:       Active                              │
│ Real Name:    Steven H.                      Role:         Sysop                               │
│ Security:     100                            Time Bank:    120 min                             │
│ Calls:        42                             Last Login:   2026-06-02 22:11                    │
│ Created:      2026-05-31                     Last Remote:  192.168.1.50                        │
├──────────────────────────────────────── Permissions ────────────────────────────────────────────┤
│ [x] Sysop          [x] Manage Users     [x] Manage Doors     [x] Manage OxideNet               │
│ [x] Post Messages  [x] Access Doors     [ ] Suspended        [ ] New User Hold                 │
├──────────────────────────────────────── Recent Activity ────────────────────────────────────────┤
│ 22:11 login_success node=1                                                                      │
│ 22:13 message_posted area=OXIDE.GENERAL                                                         │
│ 22:18 door_started lord                                                                         │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ E Edit │ P Reset Password │ L Set Level │ D Disable │ A Audit │ S Sessions │ Esc Back           │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements

- List users.
- Search users by alias, real name, id.
- Filter by active, disabled, sysop, new users, security level.
- View user detail.
- Add user.
- Edit user fields.
- Reset password.
- Set security level.
- Promote/demote sysop.
- Enable/disable user.
- View user audit history.
- View user's recent sessions.
- View user's recent posts.
- View user's door runs.
- Prefer disable over delete.
- Do not include hard delete in early TUI.
- All edits must be audit logged.

### Screen 4: Messages

#### Message areas mockup

```text
┌──────────────────────────────────────── Message Areas ──────────────────────────────────────────┐
│ Filter: all                                  Local: 5     Network: 4                            │
├───────────────┬──────────────────────────────┬──────────┬────────┬────────┬───────────────────┤
│ Key           │ Name                         │ Type     │ Msgs   │ Sec    │ Status            │
├───────────────┼──────────────────────────────┼──────────┼────────┼────────┼───────────────────┤
│ general       │ General Discussion           │ Local    │ 124    │ 10/10  │ Active            │
│ sysop         │ Sysop Discussion             │ Local    │ 18     │ 90/90  │ Active            │
│ ox-general    │ OXIDE.GENERAL                │ Network  │ 42     │ 10/10  │ Active            │
│ ox-test       │ OXIDE.TEST                   │ Network  │ 7      │ 10/10  │ Active            │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Open │ A Add │ E Edit │ D Disable │ M Messages │ S Security │ R Recount │ / Search        │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Message list mockup

```text
┌──────────────────────────────────── Messages: OXIDE.GENERAL ───────────────────────────────────┐
│ Search:                            Sort: newest                         Messages: 42            │
├────────┬──────────────────────────────┬──────────────┬────────────────────┬────────────────────┤
│ ID     │ Subject                      │ From         │ Date               │ Flags              │
├────────┼──────────────────────────────┼──────────────┼────────────────────┼────────────────────┤
│ 1042   │ Welcome to OxideNet          │ 42:1/1       │ 2026-06-02 22:14   │ Net, Pinned        │
│ 1041   │ Door runner notes            │ steven       │ 2026-06-02 21:02   │ Local              │
│ 1040   │ ANSI color experiments       │ nightowl     │ 2026-06-01 19:55   │ Net                │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Read │ D Delete │ P Pin │ M Move │ L Lock │ A Audit │ / Search │ Esc Back               │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Message detail mockup

```text
┌──────────────────────────────────────── Message 1042 ───────────────────────────────────────────┐
│ Area: OXIDE.GENERAL              From: 42:1/1 Blackboard BBS              Date: 2026-06-02       │
│ Subject: Welcome to OxideNet                                                                    │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Welcome to OxideNet. If you are reading this message, your first poll worked.                   │
│                                                                                                │
│ Reply in OXIDE.TEST to verify outbound scanning from your BBS.                                  │
├──────────────────────────────────────── Network Metadata ───────────────────────────────────────┤
│ Origin: 42:1/1         MsgID: <abc123>         SeenBy: 42:1/100          Packet: pkt-00044       │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ R Reply │ D Delete │ P Pin │ M Move │ X Export Metadata │ Esc Back                              │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements

- List areas.
- Add/edit/disable local areas.
- Show message counts.
- Show read/post security levels.
- List messages by area.
- Read message.
- Delete message with confirmation.
- Pin/unpin message.
- Move message between local areas.
- Show network metadata for OxideNet messages.
- Search messages later.
- Deletions must be audit logged.
- For network messages, deletion should be local moderation unless network moderation support exists.

### Screen 5: Doors

#### Purpose

Door support is a flagship feature. The TUI must make door management and troubleshooting excellent.

#### Door list mockup

```text
┌────────────────────────────────────────── Doors ────────────────────────────────────────────────┐
│ Runner: all                              Enabled: 3                         Failed Today: 1      │
├──────────────┬──────────────────────────────┬──────────┬──────────┬────────────┬───────────────┤
│ Key          │ Name                         │ Runner   │ Dropfile │ Runs Today │ Status        │
├──────────────┼──────────────────────────────┼──────────┼──────────┼────────────┼───────────────┤
│ lord         │ Legend of the Red Dragon     │ DOSBox   │ DORINFO  │ 4          │ Enabled       │
│ trivia       │ Death by Trivia              │ DOSBox   │ DOOR.SYS │ 2          │ Enabled       │
│ usurper      │ Usurper                      │ DOSBox   │ DORINFO  │ 0          │ Disabled      │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Open │ T Test │ C Check │ D Disable │ R Runs │ F Dropfile │ L Logs │ A Add               │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Door detail mockup

```text
┌────────────────────────────────────── Door: lord ───────────────────────────────────────────────┐
│ Name:        Legend of the Red Dragon                    Status:      Enabled                   │
│ Runner:      DOSBox                                      Exclusive:   No                        │
│ Command:     LORD.EXE                                    Drop File:   DORINFO1.DEF              │
│ Work Dir:    ./doors/lord                                Time Limit:  30 min                    │
│ Runtime:     ./runtime/nodes/{node}/lord                                                         │
├─────────────────────────────────────── Health Check ────────────────────────────────────────────┤
│ [OK] Working directory exists                                                                    │
│ [OK] Command exists                                                                              │
│ [OK] Runtime directory writable                                                                  │
│ [OK] Drop-file format supported                                                                  │
│ [!!] Last run exited with code 1                                                                 │
├─────────────────────────────────────── Recent Runs ─────────────────────────────────────────────┤
│ 22:18 node=2 user=nightowl duration=00:12:10 exit=0                                              │
│ 21:04 node=1 user=steven   duration=00:00:12 exit=1                                              │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ T Test │ C Check │ F View Dropfile │ E Edit │ R Runs │ L Logs │ D Disable │ Esc Back            │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Door test modal mockup

```text
┌────────────────────────────────────── Test Door: LORD ──────────────────────────────────────────┐
│ User:        sysop                                                                               │
│ Node:        1                                                                                   │
│ Mode:        Dry Run                                                                             │
│ Dropfile:    DORINFO1.DEF                                                                        │
│ Runtime Dir: ./runtime/test/node-001/lord                                                        │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ This will generate the runtime directory and drop file without launching DOSBox.                 │
│                                                                                                │
│ [ ] Launch actual door after dry-run                                                            │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Run Test │ F View Dropfile │ Esc Cancel                                                    │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Drop-file viewer mockup

```text
┌──────────────────────────────────── Generated DORINFO1.DEF ─────────────────────────────────────┐
│ Blackboard BBS                                                                                  │
│ Steven Hildreth                                                                                 │
│ COM1                                                                                            │
│ 38400                                                                                           │
│ 0                                                                                               │
│ steven                                                                                          │
│ Steven                                                                                          │
│ Hildreth                                                                                        │
│ 100                                                                                             │
│ 30                                                                                              │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Format: DORINFO1.DEF │ Encoding: CP437 │ Line endings: CRLF │ F Save │ C Copy │ Esc Back         │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements

- List doors.
- Add/edit/disable door definitions.
- Validate door config.
- Show per-door health.
- Show recent runs.
- Show failed runs.
- Generate drop files for inspection.
- Dry-run a door.
- Test-launch a door as a selected user/node.
- View door logs.
- Cleanup stale runtime directories.
- Show whether door is exclusive/multinode.
- Show supported drop-file formats.

#### Troubleshooting requirements

The TUI must clearly show:

- Which executable is being launched.
- Which working directory is used.
- Which runtime directory is used.
- Which drop file is generated.
- Which user/node is used for testing.
- Exit code.
- Timeout status.
- Last stdout/stderr if available.
- Last known problem.

### Screen 6: OxideNet

#### Purpose

Manage OxideNet and future FTN-style networking.

This screen should appear only when the module is enabled or installed.

#### OxideNet dashboard mockup

```text
┌────────────────────────────────────── OxideNet: 42:1/1 ─────────────────────────────────────────┐
│ Role: Primary Hub          Network: OxideNet          Zone: 42          Status: Active           │
├────────────────────┬────────────────────┬────────────────────┬─────────────────────────────────┤
│ Nodes              │ Pending Apps        │ Packets Today       │ Poll Failures                   │
│ 6 active           │ 2                   │ 42                  │ 1                               │
├──────────────────────────────────────── Applications ───────────────────────────────────────────┤
│ ID              Board                 Sysop        Submitted           Status                    │
│ OXNET-0007      Retro Cavern BBS      Night Owl    2026-06-02 20:14    Submitted                 │
│ OXNET-0008      Byte Barn             Cactus       2026-06-02 21:02    Needs Info                │
├──────────────────────────────────────── Network Queues ─────────────────────────────────────────┤
│ Inbound: 3 pending     Outbound: 12 pending     Quarantine: 1     Nodelist: current              │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ A Applications │ N Nodes │ E Areas │ P Packets │ G Generate Nodelist │ B Broadcast │ L Logs       │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Application review mockup

```text
┌──────────────────────────────── Application OXNET-0007 ────────────────────────────────────────┐
│ Board:       Retro Cavern BBS                     Requested: 2026-06-02 20:14                  │
│ Sysop:       Night Owl                            Contact: nightowl@example.net                │
│ Host:        retrocavern.example.net              BinkP: 24554                                 │
│ Software:    OxideBBS 0.4.0                       Timezone: America/Chicago                    │
│ Description: A retro ANSI board focused on door games and echomail.                             │
├────────────────────────────────────── Policy / Validation ──────────────────────────────────────┤
│ [OK] Policy accepted v1.0                                                                        │
│ [OK] Board name available                                                                        │
│ [OK] Hostname valid                                                                              │
│ [--] Reachability test not run                                                                   │
├──────────────────────────────────────── Admin Notes ────────────────────────────────────────────┤
│ Looks like a good first external test node.                                                      │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ A Approve │ R Reject │ I Needs Info │ H Hold │ T Test Host │ Esc Back                           │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Node registry mockup

```text
┌────────────────────────────────────── OxideNet Nodes ───────────────────────────────────────────┐
│ Search:                             Status: active                         Nodes: 6              │
├───────────┬─────────────────────────┬──────────────┬──────────────────────┬────────────────────┤
│ Address   │ Board                   │ Sysop        │ Last Poll            │ Status             │
├───────────┼─────────────────────────┼──────────────┼──────────────────────┼────────────────────┤
│ 42:1/1    │ Blackboard BBS          │ Steven       │ local                │ Hub                │
│ 42:1/100  │ Retro Cavern BBS        │ Night Owl    │ 00:12 ago            │ Active             │
│ 42:1/101  │ Byte Barn               │ Cactus       │ failed 02:14 ago     │ Warning            │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Open │ S Suspend │ R Rotate Password │ A Areas │ P Poll Log │ N Nodelist │ Esc Back        │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements

- Show network role.
- Show local network address.
- Show pending applications.
- Review applications.
- Approve/reject/request info.
- Assign address.
- Generate config package.
- List nodes.
- Suspend/retire node.
- Rotate node password.
- Show area subscriptions.
- Show packet queues.
- Show quarantine.
- Generate nodelist.
- Show poll logs.
- Broadcast network notice.

### Screen 7: ANSI/Screens

#### Mockup

```text
┌────────────────────────────────────── ANSI Screens ─────────────────────────────────────────────┐
│ Path: ./assets/ansi                          Encoding: CP437                                    │
├─────────────────────┬──────────┬──────────┬──────────────┬─────────────────────────────────────┤
│ Screen              │ Size     │ Modified │ Valid        │ Notes                               │
├─────────────────────┼──────────┼──────────┼──────────────┼─────────────────────────────────────┤
│ welcome.ans         │ 4.2 KB   │ Today    │ OK           │ Main welcome                        │
│ logon.ans           │ 2.1 KB   │ Today    │ OK           │ Login screen                        │
│ main-menu.ans       │ 3.8 KB   │ Today    │ Warning      │ Uses unsupported escape             │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Preview │ V Validate │ I Inspect │ R Reload │ D Duplicate │ Esc Back                         │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Preview mockup

```text
┌──────────────────────────────────── Preview: welcome.ans ───────────────────────────────────────┐
│ Mode: Rendered ANSI                         Terminal: 80x25                         CP437       │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                │
│  ╔══════════════════════════════════════════════════════════════════════════════╗                │
│  ║                         O X I D E B B S   v0.1                              ║                │
│  ╚══════════════════════════════════════════════════════════════════════════════╝                │
│                                                                                                │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ V Validate │ R Raw Bytes │ C CP437 Map │ Esc Back                                                │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements

- List screen assets.
- Validate ANSI files.
- Preview ANSI files in local terminal as best effort.
- Inspect raw bytes.
- Show unsupported escape warnings.
- Show encoding assumptions.
- Reload screens.
- Install default screens.
- Later: launch external editor.

### Screen 8: Config

#### Mockup

```text
┌──────────────────────────────────────── Config ─────────────────────────────────────────────────┐
│ File: config/oxidebbs.toml                                      Status: Valid                   │
├──────────────────────┬─────────────────────────────────────────────────────────────────────────┤
│ Section              │ Value                                                                   │
├──────────────────────┼─────────────────────────────────────────────────────────────────────────┤
│ board.name           │ Blackboard BBS                                                          │
│ telnet.bind          │ 0.0.0.0:2323                                                            │
│ nodes.count          │ 4                                                                       │
│ database.path        │ ./data/oxidebbs.ddb                                                     │
│ paths.ansi           │ ./assets/ansi                                                           │
│ paths.doors          │ ./doors                                                                 │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ C Check │ R Reload │ E External Edit │ P Paths │ Esc Back                                      │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements

- Show loaded config.
- Show config file path.
- Validate config.
- Show warnings/errors.
- Reload config if safe.
- Open external editor later.
- Never write config silently.

### Screen 9: Database

#### Mockup

```text
┌────────────────────────────────────── Database ─────────────────────────────────────────────────┐
│ Path: ./data/oxidebbs.ddb                               Status: OK                              │
├────────────────────┬───────────────────────────────────────────────────────────────────────────┤
│ Users              │ 128                                                                       │
│ Messages           │ 1,482                                                                     │
│ Door Runs          │ 344                                                                       │
│ Audit Events       │ 2,104                                                                     │
│ Last Backup        │ 2026-06-02 21:00                                                          │
│ Schema Version     │ 3                                                                         │
├────────────────────────────────────── Health Checks ────────────────────────────────────────────┤
│ [OK] Database opens                                                                             │
│ [OK] Schema current                                                                             │
│ [OK] Runtime writable                                                                           │
│ [!!] Last backup older than 24h                                                                 │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ B Backup │ D Doctor │ S Stats │ V Verify │ E Export │ Esc Back                                 │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements

- Show DecentDB path.
- Show schema version.
- Show counts by domain.
- Show last backup.
- Run doctor/check.
- Start backup.
- Show backup progress.
- Export later.
- Verify later.

### Screen 10: Logs

#### Mockup

```text
┌────────────────────────────────────────── Logs ─────────────────────────────────────────────────┐
│ Level: info+        Filter: door                         Follow: on                            │
├──────────┬─────────┬─────────────┬─────────────────────────────────────────────────────────────┤
│ Time     │ Level   │ Target      │ Message                                                     │
├──────────┼─────────┼─────────────┼─────────────────────────────────────────────────────────────┤
│ 22:18:03 │ INFO    │ door        │ door_started key=lord node=2 user=nightowl                  │
│ 22:18:04 │ DEBUG   │ door        │ dropfile_written path=runtime/node-002/DORINFO1.DEF         │
│ 22:30:13 │ INFO    │ door        │ door_finished key=lord exit=0 duration=00:12:10             │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ / Filter │ L Level │ F Follow │ C Clear View │ E Export │ Esc Back                              │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements

- Tail logs.
- Filter by level.
- Filter by target.
- Search text.
- Follow mode.
- Pause mode.
- Export selected range.
- Jump from event to related object when possible.

### Screen 11: Audit

#### Mockup

```text
┌───────────────────────────────────────── Audit ─────────────────────────────────────────────────┐
│ Filter: admin actions                         Range: today                                      │
├────────────────────┬──────────────┬──────────────┬─────────────────────────────────────────────┤
│ Time               │ Actor        │ Event        │ Details                                     │
├────────────────────┼──────────────┼──────────────┼─────────────────────────────────────────────┤
│ 2026-06-02 22:14   │ sysop        │ user_disable │ target=guest2 reason=spam                   │
│ 2026-06-02 21:55   │ sysop        │ door_test    │ door=lord result=ok                         │
│ 2026-06-02 21:00   │ system       │ db_backup    │ output=backups/oxidebbs-20260602.ddb        │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ / Search │ U User │ N Node │ D Door │ E Export │ Esc Back                                     │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements

- Show audit events.
- Filter by actor.
- Filter by target user.
- Filter by node.
- Filter by door.
- Filter by date.
- Export audit report.
- Audit all destructive/admin actions.

### Command palette

#### Mockup

```text
┌────────────────────────────────────── Command Palette ──────────────────────────────────────────┐
│ > reset pass                                                                                   │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ users reset-password <alias>        Reset a user's password                                     │
│ users list disabled                 Show disabled users                                         │
│ doors test <door>                   Run a door test                                             │
│ db backup                           Start a database backup                                     │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements

- Open with `F2`.
- Fuzzy search commands.
- Show keyboard shortcut where applicable.
- Support parameter prompts.
- Support recent commands.
- Support read-only mode hiding write actions.
- Confirm destructive actions.

### Help screen

#### Mockup

```text
┌──────────────────────────────────────── Help: Doors ────────────────────────────────────────────┐
│ This screen manages DOS door game definitions and runtime testing.                              │
├─────────────────────────────────────── Common Actions ──────────────────────────────────────────┤
│ T    Test selected door                                                                          │
│ C    Check selected door configuration                                                           │
│ F    View generated drop file                                                                    │
│ L    View logs for selected door                                                                 │
│ D    Disable selected door                                                                       │
├─────────────────────────────────────── Tips ────────────────────────────────────────────────────┤
│ Use dry-run before launching a new DOS door.                                                     │
│ Check that the working directory and runtime directory are writable.                             │
│ Use drop-file viewer to confirm user name, node number, and time limit.                          │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Requirements

- `F1` opens global help or context help.
- `?` opens context help.
- Help should explain current screen.
- Help should show shortcuts.
- Help should show common troubleshooting tips.

### Modal patterns

#### Confirmation modal

```text
┌──────────────────────────────────── Confirm Disconnect ─────────────────────────────────────────┐
│ Disconnect node 2?                                                                              │
│                                                                                                │
│ User: nightowl                                                                                  │
│ Activity: Door: LORD                                                                            │
│                                                                                                │
│ This will terminate the active session and may kill the running door process.                    │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Y Confirm │ N Cancel                                                                            │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Form modal

```text
┌──────────────────────────────────── Add User ───────────────────────────────────────────────────┐
│ Alias:        ____________________                                                              │
│ Real Name:    ____________________                                                              │
│ Security:     10                                                                                │
│ Password:     ****************                                                                  │
│ Confirm:      ****************                                                                  │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ Enter Save │ Esc Cancel │ Tab Next Field                                                        │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Error modal

```text
┌──────────────────────────────────────── Door Check Failed ──────────────────────────────────────┐
│ Door: lord                                                                                      │
│ Problem: Command not found                                                                      │
│                                                                                                │
│ Expected: ./doors/lord/LORD.EXE                                                                 │
│                                                                                                │
│ Suggested fix: verify the door working directory and command name.                              │
├────────────────────────────────────────────────────────────────────────────────────────────────┤
│ L View Logs │ E Edit Door │ Esc Close                                                           │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Feature requirements by milestone

#### Milestone TUI-0: Foundation ✅ (v1.1)

- App shell.
- Theme system.
- Navigation rail.
- Header/footer.
- Command palette shell with fuzzy search.
- Help modal shell.
- Keyboard handling.
- Async crossterm event loop.

#### Milestone TUI-1: Live dashboard and nodes ✅ (v1.1)

- Dashboard backed by real service data.
- Node list/table/grid views.
- Node detail.
- Disconnect node with confirmation.
- Message node.
- Broadcast.
- Recent events.
- Auto-refresh.
- Filter/sort nodes.

#### Milestone TUI-2: Users ✅ (v1.1)

- User list with sortable columns.
- User search/filter.
- User detail.
- Reset password (Argon2 hashed via `UserAdminService`).
- Set security level.
- Enable/disable user.
- Promote/demote sysop.

#### Milestone TUI-3: Doors ✅ (v1.1)

- Door list.
- Door detail.
- Door run history (50 recent runs).
- Enable/disable door.

_Note: door config check, drop-file viewer, dry-run, test launch, and runtime cleanup remain CLI-only for v1.1 and are planned for a future TUI enhancement._

#### Milestone TUI-4: Messages ✅ (v1.1)

- Message area list.
- Message list by area.
- Message detail.
- Delete message with confirmation.
- Soft-delete via `MessageAdminService`.

_Note: area add/edit, pin/move, and network metadata remain planned for future enhancement._

#### Milestone TUI-5: Database, logs, audit ✅ (v1.1)

- Database status (schema version, row counts, health checks).
- Live logs tail.
- Audit recent events.
- Audit user filter.

_Note: backup command, doctor/check command, and export remain CLI-only for v1.1._

#### Milestone TUI-6: OxideNet

- OxideNet dashboard.
- Applications list.
- Application review.
- Approve/reject/needs-info.
- Node registry.
- Area subscriptions.
- Packet queue/quarantine.
- Poll logs.
- Nodelist generation.

_Note: OxideNet is not yet implemented; this milestone is pending._

### V1.1 shipped TUI

The v1.1 TUI includes:

```text
Dashboard
Nodes
Users
Doors
Messages
Database
Logs
Audit
Config
ANSI
Help
```

Implemented actions:

```text
Disconnect node (with confirmation)
Broadcast message
Message node
Reset user password
Set security level
Enable/disable user
Promote/demote sysop
Delete message (with confirmation)
View door run history
Filter and sort lists
Command palette (fuzzy search)
```

### V1.5 ideal TUI enhancements

Future enhancements may include:

```text
Add/edit door definitions
Door config check in TUI
Drop-file viewer in TUI
Door dry-run/test in TUI
Database backup from TUI
Message area add/edit
Audit export
Config editing
OxideNet administration
```

### V2 / OxideNet TUI

When OxideNet exists:

```text
OxideNet dashboard
Applications
Node registry
Packet queues
Area subscriptions
Poll logs
Nodelist generation
Config package generation
```

### Engineering requirements

#### Architecture

The TUI should be layered:

```text
oxidebbs-sysop
  app.rs
  theme.rs
  input.rs
  command_palette.rs
  screens/
    dashboard.rs
    nodes.rs
    users.rs
    messages.rs
    doors.rs
    oxidenet.rs
    ansi.rs
    config.rs
    database.rs
    logs.rs
    audit.rs
    help.rs
```

#### Screen contract

The original design proposed a common `Screen` trait:

```rust
trait Screen {
    fn title(&self) -> &str;
    fn handle_event(&mut self, event: UiEvent) -> UiAction;
    fn render(&self, frame: &mut Frame, area: Rect);
}
```

In the v1.1 implementation, this trait was **removed** because different screens required different parameters (e.g., some need `AppConfig` for control socket paths, some need service references). Instead, `app.rs` dispatches events and render calls directly to each screen struct. A shared `UiAction` enum lives in `screens/common.rs` for cross-screen communication and modal handling.

#### Actions

Screens should emit actions rather than directly mutating everything.

```rust
enum UiAction {
    None,
    Navigate(ScreenId),
    OpenModal(ModalId),
    RunCommand(CommandId),
    Refresh,
    Quit,
}
```

#### Service layer

The TUI should call application services:

```text
NodeAdminService
UserAdminService
DoorAdminService
MessageAdminService
DatabaseAdminService
LogService
AuditService
OxideNetAdminService
```

Do not put DecentDB queries directly in widgets.

#### Read-only mode

The TUI should support read-only mode:

```bash
oxidebbs-server sysop --readonly
```

In read-only mode:

- Hide or disable destructive actions.
- Allow dashboards, logs, audit, status, details.
- Prevent changes.

#### Accessibility and terminal requirements

- Support 80x25 minimum with reduced layout.
- Prefer 100x30 or larger.
- Handle terminal resize.
- Avoid relying only on color.
- Use text labels for statuses.
- Avoid excessive blinking.
- Provide high-contrast theme.

#### Logging/audit requirements

Every admin write action from the TUI must be audit logged.

Examples:

- user_added
- user_disabled
- user_password_reset
- node_disconnected
- broadcast_sent
- door_test_started
- door_disabled
- message_deleted
- db_backup_started
- oxidenet_application_approved
- oxidenet_node_suspended

#### Safety requirements

Destructive actions must require confirmation:

- Disconnect node
- Kill door process
- Disable user
- Delete message
- Disable door
- Suspend OxideNet node
- Rotate credentials
- Restore database
- Delete/retire network node

### Recommended first implementation path

Build in this order:

1. TUI shell with mock data.
2. Theme and layout.
3. Dashboard from real status service.
4. Node list from real node service.
5. Node disconnect/broadcast.
6. User list/detail.
7. Door list/detail/check.
8. Drop-file viewer.
9. Database backup/status.
10. Logs tail.
11. Message-area manager.
12. OxideNet admin when the network module exists.

### Final design direction

The OxideBBS Sysop TUI should be the thing a sysop leaves running in an SSH session.

It should feel like the modern descendant of classic BBS sysop screens:

```text
Live nodes.
Door runs.
User records.
Message areas.
Network packets.
Logs.
Config.
All keyboard-driven.
All local.
All fast.
All with just enough ANSI soul.
```

The guiding sentence:

> OxideBBS Sysop is a classic BBS control center rebuilt as a modern Rust terminal application.

---

# Final Consolidated Recommendation

Build the sysop administration experience in this order:

1. **CLI foundation**
   - `oxidebbs init`
   - `oxidebbs check`
   - `oxidebbs serve`
   - `oxidebbs status`
   - users, nodes, doors, database, logs

2. **TUI shell**
   - Ratatui app frame
   - navigation rail
   - header/footer
   - command palette shell
   - help modal shell
   - theme system

3. **Live operational screens**
   - dashboard
   - nodes
   - logs
   - database health

4. **Core admin screens**
   - users
   - doors
   - messages
   - ANSI assets
   - audit

5. **Door troubleshooting polish**
   - dry-run door test
   - generated drop-file viewer
   - door run history
   - active door kill/cleanup
   - door health checks

6. **OxideNet administration**
   - application review
   - node registry
   - config package generation
   - packet queues
   - nodelist generation

The first real-world target should be an **8-node Blackboard BBS** dashboard. The code should remain fully generic and never assume exactly 4 or 8 nodes.
