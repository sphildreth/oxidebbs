# Web Terminal Implementation Plan for OxideBBS

**Document**: `design/WEB_TERMINAL_PLAN.md`
**Status**: DRAFT - For implementation
**Target**: Browser-based ANSI/CP437 terminal at `/terminal`, with ZMODEM file
transfers added on the same byte stream after the raw terminal path is stable.

---

## Overview

This document outlines the simplest stable implementation plan for adding a
web-based terminal to OxideBBS. The terminal should provide an ANSI/CP437 caller
experience in the browser, making the whole browser tab feel like a terminal
connected to the BBS. The final feature target includes browser ZMODEM file
transfers, but the first implementation step is the raw terminal path: connect,
log in, navigate menus, and disconnect through the same session loop used by
telnet and serial callers.

Users should be able to open:

```text
http://<domain_or_ip>:8080/terminal
```

and see a full-tab terminal. OxideBBS still speaks plain HTTP on this listener;
HTTPS remains the job of a reverse proxy such as Caddy.

---

## KISS Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Browser URL | `GET /terminal` | One obvious user-facing entry point. |
| WebSocket URL | `GET /terminal/ws` | Keeps terminal traffic scoped under the terminal route. |
| Listener | Reuse the existing HTTP listener on port `8080` | Avoids a second web port, second static server, CORS, and bind conflicts. |
| Config | Add only `[web_terminal].enabled`, default `true` | Boards with the HTTP listener enabled get `/terminal` by default; sysops can disable terminal access without disabling monitoring. |
| Page shape | Full browser tab terminal | No dashboard chrome, cards, nav, or explanatory UI. |
| Backend model | `WsTransport` implements `oxidebbs_telnet::Transport` | Reuses `handle_caller_transport()` and existing login/menu/session behavior. |
| Protocol | Raw WebSocket binary frames | No custom browser protocol for the MVP. |
| Auth | BBS login prompt only | Same caller trust model as telnet. No JWT, no pre-auth, no cookies. |
| File transfers | Phase 2 on the same byte pipe | Prove the terminal first, then add `zmodem.js` without changing server protocol. |
| Frontend toolchain | Reuse the root Node package | Avoids a second `package.json` and second lockfile. |
| Font | Browser monospace for MVP | Avoids vendored font/licensing/package churn. DOS font can be optional later. |

---

## Architecture

The web terminal is a byte-level bridge:

```text
Browser xterm.js
  <-> WebSocket binary frames at /terminal/ws
  <-> WsTransport
  <-> existing raw caller path
  <-> handle_caller_transport()
```

The browser writes user keystrokes as bytes to the WebSocket. The server writes
BBS output bytes back as WebSocket binary messages. For the MVP, the browser
writes those bytes directly to xterm.js.

ZMODEM is a later phase on the same stream:

```text
WebSocket bytes -> zmodem.js sentry -> xterm.js or browser file workflow
```

No new file-transfer protocol is introduced. When ZMODEM is enabled, the server
still uses the existing `oxidebbs-transfer` implementation and
`TransportAdapter::new_raw()` because WebSocket binary frames do not need telnet
IAC escaping.

---

## Routing

Mount these routes on the existing HTTP listener. In the current codebase that
listener is configured by `[admin_web]`; the name is legacy/monitoring-oriented,
but `/terminal` is caller access and must not become a sysop portal.

| Route | Method | Purpose |
|-------|--------|---------|
| `/terminal` | `GET` | Full-tab terminal page. |
| `/terminal/` | `GET` | Redirect or serve the same page. |
| `/terminal/ws` | `GET` WebSocket upgrade | Raw caller byte stream. |

The current monitoring routes (`/`, `/health`, `/status`, `/api/...`) remain
unchanged. `/terminal` is a caller terminal, not a sysop portal. When the shared
HTTP listener is running, `/terminal` is mounted by default unless
`[web_terminal].enabled = false`.

If a sysop wants public browser access, keep OxideBBS bound to loopback and put
Caddy, nginx, or another reverse proxy in front. The reverse proxy may expose
`/terminal` on port `8080` or on HTTPS. OxideBBS itself does not implement TLS
for this HTTP listener.

---

## Config

Add a minimal config block:

```toml
[web_terminal]
enabled = true
```

Validation:

- `web_terminal.enabled` defaults to `true`.
- If `[admin_web].enabled = false`, no HTTP listener runs and no web terminal is
  exposed, even though `web_terminal.enabled` defaults to `true`.
- If `[admin_web].enabled = true` and `web_terminal.enabled = true`, mount
  `/terminal` and `/terminal/ws`.
- If `[admin_web].enabled = true` and `web_terminal.enabled = false`, keep
  monitoring routes active but do not mount `/terminal` or `/terminal/ws`.
- Reuse the HTTP listener bind, TLS/reverse-proxy policy, request logging, and
  origin validation already used by the monitoring surface.
- Do not add `bind`, `static_dir`, `ws_path`, `idle_timeout_seconds`, or a
  separate file-transfer config in the first pass.

This makes browser terminal access opt-out for boards that already enable the
HTTP listener, while still allowing sysops to keep monitoring enabled and turn
off caller terminal access.

---

## Full-Tab Terminal UX

`GET /terminal` must render only the terminal experience:

- black full-viewport background
- no cards, nav, sidebar, header, footer, docs text, or status dashboard
- terminal container fixed to the full browser viewport
- keyboard focus moves into the terminal after connect
- disconnected and connection-error messages render inside the terminal
- browser resize refits xterm.js, but does not change BBS-side terminal size in
  the MVP

Minimal CSS shape:

```css
html,
body,
#terminal {
    width: 100%;
    height: 100%;
    margin: 0;
    overflow: hidden;
    background: #000;
}

#terminal {
    position: fixed;
    inset: 0;
}

.xterm {
    width: 100%;
    height: 100%;
}
```

MVP frontend dependencies:

- `@xterm/xterm`
- `@xterm/addon-fit`

Do not add `zmodem.js`, web links, custom fonts, themes, settings panels, or
file-transfer UI until the raw terminal route is verified.

---

## Backend Work

### 1. Add Config

Add `WebTerminalConfig` with only `enabled: bool`, defaulting to `true`, and add
it to `OxideConfig`.

### 2. Mount Routes

Update the existing HTTP router so it mounts `/terminal` and `/terminal/ws`
unless `web_terminal.enabled = false`.

This likely means the existing HTTP listener startup receives the caller
resources it needs to create a web caller:

- config
- db
- login menu
- main menu
- menu map
- runtime

Keep this wiring narrow. Do not duplicate the telnet session setup logic in the
web module.

### 3. Add `WsTransport`

Create `crates/oxidebbs-server/src/web_terminal.rs`.

`WsTransport` wraps an axum WebSocket and implements
`oxidebbs_telnet::Transport`:

- `read_byte()` returns one byte at a time from incoming binary messages
- `write_all()` sends one WebSocket binary message
- close/error maps to `TransportError::Closed` or `Ok(None)` as appropriate
- text messages are ignored for MVP
- ping/pong uses the WebSocket stack defaults unless a test proves manual
  handling is required

Implementation note: enable axum's `ws` feature in workspace dependencies and
add `futures-util` only if the final implementation needs `StreamExt`/`SinkExt`
directly.

### 4. Reuse The Existing Caller Loop

Expose one narrow helper from `serve.rs`, such as:

```rust
pub(crate) async fn handle_raw_caller_transport<T: Transport>(
    allocation: NodeAllocation,
    transport: T,
    transport_name: &'static str,
    peer: CallerPeer,
    resources: CallerResources,
) -> ServeResult<()>
```

The helper should call the existing session loop with `telnet_protocol = false`.
That path already uses raw input, skips telnet negotiation, and uses
`TransportAdapter::new_raw()` for file transfers.

For v1, use the existing telnet idle timeout and configured default terminal
profile. Do not add per-web idle timeout or dynamic terminal-size negotiation.

### 5. WebSocket Upgrade

The WebSocket handler should:

- only be mounted when `web_terminal.enabled = true`
- enforce same-origin / configured allowed-origin checks before upgrade
- allocate a node with `runtime.try_allocate_node()`
- send `System busy. Try again later.` and close when no node is available
- derive `CallerPeer` from the socket address, with proxy headers only if the
  existing HTTP listener already trusts the reverse proxy
- call the raw caller helper with transport name `"websocket"`

No pre-authentication is added. The caller authenticates at the BBS login prompt.

---

## Frontend Work

Use the existing root Node project rather than creating `web/package.json`.
Add a small source directory:

```text
web-terminal/
├── index.html
├── src/
│   ├── main.ts
│   └── styles.css
└── tests/
    └── terminal.test.ts
```

Build output should be served by the Rust HTTP listener at `/terminal`. The plan
should choose one packaging path during implementation and keep it fixed:

- embed the built terminal HTML/CSS/JS into `oxidebbs-server`, or
- serve a fixed build directory controlled by the release package

Do not add a configurable `static_dir` until there is a real operator need.

Minimal frontend behavior:

```typescript
const terminal = new Terminal({ convertEol: false, cursorBlink: true });
const fitAddon = new FitAddon();
terminal.loadAddon(fitAddon);
terminal.open(document.getElementById('terminal')!);
fitAddon.fit();
terminal.focus();

const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
const socket = new WebSocket(`${protocol}//${window.location.host}/terminal/ws`);
socket.binaryType = 'arraybuffer';

terminal.onData((data) => {
    if (socket.readyState === WebSocket.OPEN) {
        socket.send(new TextEncoder().encode(data));
    }
});

socket.onmessage = (event) => {
    const bytes = event.data instanceof ArrayBuffer
        ? new Uint8Array(event.data)
        : new TextEncoder().encode(event.data);
    terminal.write(bytes);
};
```

---

## ZMODEM Phase

Add ZMODEM only after the raw terminal is working and tested.

The ZMODEM phase should:

- add `zmodem.js`
- route incoming WebSocket bytes through `Zmodem.Sentry`
- download files through browser `Blob` URLs
- upload files through a browser file picker
- keep server-side transfer code unchanged except for any bugs found in the raw
  WebSocket transport path

XMODEM-CRC remains telnet/serial only for now. Browser support starts with
ZMODEM because filenames and file metadata are carried by the protocol.

---

## Tests

Rust tests:

| Test | What it verifies |
|------|------------------|
| `web_terminal_default_enabled_mounts_routes` | `/terminal` is available when the shared HTTP listener is enabled and `[web_terminal]` is omitted. |
| `web_terminal_disabled_404s_or_hides_routes` | `/terminal` is unavailable when `web_terminal.enabled = false`. |
| `web_terminal_page_is_full_tab_shell` | HTML includes the terminal root and full-viewport asset references. |
| `web_terminal_rejects_bad_origin` | WebSocket upgrade rejects disallowed origins. |
| `web_terminal_ws_allocates_node` | A WebSocket caller creates a normal caller session with transport `"websocket"`. |
| `web_terminal_ws_rejects_when_nodes_busy` | Busy boards receive a short message and close. |
| `ws_transport_reads_binary_frames_bytewise` | Multi-byte binary frames are read as bytes in order. |
| `ws_transport_writes_binary_frames` | Server output is sent as binary frames. |

Frontend tests:

| Test | What it verifies |
|------|------------------|
| `terminal fills viewport` | The terminal container uses the full browser viewport. |
| `terminal connects_to_terminal_ws` | The frontend opens `/terminal/ws`. |
| `terminal_writes_incoming_bytes` | Incoming bytes are passed to xterm.js. |
| `terminal_sends_keyboard_bytes` | xterm input sends bytes on the WebSocket. |

ZMODEM tests are added with the ZMODEM phase, not the raw terminal phase.

---

## Documentation Updates

Update:

- `config/oxidebbs.example.toml`
- `docs/project/remote-admin.md` or a new `docs/project/web-terminal.md`
- `README.md` feature list once implemented
- `design/SPEC.md`
- `design/TASKS.md`

Docs must state:

- `/terminal` is caller access, not a sysop/admin portal
- the page intentionally fills the entire browser tab
- browser callers authenticate at the normal BBS login prompt
- OxideBBS does not serve HTTPS/TLS on this HTTP listener
- use a reverse proxy such as Caddy for public HTTPS deployments

Create an ADR only if implementation introduces a lasting architecture decision
not already covered by this plan. If an ADR is added, use the next available ADR
number.

---

## Implementation Order

1. Add `[web_terminal] enabled = true` config and example TOML.
2. Mount default-enabled `/terminal` and `/terminal/ws` routes on the existing
   HTTP listener, with `web_terminal.enabled = false` disabling only those
   terminal routes.
3. Add `WsTransport`.
4. Add one narrow raw-caller helper in `serve.rs`.
5. Wire WebSocket callers into the existing session loop with
   `telnet_protocol = false`.
6. Add the minimal full-tab xterm.js frontend.
7. Add Rust tests for config, routes, Origin checks, node allocation, and
   transport byte behavior.
8. Add frontend tests for full-tab layout and WebSocket byte flow.
9. Update docs.
10. Add ZMODEM browser handling as a follow-up phase on the same byte stream.

---

## Explicitly Out Of Scope For The Raw Terminal Phase

- separate web-terminal listener or second web port
- configurable terminal route or WebSocket path
- configurable static directory
- pre-auth, JWT, OAuth, or cookies for browser callers
- sysop/admin controls in the browser
- ZMODEM UI before raw terminal login/menu/logoff is verified
- XMODEM-CRC browser support
- dynamic terminal resize signaling into the BBS session
- reconnect/session resume
- custom fonts or theme picker
- TLS inside OxideBBS
- multiple browser terminal profiles
