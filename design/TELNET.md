# Telnet Design

## v1 Goal

Provide stable telnet access for classic BBS clients, especially
SyncTERM-style clients, while keeping ordinary command-line telnet sessions
readable.

Telnet is the only remote transport for v1. Serial/modem support is deferred.

## Required Capabilities

- Accept TCP connections.
- Assign each accepted caller to a node.
- Persist session lifecycle records.
- Handle IAC command bytes without corrupting caller data.
- Negotiate the small telnet option set needed for BBS callers.
- Detect ANSI/BBS terminal capability before the first caller screen.
- Detect caller window width when the client reports NAWS.
- Select ANSI, 40-column ANSI, ASCII, or text screen assets from the detected
  terminal capability.
- Normalize CR/LF input for menu commands and line prompts.
- Detect disconnect and idle timeout.
- Clean up node/session state on disconnect.

## Connection Lifecycle

On TCP accept, the server creates transport/input state, records the session,
marks the node connected, then runs terminal capability negotiation before
rendering the first caller screen.

Capability negotiation is deliberately short and non-fatal. If the caller does
not reply, the session continues as a plain text caller. If real caller data is
received during negotiation, it must be preserved and replayed into the normal
menu/input loop.

After negotiation:

- ANSI callers may receive the configured clear-screen sequence.
- Plain text callers do not receive ANSI clear-screen bytes.
- The configured terminal welcome asset is sent from `paths.ansi`; plain text
  callers first probe sibling `.asc` and `.txt` assets before falling back to a
  stripped `.ans` rendering.
- Login, post-login, and menu screens are selected from `paths.screens`.

## Initial Telnet Options

The server starts each caller with this conservative negotiation:

- `WILL ECHO`
- `WILL SUPPRESS-GO-AHEAD`
- `DO SUPPRESS-GO-AHEAD`
- `DO TERMINAL-TYPE`
- `DO NAWS`

`WILL ECHO` means the server is prepared to handle remote echo behavior for
interactive prompts. `SUPPRESS-GO-AHEAD` avoids legacy half-duplex go-ahead
traffic. `TERMINAL-TYPE` and `NAWS` are used only for capability detection and
screen asset selection.

The parser accepts only these options for v1:

- `ECHO`
- `SUPPRESS-GO-AHEAD`
- `TERMINAL-TYPE`
- `NAWS`

Unsupported peer `WILL` requests receive `DONT`; unsupported peer `DO` requests
receive `WONT`.

## Terminal Type Detection

Callers default to plain text. The server selects ANSI assets only after the
client reports a terminal type that explicitly identifies a BBS/ANSI-capable
client.

Current ANSI-positive terminal type markers:

- `SyncTERM`
- `ANSI`
- `ANSI-BBS`
- `BBS-ANSI`
- `ANSI.SYS`
- `PC-ANSI`
- `PCANSI`

Matching is case-insensitive. Some entries are substring matches so variant
strings such as `SyncTERM 1.x` still identify correctly.

Generic telnet terminal types such as `xterm`, `xterm-256color`, and `vt100`
are treated as plain text for v1, even though they may understand ANSI escapes.
This keeps ordinary telnet sessions readable and avoids sending CP437 ANSI art
to terminals that do not behave like BBS clients.

Terminal type negotiation sequence:

1. Server sends `DO TERMINAL-TYPE`.
2. If the client replies `WILL TERMINAL-TYPE`, the server sends
   `SB TERMINAL-TYPE SEND SE`.
3. If the client replies `SB TERMINAL-TYPE IS <name> SE`, the server evaluates
   `<name>` with the ANSI marker list above.

## Width Detection

NAWS (`Negotiate About Window Size`) updates the caller width when present.
Rows are parsed but not used for v1 screen selection.

Width rules:

- Default width is 80 columns.
- A NAWS column value greater than zero updates the caller width.
- ANSI callers at 40 columns or narrower select configured `ansi_40` assets.
- ANSI callers wider than 40 columns select configured `ansi` assets.
- Plain text callers, including plain 40-column callers, select `ascii` or
  `text` assets rather than ANSI assets.

If no NAWS reply is received, the caller remains at the default width.

## Screen Asset Selection

Terminal capabilities flow into the ANSI/CP437 screen loader.

For ANSI callers:

1. Use `ansi_40` when width is 40 columns or narrower and the asset exists.
2. Otherwise use `ansi` when configured.
3. Fall back to `ansi_40` if only the 40-column ANSI asset exists.

For plain text callers:

1. Use `ascii` when configured.
2. Otherwise use `text` when configured.
3. If only ANSI assets exist, strip ANSI escape sequences and render a plain
   CP437 text fallback.

Terminal-level `welcome_screen` and `logoff_screen` assets use the configured
asset name for ANSI callers. Plain text callers replace that asset extension
with `.asc` and then `.txt` under `paths.ansi`; if neither file exists, the
configured ANSI file is stripped as a compatibility fallback.

Missing screen assets produce a visible fallback payload that names the missing
screen and error details rather than silently dropping output.

## IAC and Subnegotiation Handling

The telnet parser must:

- Treat doubled `IAC IAC` as a literal data byte.
- Parse `WILL`, `WONT`, `DO`, and `DONT` option negotiation.
- Parse subnegotiation frames from `SB` through `SE`.
- Preserve escaped `IAC` bytes inside subnegotiation data.
- Emit typed events for terminal type and NAWS.
- Emit opaque subnegotiation events for unsupported subnegotiation options.

The parser is byte-oriented. It must not decode caller UI traffic as Unicode
before the terminal layer handles CP437/ANSI rendering.

## Input Handling

Menu input is single-key and case-insensitive for ASCII keys. CR/LF immediately
following a menu key is drained so pressing `N<enter>` does not leak the newline
into the next prompt.

Line prompts:

- Accept printable ASCII and spaces.
- Support backspace/delete.
- Normalize CR, LF, and CRLF endings.
- Optionally hide typed input for passwords.
- Ignore telnet negotiation events while reading a line.

Idle timeout applies to caller input waits. The short terminal negotiation
window is separate and must not be treated as caller idle timeout.

## Transport Abstraction

The BBS session should not know whether the caller is telnet or a future serial
transport. Transport code exposes byte reads, byte writes, and hangup:

```rust
pub trait Transport: Send {
    fn read_byte(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Option<u8>, TransportError>> + Send;

    fn write_all(
        &mut self,
        bytes: &[u8],
    ) -> impl std::future::Future<Output = Result<(), TransportError>> + Send;

    fn hangup(&mut self) -> impl std::future::Future<Output = Result<(), TransportError>> + Send;
}
```

## Testing

Use loopback transport tests for deterministic telnet behavior.

Required coverage:

- IAC negotiation replies.
- Escaped `IAC` data.
- Terminal type request/reply parsing.
- NAWS parsing.
- Capability negotiation defaulting to plain text when no client response
  arrives.
- SyncTERM/ANSI terminal type detection.
- NAWS 40-column selection of `ansi_40` for ANSI callers.
- NAWS 40-column selection of plain assets for non-ANSI callers.
- CR/LF drain after menu keys.
- Disconnect and idle timeout handling.

## v1 Non-Goals

- Physical modem or serial transport.
- TLS.
- Telnet binary mode.
- MCCP or other compression.
- ANSI probing by sending escape sequences and waiting for cursor reports.
- Full terminal emulation or client-specific rendering quirks beyond the
  terminal type and NAWS rules above.
- Automatic redraw/re-layout after a resize once a screen has already been
  rendered.
