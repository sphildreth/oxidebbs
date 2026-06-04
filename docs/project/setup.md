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

`--data` can also point at a directory. Directory paths, including paths written
with a trailing slash, resolve to `oxidebbs.ddb` inside that directory.

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
- whether to install bundled sample ANSI/screen assets

Press Enter at a prompt to accept the value shown in brackets.

Setup writes the TOML config, creates required directories, installs the bundled
ANSI/screen assets when requested, initializes a schema `8` DecentDB database,
creates the initial sysop account, and creates the
default `general` local message area. The generated sysop account is real data,
so a setup-created database is not an empty restore target for `db import`.
Generated configs include the v1 authentication defaults: five failed attempts
within ten minutes lock the IP or alias scope for fifteen minutes, new callers
start at security level `10`, and Argon2id uses `memory_cost_kib = 19456`,
`iterations = 2`, and `parallelism = 1`.
See [User Security Levels](./security-levels.md) for the current sysop-facing
security-level policy and enforced gates.
Generated configs also include `[audit].retention_days = 365`; retention cleanup
is an explicit maintenance operation, not an automatic side effect of audit
inserts.
Generated configs also enable file logging under `logs/oxidebbs-server.log`.
See [Sysop CLI](./sysop-cli.md#logging) for logging levels, JSON file format,
rotation, and override precedence.

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
the board name, sysop alias, telnet port or full bind address, node count, and
sample ANSI creation. `--telnet-port` keeps the bind on `127.0.0.1`; use
`--telnet-bind` only for an explicit plaintext exposure decision.
Use `--enable-example-door` when a generated config should enable the bundled
`oxide-check` test door immediately, as the Docker first-boot path does.

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

The setup command may include a sample door definition and installs the bundled
Oxide-owned test fixture under `./doors/oxide-door-check/dist`. The example
definition points to that fixture (`oxide-check` -> `OXIDECHK.EXE`) and is
intended for launch-path validation.

To avoid third-party licensing issues, OxideBBS ships no abandonware/shareware
DOOR packages and does not bundle third-party doors.

Sysops validating doors should use commands like:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml doors check oxide-check
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml doors dropfile oxide-check --user sysop --node 1 --format DORINFO1.DEF
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml doors test oxide-check --user sysop --dry-run
```

Dry-run verifies drop-file generation without requiring DOSEMU2.

Before first live test:

1. Install your distribution's DOSEMU2 package on the host. The executable is
   commonly named `dosemu`, but legacy `dosemu-1.x` is not supported because it
   does not accept OxideBBS's run-local `pts <path>` COM1 mapping:

```bash
dosemu --version
```

   On Fedora, follow [DOSEMU2 on Fedora](/project/dosemu2-fedora) before live
   door testing. The validated Fedora setup uses the `stsp/dosemu2` Copr
   packages and must expose `libdj64.so.0` through `ldconfig`.

2. Confirm `/dev/pts` is available when running in Debian 13 LXC:

```bash
test -d /dev/pts && ls -ld /dev/pts
```

3. Enable `oxide-check` in the config if it is disabled.
4. Run:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml check
```

5. Start `serve`, connect as a caller, and launch the test door from the `Doors`
menu.

The corrected v1 model is:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOSEMU2-emulated COM1 UART
  <-> DOS door program
```

DOSEMU2 uses `$_com1 = "pts <runtime>/OXCOM1.PTY"` and writes per-run settings
into `OXDOSEMU2.CONF` in the node runtime directory.

Success criteria for the live smoke test:

- `OXIDECHK.EXE` appears to run as a door on screen through the COM1 bridge.
- The test screen should respond to keystrokes and exit cleanly from `Q`.
- On successful exit, `OXNODE.TXT` and `OXIDECHK.RPT` appear in the node runtime
  directory and include matching node metadata.

Notes:

- Maintainers only need Free Pascal and the staged `i8086-msdos` toolchain when
generating `tools/doors/oxide-door-check/src/oxidechk.pas`.
- Normal sysop validation, dry-run validation, and live sysop testing do **not**
require Free Pascal or `i8086-msdos`.
- This DOSEMU2 model requires no GUI server, display server, or SDL window.
