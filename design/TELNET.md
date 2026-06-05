# Telnet Design

## v1 Goal

Provide stable telnet access for classic BBS clients, especially
SyncTERM-style clients, while keeping ordinary command-line telnet sessions
readable.

Telnet is the only remote transport for v1. Serial/modem support is available since v1.2.

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
- Support the named `c64` terminal profile for C64, C64 Ultimate, and
  PETSCII-friendly 40-column terminal clients.
- Normalize CR/LF input for menu commands and line prompts.
- Normalize common backspace/delete bytes, including `0x08` and `0x7f`.
- Continue basic caller access over ordinary telnet/raw TCP style clients
  without requiring SSH, TLS-only access, a web client, or modern terminal
  negotiation.
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

## C64/C64 Ultimate Profile

The `c64` profile represents callers using C64, C64 Ultimate, or C64 terminal
applications. This is not a C64 port of OxideBBS; the server still runs as a
modern Rust process and exposes caller access over the normal transport.

Default profile capabilities:

- Width: 40 columns
- Height: 25 rows
- ANSI: disabled by default
- Color/control sequences: not required for basic navigation
- Charset: PETSCII-friendly ASCII fallback until full PETSCII translation is
  implemented
- Line endings: CRLF-normalized caller output; CR, LF, CRLF, and telnet CR-NUL
  accepted as input endings
- Backspace/delete: `0x08` and `0x7f`
- Optional output pacing: configurable as bytes per second

Terminal type markers such as `C64`, `C64 Ultimate`, `Ultimate 64`, `PETSCII`,
and `CGTerm` should select the C64 profile. If detection is absent or
unreliable, the configured `terminal.default_profile` and future manual profile
selection path are the fallback.

Manual selection should offer:

- ANSI / 80-column
- Plain ASCII
- C64 / 40-column / PETSCII-friendly

Persisting a per-user preference is future work until the user profile schema
has a terminal preference field.

## Width Detection

NAWS (`Negotiate About Window Size`) updates the caller width when present.
Rows are parsed but not used for v1 screen selection.

Width rules:

- Default width is 80 columns.
- The C64 profile default width is 40 columns.
- A NAWS column value greater than zero updates the caller width.
- ANSI callers at 40 columns or narrower select configured `ansi_40` assets.
- ANSI callers wider than 40 columns select configured `ansi` assets.
- Plain text callers, including C64 and other plain 40-column callers, select
  `ascii_40` or `text_40` when configured, then `ascii` or `text` assets rather
  than ANSI assets.

If no NAWS reply is received, the caller remains at the default width.

## Screen Asset Selection

Terminal capabilities flow into the ANSI/CP437 screen loader.

For ANSI callers:

1. Use `ansi_40` when width is 40 columns or narrower and the asset exists.
2. Otherwise use `ansi` when configured.
3. Fall back to `ansi_40` if only the 40-column ANSI asset exists.

For plain text callers:

1. Use `ascii_40` or `text_40` for callers at 40 columns or narrower when
   configured.
2. Use `ascii` when configured.
3. Otherwise use `text` when configured.
4. If only ANSI assets exist, strip ANSI escape sequences and render a plain
   CP437 text fallback.

Terminal-level `welcome_screen` and `logoff_screen` assets use the configured
asset name for ANSI callers. Plain text callers replace that asset extension
with `-40.asc` and `-40.txt` first for 40-column callers, then `.asc` and
`.txt` under `paths.ansi`; if none exists, the configured ANSI file is stripped
as a compatibility fallback.

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
- Support backspace/delete with both `0x08` and `0x7f`.
- Normalize CR, LF, and CRLF endings.
- Optionally hide typed input for passwords.
- Ignore telnet negotiation events while reading a line.

Idle timeout applies to caller input waits. The short terminal negotiation
window is separate and must not be treated as caller idle timeout.

## Transport Abstraction

The BBS session should not know whether the caller is telnet or a serial
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
