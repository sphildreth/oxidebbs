# DOSEMU2 On Fedora

OxideBBS v1 DOS doors require DOSEMU2, not legacy DOSEMU 1.x. On Fedora the
runtime executable is still named `dosemu`, so always verify the version before
debugging door behavior.

## Expected Runtime

The correct runtime reports a DOSEMU2 version:

```bash
dosemu --version
```

Expected shape:

```text
dosemu2-2.0pre9
Revision: ...
```

This is not sufficient:

```text
dosemu-1.4.0.8-...
```

Legacy `dosemu-1.x` does not support the run-local `pts <path>` COM1 mapping
used by OxideBBS. If `dosemu --version` reports `dosemu-1.x`, remove or replace
that package before live door testing.

## Install Packages

The Fedora 44 validation host used the DOSEMU2 packages from the `stsp/dosemu2`
Copr repository. Enable that repository and install the runtime packages:

```bash
sudo dnf copr enable stsp/dosemu2
sudo dnf install -y dosemu2 install-freedos comcom64 fdpp \
  dj64dev-dj64 dj64dev-djdev64
```

Verify the installed package set:

```bash
rpm -qa | grep -Ei 'dosemu2|install-freedos|comcom64|fdpp|dj64' | sort
```

The validated package set looked like:

```text
comcom64-...
dj64dev-dj64-...
dj64dev-djdev64-...
dosemu2-2.0pre9-...
fdpp-...
install-freedos-...
```

## Configure The DJ64 Loader Path

On the Fedora validation host, `dosemu` could start but failed while booting
`COMMAND.COM` until the DJ64 runtime library path was added to the system
dynamic loader cache.

The failure looked like:

```text
cannot dlopen .../COMMAND.COM: libdj64.so.0: cannot open shared object file
```

Confirm whether the loader already sees `libdj64.so.0`:

```bash
ldconfig -p | grep libdj64
```

Expected shape:

```text
libdj64.so.0 (libc6,x86-64) => /usr/i386-pc-dj64/lib64/libdj64.so.0
```

If no `libdj64.so.0` entry appears, add the DJ64 library directory and refresh
the loader cache:

```bash
echo /usr/i386-pc-dj64/lib64 | sudo tee /etc/ld.so.conf.d/dj64.conf
sudo ldconfig
ldconfig -p | grep libdj64
```

For temporary local testing only, `LD_LIBRARY_PATH=/usr/i386-pc-dj64/lib64` may
help with some direct commands, but OxideBBS service deployments should use the
loader-cache configuration above so `systemd` and non-interactive shells behave
the same way as manual tests.

## Verify DOSEMU2 Boots

Run a minimal command:

```bash
dosemu -dumb -quiet -E ver
```

This should boot DOSEMU2/FDPP and print version output instead of failing with
`libdj64.so.0`.

If `~/.dosemu/drive_c` is empty, that is not automatically a blocker for the
Oxide Door Check fixture. The Fedora DOSEMU2 packages boot FDPP and `comcom64`
from system paths. The critical check is that `dosemu -dumb -quiet -E ver`
starts without the DJ64 loader error.

## Validate Oxide Door Check

Normal Rust builds and normal Rust tests do not require DOSEMU2:

```bash
./scripts/dev-check.sh
```

Dry-run door validation also does not require DOSEMU2:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml \
  doors test oxide-check --user sysop --dry-run
```

Once Fedora DOSEMU2 is installed and `libdj64.so.0` is visible through
`ldconfig`, run the optional live smoke tests:

```bash
OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
```

Expected result:

```text
DOSEMU2 smoke test passed
```

Then verify separate node runtime behavior:

```bash
OXIDE_DOOR_INTERACTIVE=1 OXIDE_DOOR_MULTI_NODE=1 \
  ./scripts/test-oxide-door-dosemu2.sh
```

Expected result:

```text
multi-node DOSEMU2 smoke test passed
```

The smoke script launches the checked-in `OXIDECHK.EXE` fixture through DOSEMU2,
maps COM1 to a run-local PTY, sends `I`, `R`, and `Q`, and verifies
`OXIDECHK.RPT` creation. It exits `77` with a `SKIP:` message when DOSEMU2 is
missing or when `dosemu` is legacy DOSEMU 1.x.

## Troubleshooting

If the smoke script says legacy DOSEMU is installed:

```text
SKIP: DOSEMU2 is required; dosemu is legacy dosemu-1...
```

Check which package owns the binary:

```bash
rpm -qf "$(command -v dosemu)"
dosemu --version
```

Install DOSEMU2 from the Copr repository and remove the legacy runtime if it is
still taking precedence on `PATH`.

If the smoke test times out waiting for `OXCOM1.PTY`, inspect the latest
DOSEMU2 boot log:

```bash
sed -n '1,220p' ~/.dosemu/boot.log
```

The generated per-run DOSEMU2 config should contain:

```text
$_com1 = "pts <repo>/target/.../node-001/OXCOM1.PTY"
```

If the PTY appears but the door exits immediately, verify the current code is
staging the door executable into the node runtime and launching it by DOS
filename. The expected DOSEMU2 command model is:

```text
dosemu -dumb -quiet -K <node runtime> -f <node runtime>/OXDOSEMU2.CONF -E OXIDECHK.EXE
```

Do not use an absolute host path as the DOS command for `OXIDECHK.EXE`; that can
cause the program to run outside the node runtime and miss its drop files.
