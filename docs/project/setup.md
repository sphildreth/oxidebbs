# Setup Wizard

OxideBBS includes an interactive setup command for creating a new local board
configuration.

Run it from the repository root:

```bash
cargo run -p oxidebbs-server -- setup
```

By default the wizard writes:

```text
config/oxidebbs.toml
```

Use `--output` to choose another config path:

```bash
cargo run -p oxidebbs-server -- setup --output config/my-bbs.toml
```

Use the global `--data` option when setup should initialize a database path
other than the generated config default:

```bash
cargo run -p oxidebbs-server -- --data /srv/oxidebbs/oxidebbs.ddb setup
```

The wizard asks for:

- board name
- tagline
- sysop name
- sysop alias
- sysop password
- timezone
- telnet bind address
- node count
- database path
- whether to include the placeholder example door definition
- whether to create sample ANSI screen directories

Press Enter at a prompt to accept the value shown in brackets.

Setup writes the TOML config, creates required directories, initializes a
schema `3` DecentDB database, creates the initial sysop account, and creates the
default `general` local message area. The generated sysop account is real data,
so a setup-created database is not an empty restore target for `db import`.

After setup, `serve` can use the generated file directly:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml serve
```

## Created Paths

Setup creates the starter directories used by the generated config:

```text
config/
data/
assets/ansi/
assets/screens/
doors/
runtime/
logs/
```

The command will not overwrite an existing output file unless `--force` is
provided:

```bash
cargo run -p oxidebbs-server -- setup --force
```

For unattended setup, provide the required values as flags:

```bash
cargo run -p oxidebbs-server -- setup \
  --board-name "My BBS" \
  --sysop-alias sysop \
  --sysop-password "change-this" \
  --nodes 4
```

Non-interactive setup requires `--sysop-password`. Optional flags can override
the board name, sysop alias, telnet port, node count, and sample ANSI creation.

## After Setup

Validate the generated config:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml check
```

The same validation is also available as:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml config check
```

Then inspect local sysop commands:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml users list
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml nodes list
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml db stats
```

The setup command may include a sample door definition, but it does not install
or bundle any DOS game door binaries. The example definition currently points to
the bundled Oxide-owned test door (`oxide-check` → `OXIDECHK.EXE`) and is intended
for launch-path validation.

To avoid third-party licensing issues, OxideBBS ships no abandonware/shareware
DOOR packages and does not bundle third-party doors.

Sysops validating doors should use commands like:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml doors check oxide-check
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml doors dropfile oxide-check --user sysop --node 1 --format DORINFO1.DEF
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml doors test oxide-check --user sysop --dry-run
```

Dry-run verifies drop-file generation without requiring DOSBox.

Before first live test:

1. Install DOSBox.
2. Enable `oxide-check` in the config if it is disabled.
3. Run:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml check
```

4. Run live smoke test after starting `serve` and connecting as a caller:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml serve
```

From a BBS caller session: navigate to the Doors menu and launch the test door.
The corrected v1 model is:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> run-local 127.0.0.1 TCP bridge
  <-> DOSBox nullmodem serial backend
  <-> DOSBox-emulated COM1 UART
  <-> DOS door program
```

The bridge is local loopback:

```text
serial1=nullmodem server:127.0.0.1 port:<bridge_port> transparent:1 rxdelay:1000 txdelay:10
```

OxideBBS receives caller telnet bytes, writes them to the bridge socket, and
DOSBox exposes them to the door as COM1 input. Door output follows the same path
in reverse: the door writes to COM1, DOSBox sends those bytes to the bridge
socket, and OxideBBS writes them to the caller telnet connection. The caller can
run the test door only when DOSBox can successfully start the serial endpoint.

Success criteria for the live smoke test:

- `OXIDECHK.EXE` appears to run as a door on screen through the COM1 bridge.
- The test screen should respond to keystrokes and exit cleanly from `Q`.
- On successful exit, `OXNODE.TXT` and `OXIDECHK.RPT` appear in the node runtime
  directory and include matching node metadata.

Notes:

- Maintainership-only rebuild of `OXIDECHK.EXE` requires Free Pascal and the
  staged `i8086-msdos` toolchain.
- Normal sysop validation, dry-run validation, and live sysop testing do **not** require
  Free Pascal or `i8086-msdos`.
