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
[[doors]]
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

## Legal note

Do not bundle doors unless their license clearly allows redistribution.
