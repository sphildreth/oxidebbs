# Serial And Modem Transport

Telnet remains the only release-ready caller transport today. Serial/modem
transport is tracked by the v1.2 release plan and is disabled by default.

Current foundation:

- A `[serial]` configuration section can model operator intent without opening
  device files by default.
- The transport boundary keeps caller sessions independent of whether bytes come
  from telnet or another byte stream.
- Tests can use in-memory or loopback-style transports for serial-adjacent
  behavior.

Not release-ready yet:

- Opening and serving configured physical TTY devices.
- Modem line-state handling and platform-specific operator errors.
- Multi-device serial listener orchestration.
- Hardware or pseudo-terminal smoke coverage that completes login, menu input,
  and logoff through a real serial path.

Operators should continue to expose callers through telnet unless they are
working on the P4 implementation and validation path.
