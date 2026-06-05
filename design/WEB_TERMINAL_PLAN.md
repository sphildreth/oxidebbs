# Web Terminal Implementation Plan for OxideBBS

**Document**: `design/WEB_TERMINAL_PLAN.md`
**Status**: DRAFT - For implementation
**Target**: Browser-based ANSI/CP437 terminal with ZMODEM file transfers

---

## Overview

This document outlines the implementation plan for adding a web-based terminal
to OxideBBS. The terminal will provide a full-featured ANSI/CP437 experience with
ZMODEM file transfers, allowing users to connect via browser without the need for native telnet clients.


## Architecture

The web terminal is a **byte-level proxy** — the browser speaks ZMODEM directly
to the BBS server via WebSocket binary frames, just as a telnet client speaks
ZMODEM over its TCP connection. No custom file transfer protocol is needed.

```
Browser (xterm.js + zmodem.js)  ←WebSocket binary→  WsTransport  ←→  handle_caller_transport()
                                       ↑                                    ↑
                                  raw bytes                      existing session loop
                                  (no protocol                   (login, menus, ZMODEM
                                   awareness)                    via oxidebbs-transfer)
```

Key decisions:

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Where is the WS code? | Module in `oxidebbs-server` | Server already has axum, `CallerResources`, session loop. A separate crate creates coupling issues and duplicates state. |
| Transport integration | `WsTransport` impls `oxidebbs_telnet::Transport` | Feeds directly into `handle_caller_transport()` — same code path as telnet callers |
| Binary adapter | Reuse `TransportAdapter::new_raw()` | No telnet IAC escaping needed for WebSocket (binary frames carry raw bytes) |
| ZMODEM in browser | `zmodem.js` npm package | Industry standard (used by ttyd, electerm). Browser handles ZMODEM framing via `Zmodem.Sentry` |
| ZMODEM on server | Existing `oxidebbs-transfer` code in session loop | Server initiates/accepts transfers same as telnet callers. `WsTransport` is just the pipe |
| Auth | Open WebSocket (like telnet) | Same trust model as telnet: anyone who can connect gets a login prompt. Authentication happens at the BBS login screen, not the WebSocket endpoint. No JWT, no tokens, no pre-auth — ever. |
| Static files | Same axum router, same port | No CORS issues, no separate HTTP server |

---

## Phase Map

| Phase | Status | Description |
|-------|--------|-------------|
| **Phase 1** | TODO | `WsTransport` + config + axum route in `oxidebbs-server` |
| **Phase 2** | TODO | Frontend: xterm.js + zmodem.js + Vite |
| **Phase 3** | TODO | Wire web callers into `handle_caller_transport()` |
| **Phase 4** | TODO | ZMODEM file transfer integration |
| **Phase 5** | TODO | Tests, config example, documentation |

---

## Phase 1: Rust — WsTransport + Config + Axum Route

### 1.1 Config

Add to `crates/oxidebbs-server/src/config.rs`:

```rust
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WebTerminalConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_web_terminal_bind")]
    pub bind: String,
    #[serde(default = "default_web_terminal_ws_path")]
    pub ws_path: String,
    #[serde(default = "default_web_terminal_static_dir")]
    pub static_dir: PathBuf,
    #[serde(default = "default_web_terminal_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_web_terminal_idle_timeout_seconds")]
    pub idle_timeout_seconds: u64,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}
```

Add to `OxideConfig`:

```rust
#[serde(default)]
pub web_terminal: WebTerminalConfig,
```

Defaults: `bind = "127.0.0.1:8421"`, `ws_path = "/ws"`, `static_dir = "./web/dist"`,
`max_connections = 4`, `idle_timeout_seconds = 900`.

Validation: if `enabled`, `bind` must parse as `SocketAddr`, `max_connections > 0`,
`idle_timeout_seconds > 0`, `allowed_origins` must be valid origins or empty
(empty = same-origin only, same as existing `admin_web` validation pattern).

### 1.2 WsTransport

New file: `crates/oxidebbs-server/src/web_terminal.rs`

`WsTransport` wraps an axum `WebSocket` (after `split()`) and implements
`oxidebbs_telnet::Transport`. The pattern follows `TcpTransport`: own both
halves, use an internal read buffer, flush on write.

```rust
use std::collections::VecDeque;
use axum::extract::ws::{Message, WebSocket};
use oxidebbs_telnet::{Transport, TransportError};
use tokio::sync::mpsc;
use futures_util::{SinkExt, StreamExt};

pub(crate) struct WsTransport {
    write_half: axum::extract::ws::SplitSink<axum::extract::ws::WebSocket>,
    read_half: axum::extract::ws::SplitStream<axum::extract::ws::WebSocket>,
    read_buffer: VecDeque<u8>,
    closed: bool,
}

impl WsTransport {
    pub(crate) fn new(socket: WebSocket) -> Self {
        let (write_half, read_half) = socket.split();
        Self {
            write_half,
            read_half,
            read_buffer: VecDeque::new(),
            closed: false,
        }
    }
}

impl Transport for WsTransport {
    async fn read_byte(&mut self) -> Result<Option<u8>, TransportError> {
        loop {
            if let Some(byte) = self.read_buffer.pop_front() {
                return Ok(Some(byte));
            }
            if self.closed {
                return Ok(None);
            }
            match self.read_half.next().await {
                Some(Ok(Message::Binary(data))) => {
                    self.read_buffer.extend(&data);
                    continue;
                }
                Some(Ok(Message::Text(_))) => continue,
                Some(Ok(Message::Ping(data))) => {
                    // Pong is handled automatically by axum
                    let _ = self.write_half.send(Message::Pong(data)).await;
                    continue;
                }
                Some(Ok(Message::Pong(_))) => continue,
                Some(Ok(Message::Close(_))) | None => {
                    self.closed = true;
                    return Ok(None);
                }
                Some(Err(_)) => {
                    self.closed = true;
                    return Ok(None);
                }
            }
        }
    }

    async fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if self.closed {
            return Err(TransportError::Closed);
        }
        self.write_half
            .send(Message::Binary(bytes.to_vec().into()))
            .await
            .map_err(|_| TransportError::Closed)
    }

    async fn hangup(&mut self) -> Result<(), TransportError> {
        let _ = self.write_half.send(Message::Close(None)).await;
        self.closed = true;
        Ok(())
    }
}
```

**Dependency note**: `axum::extract::ws` already uses `tokio-tungstenite` internally.
No need to add `tokio-tungstenite` or `tungstenite` as workspace deps. `futures-util`
is needed for `StreamExt`/`SinkExt` and is already a transitive dep via axum; add it
explicitly to `oxidebbs-server/Cargo.toml`.

### 1.3 Axum WebSocket Handler

In the same `web_terminal.rs`:

```rust
pub(crate) async fn ws_handler(ws: WebSocketUpgrade) -> axum::response::Response {
    ws.on_upgrade(handle_ws_caller)
}

async fn handle_ws_caller(socket: WebSocket) {
    // Obtain CallerResources, allocate node, call handle_caller_transport
    // (see Phase 3 for full integration)
}
```

The handler is added to the existing `admin_web` router (or a sibling router
on the same port). Static files are served from the same router as a fallback.

### 1.4 Start Web Terminal Listener

In `serve.rs`, alongside the telnet listener and admin web server, start the
web terminal if enabled:

```rust
let web_terminal_handle = if config.web_terminal.enabled {
    Some(crate::web_terminal::start_web_terminal(
        Arc::clone(&shared_config),
        Arc::clone(&db),
        Arc::clone(&login_menu),
        Arc::clone(&main_menu),
        Arc::clone(&menus),
        Arc::clone(&runtime),
    )?)
} else {
    None
};
```

The function returns a `tokio::task::JoinHandle<()>` that's aborted on shutdown,
same pattern as `admin_web_handle` and `binkp_listener_handle`.

---

## Phase 2: Frontend — xterm.js + zmodem.js + Vite

### 2.1 Directory Structure

```
web/
├── package.json
├── tsconfig.json
├── vite.config.ts
├── index.html
├── src/
│   └── main.ts
├── public/
│   └── fonts/
│       └── Perfect_DOS_VGA_437_Win.ttf
└── tests/
    └── terminal.test.ts
```

Minimal: one source file, one font, one test file.

### 2.2 package.json

```json
{
  "name": "oxidebbs-web-terminal",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "test": "vitest run"
  },
  "dependencies": {
    "@xterm/xterm": "^5.5.0",
    "@xterm/addon-fit": "^0.10.0",
    "@xterm/addon-web-links": "^0.11.0",
    "zmodem.js": "^0.1.10"
  },
  "devDependencies": {
    "typescript": "^5.5.0",
    "vite": "^5.4.0",
    "vitest": "^2.0.0"
  }
}
```

No `@xterm/addon-attach` — we do NOT attach the terminal directly to the WebSocket.
All data must flow through `Zmodem.Sentry` so file transfers are intercepted.

### 2.3 main.ts — Core Logic

```typescript
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import * as Zmodem from 'zmodem.js';
import '@xterm/xterm/css/xterm.css';
import './styles.css';

class OxideBBSTerminal {
    private term: Terminal;
    private fitAddon: FitAddon;
    private ws: WebSocket | null = null;
    private zmodemSentry: Zmodem.Sentry | null = null;
    private zmodemSession: any = null;
    private inZmodem = false;

    constructor() {
        this.term = new Terminal({
            fontFamily: '"Perfect DOS VGA 437 Win", monospace',
            fontSize: 16,
            lineHeight: 1.2,
            cursorBlink: true,
            cursorStyle: 'block',
            scrollback: 10000,
            allowTransparency: false,
            convertEol: false,
            theme: {
                background: '#000000',
                foreground: '#BBBBBB',
                cursor: '#FFFFFF',
                selectionBackground: '#FFFFFF40',
                black: '#000000',
                red: '#AA0000',
                green: '#00AA00',
                yellow: '#AA5500',
                blue: '#0000AA',
                magenta: '#AA00AA',
                cyan: '#00AAAA',
                white: '#AAAAAA',
                brightBlack: '#555555',
                brightRed: '#FF5555',
                brightGreen: '#55FF55',
                brightYellow: '#FFFF55',
                brightBlue: '#5555FF',
                brightMagenta: '#FF55FF',
                brightCyan: '#55FFFF',
                brightWhite: '#FFFFFF',
            },
        });

        this.fitAddon = new FitAddon();
        this.term.loadAddon(this.fitAddon);
        this.term.loadAddon(new WebLinksAddon());

        this.term.onData((data: string) => {
            if (this.inZmodem && this.zmodemSession) {
                // During ZMODEM, keystrokes go to the session, not the terminal
                // (e.g., Ctrl+C to cancel)
                return;
            }
            if (this.ws?.readyState === WebSocket.OPEN) {
                this.ws.send(new TextEncoder().encode(data));
            }
        });

        window.addEventListener('resize', () => this.fitAddon.fit());
    }

    connect(url: string): void {
        this.ws = new WebSocket(url);
        this.ws.binaryType = 'arraybuffer';

        this.ws.onopen = () => {
            this.term.open(document.getElementById('terminal')!);
            this.fitAddon.fit();
            this.term.focus();
            this.setupZmodem();
        };

        this.ws.onmessage = (event: MessageEvent) => {
            const data = event.data instanceof ArrayBuffer
                ? new Uint8Array(event.data)
                : new TextEncoder().encode(event.data);
            if (this.inZmodem && this.zmodemSentry) {
                this.zmodemSentry.consume(data);
            } else if (this.zmodemSentry) {
                this.zmodemSentry.consume(data);
            } else {
                this.term.write(data);
            }
        };

        this.ws.onclose = () => {
            this.term.writeln('\r\n\x1b[1;31mDisconnected.\x1b[0m');
        };

        this.ws.onerror = () => {
            this.term.writeln('\r\n\x1b[1;31mConnection error.\x1b[0m');
        };
    }

    private setupZmodem(): void {
        this.zmodemSentry = new Zmodem.Sentry({
            to_terminal: (octets: number[]) => {
                if (!this.inZmodem) {
                    this.term.write(new Uint8Array(octets));
                }
            },
            sender: (octets: number[]) => {
                if (this.ws?.readyState === WebSocket.OPEN) {
                    this.ws.send(new Uint8Array(octets));
                }
            },
            on_detect: (detection: any) => {
                this.inZmodem = true;
                this.term.options.disableStdin = true;
                const session = detection.confirm();
                this.zmodemSession = session;

                session.on('receive', () => {
                    // Handle received file data
                });

                session.on('session_end', () => {
                    this.inZmodem = false;
                    this.zmodemSession = null;
                    this.term.options.disableStdin = false;
                    this.term.focus();
                });

                // If this is an offer (download from BBS)
                const offer = session.get_offer();
                if (offer) {
                    this.handleDownload(offer, session);
                } else {
                    // Upload: show file picker
                    this.handleUpload(session);
                }
            },
            on_retract: () => {
                this.inZmodem = false;
                this.zmodemSession = null;
                this.term.options.disableStdin = false;
            },
        });
    }

    private handleDownload(offer: any, session: any): void {
        // Auto-accept all offers from the BBS
        offer.accept().then(() => {
            return session.end();
        }).catch(() => {
            session.abort();
        });

        session.on('receive', (xfer: any) => {
            const bytes = new Uint8Array(xfer.get_buffer());
            const blob = new Blob([bytes]);
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = xfer.get_details().name;
            a.click();
            URL.revokeObjectURL(url);
        });
    }

    private handleUpload(session: any): void {
        const input = document.createElement('input');
        input.type = 'file';
        input.onchange = () => {
            const file = input.files?.[0];
            if (!file) {
                session.abort();
                return;
            }
            file.arrayBuffer().then((buffer) => {
                const send = session.send({
                    name: file.name,
                    size: file.size,
                    mode: 0o100644,
                    mtime: new Date(file.lastModified),
                    remaining: file.size,
                });
                send.on('input', (ctx: any) => {
                    const chunk = new Uint8Array(buffer, ctx.offset, ctx.length);
                    return chunk;
                });
                session.end();
            });
        };
        input.click();
    }
}

document.addEventListener('DOMContentLoaded', () => {
    const app = new OxideBBSTerminal();
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const url = `${protocol}//${window.location.host}/ws`;
    app.connect(url);
});
```

### 2.4 CSS — styles.css

```css
* { margin: 0; padding: 0; box-sizing: border-box; }
html, body { height: 100%; width: 100%; overflow: hidden; background: #000; color: #bbb; }
#terminal { position: fixed; top: 0; left: 0; right: 0; bottom: 0; width: 100%; height: 100%; }
.xterm { height: 100% !important; width: 100% !important; }
.xterm .xterm-viewport { overflow: hidden !important; }
@font-face {
    font-family: 'Perfect DOS VGA 437 Win';
    src: url('/fonts/Perfect_DOS_VGA_437_Win.ttf') format('truetype');
    font-display: swap;
    unicode-range: U+0000-00FF, U+2500-257F, U+2580-259F, U+25A0-25FF;
}
.xterm .xterm-rows { font-variant-ligatures: none; }
```

### 2.5 index.html

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
    <title>OxideBBS</title>
</head>
<body>
    <div id="terminal"></div>
    <script type="module" src="/src/main.ts"></script>
</body>
</html>
```

### 2.6 vite.config.ts

```typescript
import { defineConfig } from 'vite';

export default defineConfig({
    root: '.',
    publicDir: 'public',
    build: {
        outDir: '../crates/oxidebbs-server/web-dist',
        emptyOutDir: true,
        minify: 'esbuild',
        target: 'es2022',
    },
    server: {
        port: 3000,
        proxy: {
            '/ws': { target: 'ws://127.0.0.1:8421', ws: true },
        },
    },
});
```

### 2.7 Font

Download `Perfect_DOS_VGA_437_Win.ttf` from
https://github.com/viler-int10h/dos VGA-textmode-mac/releases and place in
`web/public/fonts/`. This font provides authentic CP437 box-drawing and block
characters.

No fallback font needed — xterm.js will use `monospace` as the CSS fallback
when CP437 chars are unavailable, which is acceptable for basic ASCII content.

---

## Phase 3: Wire Web Callers into Session Loop

### 3.1 The Integration Point

The existing session loop is `handle_caller_transport()` in `serve.rs`:
signature: `handle_caller_transport<T: Transport>(allocation, transport,
transport_name, telnet_protocol, peer, resources) -> ServeResult<()>`.

For web callers:
- `transport` = `WsTransport::new(socket)`
- `transport_name` = `"websocket"`
- `telnet_protocol` = `false` (no IAC negotiation — browser speaks raw bytes)
- `peer` = `CallerPeer` derived from the HTTP request's remote address and
  `X-Forwarded-For` / `X-Real-Ip` headers

The `telnet_protocol = false` path already exists in `handle_caller_transport`:
it uses `InputSession::raw()` and skips NAWS/TTYPE negotiation. Terminal
capabilities are set to the configured default profile (e.g., `ansi80`).

### 3.2 Node Allocation and Dispatch

In `web_terminal.rs`:

```rust
async fn handle_ws_caller(
    socket: WebSocket,
    config: Arc<OxideConfig>,
    db: Arc<OxideDb>,
    login_menu: Arc<Menu>,
    main_menu: Arc<Menu>,
    menus: Arc<HashMap<String, Arc<Menu>>>,
    runtime: Arc<ServerRuntime>,
    peer: CallerPeer,
) {
    let Some(allocation) = runtime.try_allocate_node() else {
        // No node available — send rejection and close
        let mut transport = WsTransport::new(socket);
        let _ = transport.write_all(b"System busy. Try again later.\r\n").await;
        let _ = transport.hangup().await;
        return;
    };

    let resources = CallerResources {
        db,
        config,
        login_menu,
        main_menu,
        menus,
        runtime,
    };

    let transport = WsTransport::new(socket);
    if let Err(error) = handle_caller_transport(
        allocation,
        transport,
        "websocket",
        false,  // no telnet protocol
        peer,
        resources,
    ).await {
        warn!("web caller session ended with error: {error}");
    }
}
```

### 3.3 Idle Timeout

The existing `idle_timeout_seconds` from `[telnet]` config is used by
`handle_caller_transport` via `next_event()`. Web callers should use the
`web_terminal.idle_timeout_seconds` value instead. This requires passing the
timeout value through or having `handle_caller_transport` accept it as a
parameter rather than reading from `config.telnet`.

For v1: use the telnet idle timeout for all callers. A follow-up can make it
per-transport-type.

### 3.4 Terminal Resize

Web callers' initial terminal dimensions come from the configured default
profile (e.g., 80x25 for `ansi80`). For v1, no dynamic resize signaling from
the browser. The BBS session uses the static dimensions.

Future enhancement: the browser can send terminal resize events via a simple
protocol (e.g., JSON text frames on the WebSocket alongside binary data frames).
This is deferred because it requires changes to the session loop's
`InputSession` to handle resize mid-session.

---

## Phase 4: ZMODEM File Transfer Integration

### 4.1 How It Works

ZMODEM transfers flow through the same `Transport` trait as all other I/O:

1. **Download** (BBS → browser): User selects "Download" in BBS menu.
   The session loop runs `send_zmodem_file()` from `oxidebbs-transfer`, which
   writes ZMODEM protocol bytes to the `Transport`. The browser's `Zmodem.Sentry`
   detects the `**B0...` ZRQINIT sequence, enters ZMODEM mode, and offers the
   file as a browser download.

2. **Upload** (browser → BBS): User selects "Upload" in BBS menu. The session
   loop enters `receive_zmodem_file()` which sends ZRINIT bytes. The browser's
   `Zmodem.Sentry` detects this, shows a file picker, and sends the file via
   ZMODEM protocol.

3. **During transfer**: Both sides send raw binary frames. The `WsTransport`
   passes these through unchanged. The `Zmodem.Sentry` on the browser side
   intercepts ZMODEM data and routes it to the download/upload handler instead
   of `terminal.write()`. When the transfer completes, normal terminal rendering
   resumes.

### 4.2 No Telnet IAC Escaping for Web Callers

For telnet callers, file transfers use `TransportAdapter::new_telnet()` which
doubles `0xFF` bytes to escape them through the telnet IAC protocol. For web
callers, WebSocket binary frames carry raw bytes with no escaping needed. Use
`TransportAdapter::new_raw()` instead.

The existing session code in `serve.rs` constructs a `TransportAdapter` when
entering a file transfer. The `telnet_protocol: bool` flag (already passed to
`handle_caller_transport`) determines which mode to use:
- `telnet_protocol = true` → `TransportAdapter::new_telnet(transport)`
- `telnet_protocol = false` → `TransportAdapter::new_raw(transport)`

This is the only change needed in the existing session transfer code.

### 4.3 Browser-Side ZMODEM Details

The `zmodem.js` library's `Zmodem.Sentry` sits between the WebSocket and
`terminal.write()`:

```
WebSocket → Zmodem.Sentry.consume(bytes)
                       ├── Non-ZMODEM bytes → terminal.write()
                       └── ZMODEM detected → Zmodem.Session
                                                   ├── Download → Blob + <a> click
                                                   └── Upload → <input type=file>
```

During an active ZMODEM session:
- `terminal.options.disableStdin = true` — keystrokes go to ZMODEM, not terminal
- `Zmodem.Sentry` consumes ALL incoming bytes and routes them appropriately
- On `session_end`, `disableStdin` is reset and normal operation resumes

### 4.4 XMODEM-CRC

XMODEM-CRC is **not supported** for web callers in v1. Reasons:

- XMODEM requires the user to type a filename on the BBS command line, which
  doesn't map well to a browser UX
- ZMODEM handles filename metadata in-band and supports batch transfers
- The existing ZMODEM implementation in `oxidebbs-transfer` works for both
  telnet and web callers with no protocol changes

Telnet callers retain XMODEM-CRC support unchanged.

---

## Phase 5: Tests, Config, Docs

### 5.1 Rust Tests

| Test | Location | What it verifies |
|------|----------|-----------------|
| `ws_transport_loopback` | `oxidebbs-server/src/web_terminal.rs` (test module) | Write bytes to WsTransport, read them back; close detection |
| `ws_transport_large_frame` | same | Send a frame larger than 1 byte, verify byte-by-byte read |
| `ws_transport_concurrent` | same | Multiple read/write cycles interleaved |
| `ws_config_validation` | `oxidebbs-server/src/config.rs` (existing tests) | WebTerminalConfig validation (bind parse, origins) |

### 5.2 Frontend Tests

| Test | Location | What it verifies |
|------|----------|------------------|
| `terminal opens in container` | `web/tests/terminal.test.ts` | xterm.js initializes and renders |
| `fitAddon resizes` | same | cols/rows > 0 after fit() |
| `Zmodem.Sentry detects ZRQINIT` | same | Zmodem.Sentry triggers on_detect for `**B0...` |

### 5.3 Config Example Addition

Add to `config/oxidebbs.example.toml`:

```toml
[web_terminal]
enabled = false
bind = "127.0.0.1:8421"
ws_path = "/ws"
static_dir = "./web/dist"
max_connections = 4
idle_timeout_seconds = 900
allowed_origins = []
```

`allowed_origins` empty = same-origin only (most secure default). No authentication is applied to the WebSocket endpoint — users authenticate via the BBS login prompt after connecting, exactly like telnet.

### 5.4 Architectural Decision Record

Create `design/adr/001-web-terminal.md`:

- Decision: Module in `oxidebbs-server` (not a separate crate)
- Rationale: Avoids circular deps, reuses `CallerResources` and session loop directly
- Consequence: `oxidebbs-server` gains axum WS dependency but already has axum

### 5.5 CI Addition

Add a Node.js build/test step to CI for the `web/` directory:

```yaml
- name: Build and test web terminal
  run: |
    cd web
    npm ci
    npm run build
    npm run test
```

### 5.6 Documentation

Create user-facing doc explaining:
- How to enable `[web_terminal]` in config
- How to build the frontend (`cd web && npm ci && npm run build`)
- Browser requirements (modern browser with WebSocket support)
- ZMODEM download/upload workflow in the browser

---

## Implementation Order for Coding Agents

1. **Add `WebTerminalConfig`** to `config.rs` with validation and defaults
2. **Add `[web_terminal]`** to `oxidebbs.example.toml`
3. **Create `web_terminal.rs`** module in `oxidebbs-server` with `WsTransport` + axum handler
4. **Wire into `serve.rs`**: start web terminal listener, pass `CallerResources`
5. **Adjust `handle_caller_transport`** to accept a per-transport idle timeout and use `TransportAdapter::new_raw()` when `telnet_protocol == false`
6. **Create `web/`** directory with `package.json`, `vite.config.ts`, `index.html`, `src/main.ts`, `src/styles.css`
7. **Add `zmodem.js` and `@xterm/xterm`** npm deps, implement `OxideBBSTerminal` class
8. **Add font** to `web/public/fonts/`
9. **Write Rust tests** for `WsTransport`
10. **Write frontend tests** for terminal init and Zmodem.Sentry detection
11. **Add `futures-util`** to `oxidebbs-server` Cargo.toml (for `StreamExt`/`SinkExt` on WebSocket)
12. **Update CI** to build and test `web/`

---

## Directory Structure After Implementation

```
oxidebbs/
├── config/
│   └── oxidebbs.example.toml           # Updated: add [web_terminal]
├── crates/
│   └── oxidebbs-server/
│       ├── Cargo.toml                   # Updated: add futures-util
│       └── src/
│           ├── main.rs                  # Unchanged
│           ├── config.rs               # Updated: add WebTerminalConfig
│           ├── serve.rs                # Updated: start web terminal listener
│           └── web_terminal.rs          # NEW: WsTransport + axum WS handler
├── web/                                 # NEW: Frontend
│   ├── package.json
│   ├── tsconfig.json
│   ├── vite.config.ts
│   ├── index.html
│   ├── src/
│   │   ├── main.ts
│   │   └── styles.css
│   ├── public/
│   │   └── fonts/
│   │       └── Perfect_DOS_VGA_437_Win.ttf
│   └── tests/
│       └── terminal.test.ts
└── design/
    ├── WEB_TERMINAL_PLAN.md            # THIS FILE
    └── adr/
        └── 001-web-terminal.md          # NEW: ADR
```

**Files changed**: 5 existing files, 8 new files. **No new crates.**

---

## Phase Status Tracking

| Phase | Status | Started | Completed | Notes |
|-------|--------|---------|-----------|-------|
| 1: WsTransport + config + route | TODO | | | |
| 2: Frontend (xterm + zmodem.js) | TODO | | | |
| 3: Session integration | TODO | | | |
| 4: ZMODEM transfers | TODO | | | |
| 5: Tests, config, docs | TODO | | | |

---

## What Is NOT In Scope

- **WebSocket authentication**: The WebSocket endpoint is open, identical to telnet. Users authenticate via the BBS login prompt once connected. No JWT, no tokens, no pre-authentication — authentication is the BBS's job, not the transport's.
- **XMODEM-CRC in browser**: ZMODEM only. XMODEM still works for telnet callers.
- **Terminal resize signaling**: Static 80x25 from the configured profile.
- **Reconnection/session resume**: If the WebSocket drops, the session reconnects as a new caller.
- **TLS termination**: Use a reverse proxy (nginx, caddy) in front. The default bind is `127.0.0.1:8421`.
- **Multiple web terminal profiles**: Uses the configured `terminal.default_profile` (typically `ansi80`).