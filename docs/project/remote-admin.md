# Remote Monitoring

Remote monitoring is disabled by default. OxideBBS validates the
security-sensitive `[admin_web]` configuration and can start an HTTP status and
browser-terminal surface when explicitly enabled. The listener may be bound to
loopback, private LAN, link-local, or all local interfaces for direct LAN use.
WAN/public exposure should be placed behind a TLS-terminating reverse proxy.

The public HTTP surface is intentionally narrow: `GET /status` is available only
when both `enabled = true` and `public_status_enabled = true`. The payload omits
database paths, caller addresses, secrets, and audit rows.

## HTTP And TLS Boundary

OxideBBS does not implement HTTPS or TLS for `[admin_web]`. The listener speaks
plain HTTP only, even when `require_tls = true`. That setting is an operator
policy flag for proxied deployments; it does not make OxideBBS accept native TLS
connections.

For LAN-only access, binding directly to a LAN address or all local interfaces
is allowed:

```toml
[admin_web]
enabled = true
bind = "0.0.0.0:8080"
public_status_enabled = true
read_only = true
```

`0.0.0.0` means all local IPv4 interfaces. On a LAN-only host this exposes
OxideBBS to the LAN; on a VPS or a machine with a public NIC it also exposes
the listener publicly. Use firewall rules or a reverse proxy when the host has
WAN reachability.

Keep `bind = "127.0.0.1:8080"` and place OxideBBS behind a local reverse proxy
when HTTPS or WAN/public access is needed. The reverse proxy, such as Caddy,
nginx, or Traefik, terminates TLS and forwards plain HTTP to the loopback
OxideBBS listener.
Pointing a browser or curl at `https://127.0.0.1:8080` talks TLS to an HTTP
socket and will fail.

Minimal Caddy example:

```caddyfile
monitor.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

Matching OxideBBS settings:

```toml
[admin_web]
enabled = true
bind = "127.0.0.1:8080"
require_tls = true
behind_reverse_proxy = true
public_status_enabled = true
read_only = true
```

## Browser Caller Terminal

When `[admin_web].enabled = true`, OxideBBS also serves a caller-facing terminal
surface on the same listener when `[web_terminal].enabled = true` (default):

- `GET /terminal` renders a full-browser terminal with no admin/dashboard chrome.
- `GET /terminal/ws` upgrades to the raw websocket byte stream used by caller
  sessions.
- `GET /terminal/zmodem.js` serves browser-side ZMODEM support.

Browser callers authenticate at the normal BBS login prompt. This is not a sysop
or administration portal.

Set `[web_terminal].enabled = false` to disable only `/terminal` and
`/terminal/ws` while leaving `/`, `/health`, `/status`, and API routes untouched.

This terminal milestone uses raw websocket binary input/output and reuses the same
caller session allocation, tracking, and transport record path as telnet callers.

ZMODEM transfers happen on the same websocket channel through the existing file
transfer engine.

`/terminal` is plain HTTP over the existing listener; OxideBBS does not serve
HTTPS/TLS itself. Use a local reverse proxy (for example Caddy, nginx, or
Traefik) for public HTTPS deployments.

`GET /` returns a small static landing page that confirms the listener is
running and points operators to the available JSON routes. The monitoring
listener speaks HTTP directly.

`GET /health` runs the same doctor checks used by the local sysop tooling and
returns JSON with `healthy = true` plus HTTP `200 OK` when doctor has no failed
checks. It returns `healthy = false` plus HTTP `503 Service Unavailable` when
doctor reports failures. `/healthz` and `/healtz` are aliases for monitoring
systems that expect those spellings.

Every monitoring web request is emitted through the configured OxideBBS logger
as an activity line with HTTP method, path without query string, response
status, elapsed milliseconds, remote address when available, and authenticated
user ID when the session is already logged in. Request bodies, query strings,
cookies, CSRF tokens, replay headers, and other header values are not logged.

Authenticated read endpoints are available after a sysop login:

- `GET /csrf-token` creates or refreshes a pre-auth session and CSRF token.
- `POST /login` verifies an active sysop user's Argon2 password and upgrades the
  session.
- `POST /logout` requires the session cookie and CSRF token, then deletes the
  session.
- `GET /api/status`, `/api/nodes`, `/api/users`, `/api/doors`, `/api/messages`,
  `/api/database`, `/api/logs`, `/api/audit`, `/api/network`, and
  `/api/oxidenet` return authenticated read-only JSON summaries.

Session cookies are `HttpOnly`, `Secure`, and `SameSite=Strict`. Browser
requests with an `Origin` header must match `allowed_origins` or the request
host. Remote mutation attempts remain blocked while `read_only = true`; guarded
mutation routes validate auth, CSRF, nonce/timestamp replay headers, rate
limits, and audit logging before refusing to mutate state.

Default configuration:

```toml
[admin_web]
enabled = false
public_status_enabled = false
bind = "127.0.0.1:8080"
require_tls = true
read_only = true
allowed_origins = []
behind_reverse_proxy = false
session_timeout_seconds = 900
csrf_token_ttl_seconds = 900
replay_window_seconds = 300
rate_limit_per_minute = 30
```

Validation rules:

- `bind` must be an IP socket address.
- timeout, CSRF, replay-window, and rate-limit values must be greater than zero.
- `allowed_origins` entries must be exact `https://` origins or direct
  loopback/private-LAN/link-local `http://` origins without paths, whitespace,
  userinfo, or `*`.
- enabled remote monitoring may bind to loopback, private LAN, link-local, or
  unspecified addresses. OxideBBS does not serve native HTTPS/TLS on this
  listener.
- `behind_reverse_proxy = true` requires a loopback bind and `require_tls = true`
  so HTTPS is terminated by a local reverse proxy before plain HTTP is forwarded
  to OxideBBS.
- `read_only = false` is rejected; remote mutations are not supported.

The real sysop surfaces remain the local CLI, local Unix control socket, and
local Ratatui sysop TUI. Remote mutations are intentionally unavailable.
