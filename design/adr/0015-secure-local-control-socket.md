# ADR 0015: Secure The Local Control Socket With Filesystem And Peer-UID Checks

## Status

Accepted

## Context

OxideBBS exposes a local Unix-domain control socket for sysop operations such as
status, node listing, node disconnect, node message, broadcast, and stale-node
reset.

Those commands can affect live callers. A local unprivileged user who can write
to the socket can disconnect callers or send messages that appear to come from
the sysop.

The control socket is local-only, but local-only does not mean unauthenticated.

## Decision

For v1, secure the control socket with filesystem permissions and peer UID
validation.

The server MUST:

- create the runtime directory with mode `0700` on Unix when it does not exist
- remove any stale socket before binding
- bind `runtime/oxidebbs-control.sock`
- chmod the socket path to `0600` immediately after binding
- reject control clients whose Unix peer UID is not the effective UID of the
  running OxideBBS process

The client MUST continue to use the configured runtime socket path. No shared
secret token is required for v1.

If peer UID inspection is unavailable on the target platform, the control
socket server MUST return an unsupported/unavailable error instead of accepting
unauthenticated commands.

Control request and response JSON shapes remain unchanged.

## Consequences

- A process running as a different local user cannot use the control socket even
  if it can discover the path.
- Existing sysop CLI workflows remain simple for the same OS user.
- No new token lifecycle or secret storage mechanism is needed for v1.
- Tests must use Unix-only assertions for socket mode and peer UID behavior.

## Rejected Options

- JSON shared secret token for v1: it adds token generation, storage, rotation,
  and redaction work without being necessary for the local same-user CLI.
- Filesystem permissions only: this is weaker if a directory permission bug or
  inherited socket mode exposes the path.
- Network control API: out of scope for v1.
