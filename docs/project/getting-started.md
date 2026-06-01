# Getting Started

OxideBBS is a local Rust BBS server with:

- Telnet caller runtime
- ANSI/CP437 rendering
- DecentDB persistence
- DOS door launch support
- CLI-first sysop operations

## Prerequisites

- Rust stable with `rustfmt` and `clippy`
- Node.js 20 or newer (documentation site only)
- Native DecentDB headers:

```bash
sudo apt-get install -y clang libclang-dev
```

From repository root, the normal validation command is:

```bash
./scripts/dev-check.sh
```

## 1) Create a board

```bash
cargo run -p oxidebbs-server -- setup
```

`setup` creates `config/oxidebbs.toml`, initializes `data/oxidebbs.ddb`,
creates the initial sysop account, and prepares runtime/asset directories.

For unattended setup, pass required values:

```bash
cargo run -p oxidebbs-server -- setup \
  --board-name "My BBS" \
  --sysop-alias sysop \
  --sysop-password "change-this" \
  --nodes 4
```

## 2) Validate config and runtime paths

```bash
cargo run -p oxidebbs-server -- check
cargo run -p oxidebbs-server -- config check
```

Validation checks:

- socket address parsing
- config paths and screen assets
- door definitions and runner availability
- drop-file format and timeout constraints
- runtime directory writability

`check` errors on missing/invalid configuration and reports warnings for optional
but missing directories or assets.

Install your distribution's DOSEMU2 package before live door testing. The
runtime executable is commonly named `dosemu`, but legacy `dosemu-1.x` is not
supported because it does not accept OxideBBS's run-local `pts <path>` COM1
mapping.

```bash
dosemu --version
```

The version output should identify DOSEMU2, not `dosemu-1.x`.

For Debian 13 LXC hosts, verify PTYs are present and writable by the runtime
user; DOSEMU2 uses this for `COM1` bridging:

```bash
test -d /dev/pts && ls -ld /dev/pts
```

Validate the bundled test door without DOSEMU2:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors check oxide-check
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors dropfile oxide-check --user sysop --node 1 --format DORINFO1.DEF
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors test oxide-check --user sysop --dry-run
```

Free Pascal (`i8086-msdos`) is only needed when maintaining
`tools/doors/oxide-door-check/src/oxidechk.pas`. `check`, `dropfile`, and
`--dry-run` validation do not need it.

The v1 live model does not use DOS console I/O. During a caller door session,
OxideBBS pauses normal menu parsing and forwards raw bytes through this path:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOSEMU2-emulated COM1 UART
  <-> DOS door program
```

OxideBBS starts DOSEMU2 with `COM1` mapped to the per-node PTY path:

```text
$_com1 = "pts <absolute/path/to/runtime/node-001/OXCOM1.PTY>"
```

Per-run DOSEMU2 config is written to `OXDOSEMU2.CONF` and includes:

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

When the caller presses a key, OxideBBS reads that byte from telnet and writes it
to the PTY bridge. DOSEMU2 receives it on `COM1` and passes it to the door. When
the door writes to COM1, DOSEMU2 emits those bytes on the PTY bridge, and
OxideBBS writes them to the caller's telnet connection. That means an end-to-end
smoke test validates both launch and serial transport behavior, not just console
I/O.

DOSEMU2 maps `COM1` directly to the host PTY, so this is not a Rust-hosted
FOSSIL driver. The host bridge is byte transport only. A DOS-side FOSSIL TSR can
be loaded inside DOSEMU2 if a specific door explicitly requires FOSSIL APIs.

In this model there is no SDL window and no display-server requirement for door
runtime.

## 3) Start serving

```bash
cargo run -p oxidebbs-server -- serve
```

`serve` binds telnet, accepts caller sessions, persists session/audit rows, and
starts the local Unix control socket:

```text
runtime/oxidebbs-control.sock
```

If that socket path is already active, startup fails to avoid clobbering an
already-running server.

## 4) Confirm runtime

```bash
cargo run -p oxidebbs-server -- status
cargo run -p oxidebbs-server -- nodes list
cargo run -p oxidebbs-server -- nodes watch
```

While running, node status comes from live runtime registry and includes heartbeat
age. If the socket is unreachable, status/list/watch read through active session
rows from DecentDB.

## 5) Use local sysop controls

```bash
cargo run -p oxidebbs-server -- nodes message 1 "Maintenance in 10 minutes."
cargo run -p oxidebbs-server -- nodes disconnect 1
cargo run -p oxidebbs-server -- nodes broadcast "Server restart at 00:00 UTC."
cargo run -p oxidebbs-server -- nodes reset-stale
```

When the control socket is unavailable, these commands still record explicit
sysop intent in audit rows and return explicit messaging explaining the delivery
gap.

## 6) Doors and data safety checks

```bash
cargo run -p oxidebbs-server -- doors check oxide-check
cargo run -p oxidebbs-server -- doors test oxide-check --user sysop --dry-run
```

Caller `Doors` menu launch uses live door execution in the caller path and records
door run rows with timeout and byte counters. With DOSEMU2 configured and
`oxide-check` enabled, run the door from the caller `Doors` menu to complete an
end-to-end live test and confirm:

- caller keystrokes reach the door through DOSEMU2 COM1,
- door output written to COM1 reaches the caller telnet session,
- the door responds to keyboard input,
- and `OXIDECHK.RPT`/`OXNODE.TXT` are created under the node runtime directory.

Optional live smoke test command:

```bash
OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
```

## 7) Backup, export, and restore

```bash
cargo run -p oxidebbs-server -- db backup backups/oxidebbs.ddb
cargo run -p oxidebbs-server -- db export --format json > backups/oxidebbs.json
cargo run -p oxidebbs-server -- db import --format json backups/oxidebbs.json
```

`db import --format json` is a full restore into schema-only targets only. It is
transactional and validates IDs, relationships, and schema compatibility.

`db compact` is explicitly unsupported in this release because DecentDB has no
safe compaction API contract.

## 8) Local-only boundary

There is no remote web or TCP admin interface in this phase. All operational
control is local to the host running `oxidebbs-server`.
