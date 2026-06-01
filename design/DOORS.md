# Door Runner Design

## Purpose

OxideBBS should make old DOS door games feel native.

## Required v1 behavior

- Door definitions in TOML
- Per-node runtime directories
- Drop-file generation
- Process launch
- I/O bridge
- Timeout handling
- Disconnect cleanup
- Door run logging

## Door definition example

```toml
[[doors.definitions]]
key = "lord"
name = "Legend of the Red Dragon"
runner = "dosbox"
working_dir = "./doors/lord"
command = "LORD.EXE"
drop_file = "DORINFO1.DEF"
exclusive = false
time_limit_minutes = 30
```

## Drop files

Support early:

- `DOOR.SYS`
- `DORINFO1.DEF`

Support later:

- `CHAIN.TXT`
- `DOORFILE.SR`
- Wildcat, PCBoard, and other variants as needed

## Runtime flow

```text
User selects door
    ↓
Session asks DoorService
    ↓
NodeManager validates availability
    ↓
DoorService creates runtime dir
    ↓
DropFileWriter writes files
    ↓
DoorRunner launches DOSBox
    ↓
I/O bridge connects process and caller
    ↓
Timeout/disconnect cleanup
    ↓
Door run saved to DecentDB
```

## Live caller launch

The caller-facing `Doors` menu action is a live runtime path. Authenticated
callers see enabled door definitions, can select by key or list number, and
return to the main menu after a normal child exit or enforced timeout. The
session rejects disabled doors, missing working directories, missing runner
executables, unsupported drop-file formats, unwritable node runtime
directories, and non-positive time limits before launch.

The server records a `door_started` audit event and inserts a `door_runs` row
after the drop file is generated. When the child exits, times out, or is killed
because the caller/sysop disconnected the door session, the server records
`door_finished` or `door_timed_out` and finishes the `door_runs` row with exit
code, timeout/forced flags, and byte counters.

`oxidebbs-door` remains responsible for drop-file rendering, per-node runtime
directory helpers, and `DoorRunPlan` construction. The live interactive bridge
lives in `oxidebbs-server` as a server adapter around that plan so core session
logic does not know child-process details.

## Byte-bridge contract

The bridge uses the existing byte-oriented `Transport` trait; no split or new
transport methods are required for this phase. While a door is active, the
normal session read loop is paused and the bridge borrows the caller transport.
Menu routing, prompt editing, CR/LF normalization, and line-based parsing do
not run until the bridge returns.

Bridge behavior:

- Spawn the configured runner with piped stdin, stdout, and stderr.
- Forward raw caller bytes from `Transport::read_byte()` to child stdin without
  menu parsing.
- Forward child stdout and stderr bytes directly to the caller transport.
- Keep heartbeats fresh while the door bridge is active.
- Watch the local runtime command channel for sysop messages and disconnects.
- Kill the child on timeout, caller disconnect, or sysop disconnect.
- Clean up the node runtime directory after the run is finalized.

Normal child exit and timeout return the caller to the BBS main menu. Caller
disconnect and sysop disconnect leave the door bridge through the normal session
disconnect path. The node registry reports `in_door` while the bridge owns the
transport.

## Legal note

Do not bundle doors unless their license clearly allows redistribution.
