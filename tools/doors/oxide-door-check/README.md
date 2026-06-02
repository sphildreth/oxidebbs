# Oxide Door Check

Oxide Door Check is an OxideBBS-owned DOS door fixture. It exists to verify the
same DOSEMU2 launch path that real DOS doors use, without bundling third-party
software.

The checked-in `dist/OXIDECHK.EXE` file is the conformance-test fixture used by
normal development and sysop testing. Normal OxideBBS builds, Cargo tests, and
`./scripts/dev-check.sh` do not rebuild this executable and do not require Free
Pascal, DOSEMU2, or the `i8086` cross compiler.

Only maintainers changing `src/oxidechk.pas` need the Free Pascal i8086/MS-DOS
cross compiler.

## Runtime

Sysops need DOSEMU2 only when running the door live from the caller `Doors`
menu.

The door speaks through COM1 UART I/O. OxideBBS creates a per-node PTY bridge and
starts DOSEMU2 with COM1 mapped as:

```text
$_com1 = "pts /absolute/path/to/runtime/node-001/OXCOM1.PTY"
```

The door path is:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOSEMU2-emulated COM1 UART
  <-> OXIDECHK.EXE
```

When the caller presses a key, OxideBBS reads that byte from telnet and writes it
to the PTY bridge. DOSEMU2 exposes the byte to `OXIDECHK.EXE` as COM1 input. When
`OXIDECHK.EXE` writes to COM1, DOSEMU2 sends those bytes back through the PTY
bridge, and OxideBBS writes them to the caller's telnet connection.

This is not a Rust-hosted FOSSIL driver; it is a host PTY bridge model.

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml serve
```

Then connect over telnet, log in, open the `Doors` menu, and select
`oxide-check`.

Dry runs and checksum verification do not require DOSEMU2:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors test oxide-check --user sysop --dry-run
```

Maintainers with DOSEMU2 can run the optional automated COM1 smoke test:

```bash
OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
```

The executable is commonly named `dosemu`, but it must be DOSEMU2. Legacy
`dosemu-1.x` does not accept OxideBBS's run-local `pts <path>` COM1 mapping, so
the smoke script skips that runtime with exit `77`.

On Fedora, see `docs/project/dosemu2-fedora.md` for the validated Copr package
set and the `libdj64.so.0` loader-cache check.

To run on systems with custom DOSEMU2 binaries:

```bash
DOSEMU_BIN=/path/to/dosemu OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
```

This mode is optional and must be clear-skip cleanly when DOSEMU2 is unavailable.

## Fixture Checksum

Verify the checked-in executable:

```bash
cd tools/doors/oxide-door-check
sha256sum -c SHA256SUMS
```

## Rebuilding

Rebuild only when `src/oxidechk.pas` changes:

```bash
./scripts/bootstrap-fpc-i8086-msdos.sh
./scripts/build-oxidechk-door.sh
```

The bootstrap script stages the official Free Pascal `i8086-msdos` cross
compiler under `target/fpc-i8086-msdos/`. It does not install packages, does not
use `sudo`, and does not write outside the repository.

The fixture currently targets Free Pascal 3.2.2 `i8086-msdos` with the small
memory model and smartlinking (`-Wmsmall -CX -XX`). The generated executable is
a plain DOS MZ executable intended to run in DOSEMU2 without GO32v2, CWSDPMI,
a separate DPMI host, overlays, DLLs, or separate runtime files.
