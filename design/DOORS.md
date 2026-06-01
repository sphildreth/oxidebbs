# Door Runner Design

## Purpose

OxideBBS should make old DOS door games feel native.

## Required v1 behavior

- Door definitions in TOML
- Per-node runtime directories
- Drop-file generation
- Process launch
- COM1 serial bridge
- Timeout handling
- Disconnect cleanup
- Door run logging

## Door definition example

```toml
[[doors.definitions]]
key = "oxide-check"
name = "Oxide Door Check"
runner = "dosbox"
working_dir = "./tools/doors/oxide-door-check/dist"
command = "OXIDECHK.EXE"
drop_file = "DORINFO1.DEF"
exclusive = false
time_limit_minutes = 5
```

### Bundled test fixture

OxideBBS ships an owned DOSBox test fixture, not third-party game content:

- `Oxide Door Check` is defined by `key = "oxide-check"` in docs examples.
- The fixture source is Free Pascal (`tools/doors/oxide-door-check/src/oxidechk.pas`)
  targeting `i8086-msdos`.
- The checked-in executable `OXIDECHK.EXE` is a committed conformance fixture.
- The fixture checksum is validated via `tools/doors/oxide-door-check/SHA256SUMS`.
- Maintainers changing `oxidechk.pas` run
  `scripts/bootstrap-fpc-i8086-msdos.sh` and `scripts/build-oxidechk-door.sh`.

## Drop files

Support early:

- `DOOR.SYS`
- `DORINFO1.DEF`

Support later:

- `CHAIN.TXT`
- `DOORFILE.SR`
- Wildcat, PCBoard, and other variants as needed

For DOSBox execution:

- mount door working directory as `C:`
- mount node runtime directory as `D:`
- use `D:` as the current directory for command execution
- start a per-door bridge process for the door launch
- map `COM1` inside DOSBox using
  `serial1=nullmodem server:127.0.0.1 port:<bridge_port> transparent:1 rxdelay:1000 txdelay:10`
  to that bridge
- add `C:\` to DOS `PATH` and run bare commands by name so `D:` remains the
  current runtime directory for drop-file reads and report writes
- write `OXNODE.TXT` into the node runtime directory as Oxide-owned diagnostic metadata
  (not required by third-party doors)

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

The bridge uses the existing byte-oriented `Transport` trait and a per-door
run-local TCP endpoint; no split or new transport methods are required for this
phase. While a door is active, the normal session read loop is paused and the
bridge owns caller transport mediation. Menu routing, prompt editing, CR/LF
normalization, and line-based parsing do not run until the bridge returns.

Bridge behavior:

- Start a per-door Rust TCP bridge bound to localhost and expose it to the runner.
- Launch DOSBox with `serial1=nullmodem server:127.0.0.1 port:<bridge_port> transparent:1 rxdelay:1000 txdelay:10`
  so `COM1` reaches the bridge.
- Forward serial bytes bidirectionally between the bridge socket and the caller
  `Transport`.
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
