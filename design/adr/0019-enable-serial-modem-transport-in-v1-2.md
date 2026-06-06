# ADR 0019: Enable Serial And Modem Caller Transport In v1.2

## Status

Accepted

## Context

ADR 0004 kept serial and modem support out of v1 while requiring a transport
abstraction that would allow it later. v1.2 is the release that activates that
later transport work.

Serial support adds hardware, line-state, modem initialization, and deployment
complexity. It must not leak into telnet session logic.

## Decision

v1.2 adds `SerialTransport` as a first-class caller transport alongside
`TelnetTransport`.

The session layer continues to depend only on the byte-oriented transport
interface. Serial-specific behavior lives in a transport module and server
listener/acceptor code.

The configuration model must add a disabled-by-default `[serial]` section with:

- `enabled`
- `devices`, each with `name`, `path`, `baud_rate`, `data_bits`, `parity`,
  `stop_bits`, `flow_control`, `answer_mode`, and optional modem init strings
- `require_carrier_detect`
- `drop_dtr_on_hangup`

The runtime must support:

- direct attached serial devices
- modem initialization strings
- carrier detect handling where the platform exposes it
- normal session cleanup on carrier loss
- operator-facing errors for unsupported line-state features

## Consequences

- Telnet remains enabled and unchanged by default.
- Serial devices are opt-in and must not bind or open when `[serial].enabled`
  is false.
- Tests should use loopback or pseudo-terminal fixtures where possible.
- Hardware smoke tests are optional but must be documented.
