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
runner = "dosemu"
working_dir = "./doors/oxide-door-check/dist"
command = "OXIDECHK.EXE"
drop_file = "DORINFO1.DEF"
exclusive = false
time_limit_minutes = 5
```

### Bundled test fixture

OxideBBS ships an owned DOS test fixture, not third-party game content:

- `Oxide Door Check` is defined by `key = "oxide-check"` in docs examples.
- The fixture source is Free Pascal (`tools/doors/oxide-door-check/src/oxidechk.pas`)
  targeting `i8086-msdos`.
- The checked-in executable `OXIDECHK.EXE` is a committed conformance-test fixture.
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

For DOSEMU2 execution:

- run door executable in node runtime directory,
- generate per-run `OXDOSEMU2.CONF`, and
- map `COM1` with the host PTY path:

```text
$_com1 = "pts /absolute/path/to/runtime/node-001/OXCOM1.PTY"
```

- apply container-safe DOSEMU2 defaults in that config:

```text
$_cpu_vm = "emulated"
$_cpu_vm_dpmi = "emulated"
$_sound = (off)
$_mouse_internal = (off)
$_joy_device = ""
$_pktdriver = (off)
$_tcpdriver = (off)
$_ttylocks = ""
```

- write `OXNODE.TXT` into the node runtime directory as Oxide-owned diagnostic
  metadata (not required by third-party doors)

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
DoorRunner launches DOSEMU2
    ↓
I/O bridge connects process and caller
    ↓
Timeout/disconnect cleanup
    ↓
Door run saved to DecentDB
```

## Live caller launch

The caller-facing `Doors` menu action is a live runtime path. Authenticated
callers see enabled door definitions, can select by key or list number, and return
to the main menu after a normal child exit or enforced timeout. The session
rejects disabled doors, missing working directories, missing runner executables,
unwritable node runtime directories, and non-positive time limits before launch.

The caller-facing byte path during runtime is:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOSEMU2-emulated COM1 UART
  <-> DOS door program
```

The server records a `door_started` audit event and inserts a `door_runs` row
after the drop file is generated. When the child exits, times out, or is killed
because the caller/sysop disconnects, the server records `door_finished` or
`door_timed_out` and finishes the `door_runs` row with exit code, timeout/forced
flags, and byte counters.

`oxidebbs-door` remains responsible for drop-file rendering, per-node runtime
directory helpers, and `DoorRunPlan` construction. The live interactive bridge
lives in `oxidebbs-server` as a server adapter around that plan so core session
logic does not know child-process details.

## Byte-bridge contract

The bridge uses the existing byte-oriented `Transport` trait and a DOSEMU2
per-door host PTY endpoint; no split or new transport methods are required for
this phase.

Bridge behavior:

- Create a per-door runtime PTY file before launch, then launch DOSEMU2 with
  per-run config.
- Launch DOSEMU2 with `$_com1` and container-safe runtime settings.
- Wait for the PTY endpoint path to appear in the runtime directory.
- Open the PTY as raw byte device and disable console line discipline.
- Forward serial bytes bidirectionally between the caller transport and PTY bridge.
- Keep heartbeats fresh while the bridge is active.
- Watch the local runtime command channel for sysop messages and disconnects.
- Kill the child on timeout, caller disconnect, or sysop disconnect.
- Clean up the node runtime directory after run finalization.

Normal child exit and timeout return the caller to the BBS main menu. Caller
disconnect and sysop disconnect leave the bridge through the normal session
disconnect path. The node registry reports `in_door` while the bridge owns the
transport.

## FOSSIL and transport model

The DOSEMU2 COM1 bridge is host-owned byte transport, not a Rust FOSSIL TSR or
DOS interrupt driver replacement. DOSEMU2 remains responsible for presenting COM1
UART semantics to DOS door programs.
