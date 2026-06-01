# ADR 0010: Use DOSEMU2 For DOS Door Runtime

## Status

Accepted

## Context

OxideBBS v1 needs to run classic DOS door programs for telnet callers. The
initial implementation used DOSBox to prove drop-file generation, DOS process
launching, and COM1 byte bridging.

That path works as a development proof, but it is not the right long-term
server runtime. Plain DOSBox is SDL/window oriented. In a Proxmox LXC or other
headless server environment, running DOSBox without a visible window requires an
Xvfb wrapper. That is a workaround around the runtime's desktop assumptions,
not a clean production model for a BBS server.

DOSEMU2 is designed to run DOS programs under Linux and exposes DOS COM port
mapping through runtime configuration. Its documented serial backends include
Linux devices, `virtual`, `exec <command>`, `pts <name>`, `vmodem`, and
`nullmodem`.

OxideBBS already owns the caller telnet connection, node state, auditing,
timeouts, and disconnect behavior. Therefore the DOS runtime should expose a
local serial endpoint to OxideBBS, not become the network-facing telnet server.

## Decision

Use DOSEMU2 as the v1 DOS door runtime.

The preferred serial integration is DOSEMU2 `COM1` mapped to a run-local host
pseudo-terminal:

```text
$_com1 = "pts /absolute/path/to/runtime/node-001/OXCOM1.PTY"
```

OxideBBS will bridge bytes between the existing caller `Transport` and the PTY.
The DOS door will continue to read and write `COM1` as a DOS serial port.

The intended runtime flow is:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOSEMU2-emulated COM1 UART
  <-> DOS door program
```

The generated DOSEMU2 config should prefer container-safe operation:

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

The DOSEMU2 integration must be proven in Debian 13 LXC before the refactor is
considered complete.

## Consequences

- OxideBBS no longer needs Xvfb to hide a DOSBox window.
- The v1 door runtime is Linux/server/container oriented.
- The implementation must add PTY handling and raw terminal configuration.
- The implementation must clearly document any Debian 13 LXC package or build
  requirements for DOSEMU2.
- The bundled `OXIDECHK.EXE` test door remains valuable because it validates a
  real DOS COM1 path.
- OxideBBS still does not become a DOS-side FOSSIL driver. A FOSSIL driver is a
  DOS TSR/API component that can be loaded inside the DOSEMU2 environment later
  if a specific door requires one.
- If the `pts <path>` backend is not viable, the fallback order is:
  `exec <command>`, then `virtual`, then `vmodem`. The fallback decision must be
  documented before implementation continues.

## References

- DOSEMU2 project page: `https://dosemu2.github.io/dosemu2/`
- DOSEMU2 README: `https://github.com/dosemu2/dosemu2`
- DOSEMU2 runtime configuration options:
  `https://github-wiki-see.page/m/dosemu2/dosemu2/wiki/Runtime-Configuration-Options`
- DOSEMU2 sample serial configuration:
  `https://raw.githubusercontent.com/dosemu2/dosemu2/devel/etc/dosemu.conf`
