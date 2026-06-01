# Oxide Door Check

Oxide Door Check is an OxideBBS-owned DOS door fixture. It exists to verify the
same DOSBox launch path that real DOS doors use, without bundling third-party
door software.

The checked-in `dist/OXIDECHK.EXE` file is the conformance-test fixture used by
normal development and sysop testing. Normal OxideBBS builds, Cargo tests, and
`./scripts/dev-check.sh` do not rebuild this executable and do not require Free
Pascal, DOSBox, or the i8086 cross compiler.

Only maintainers changing `src/oxidechk.pas` need the Free Pascal i8086/MS-DOS
cross compiler.

## Runtime

Sysops need DOSBox only when running the door live from the caller `Doors` menu.
The door speaks through COM1 UART I/O; OxideBBS creates a local TCP serial bridge
for each run and starts DOSBox with COM1 mapped to that bridge.

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml serve
```

Then connect over telnet, log in, open the `Doors` menu, and select
`oxide-check`.

Dry runs and checksum verification do not require DOSBox:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors test oxide-check --user sysop --dry-run
```

Maintainers with DOSBox and `nc` installed can run the optional automated COM1
smoke test:

```bash
OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosbox.sh
```

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
compiler under `target/fpc-i8086-msdos/`. It does not install packages, does
not use `sudo`, and does not write outside the repository.

The fixture currently targets Free Pascal 3.2.2 `i8086-msdos` with the small
memory model and smartlinking (`-Wmsmall -CX -XX`). The generated executable is
a plain DOS MZ executable intended to run in DOSBox without GO32v2, CWSDPMI, a
DPMI host, overlays, DLLs, or separate Pascal runtime files.
