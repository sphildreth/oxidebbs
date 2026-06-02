# ADR 0013: Use Local-First Defaults For Plaintext Telnet

## Status

Accepted

## Context

OxideBBS v1 is telnet-only by project constraint. Telnet is plaintext: caller
passwords, messages, menu traffic, and door I/O are visible to any party that
can observe the network path.

The initial example configuration listened on `0.0.0.0:2323`, and telnet is
enabled by default. That is convenient for demos, containers, and local network
testing, but it is not a safe default for an operator who starts the server on a
machine that is reachable from untrusted networks.

TLS is not part of v1. Adding TLS would either change the v1 transport scope or
require a separate proxy/deployment decision.

## Decision

Keep telnet as the v1 remote transport, but make the runtime local-first by
default.

The default telnet bind address MUST be `127.0.0.1:2323` in:

- `TelnetConfig::default`
- `config/oxidebbs.example.toml`
- generated setup config
- documentation examples unless the example is explicitly about public
  deployment

`telnet.enabled` remains `true` by default so local development and first-run
setup still work without extra configuration.

Any documentation or configuration path that shows a public bind such as
`0.0.0.0:2323` MUST include a warning that telnet is plaintext and should be
exposed only on trusted networks, through an operator-controlled proxy, or in a
deliberately public retro-BBS deployment.

The config validation/check command MUST warn, not fail, when telnet binds to:

- `0.0.0.0`
- `[::]`
- a non-loopback IPv4 or IPv6 address

The warning text MUST mention that credentials and caller traffic are sent in
plaintext.

## Consequences

- A default local run is not accidentally exposed to a LAN or public interface.
- Operators can still opt into public telnet by editing the config.
- v1 remains compatible with classic BBS telnet clients.
- TLS support remains outside the v1 implementation scope.
- Deployment docs must be explicit about plaintext risk.

## Rejected Options

- Disable telnet by default: this would make the first local run less useful
  and would not remove the need to document plaintext telnet.
- Add TLS directly to v1: this conflicts with the current telnet-only v1 scope
  and would require a separate certificate and client compatibility decision.
- Keep `0.0.0.0:2323` as the default: this optimizes demos at the cost of an
  unsafe operator default.
