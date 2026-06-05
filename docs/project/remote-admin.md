# Remote Monitoring

Remote monitoring is disabled by default. OxideBBS validates the
security-sensitive `[admin_web]` configuration and can start a loopback HTTP
status surface when explicitly enabled. The configuration section retains the
`[admin_web]` name for compatibility, but this is not a browser sysop portal.

The public HTTP surface is intentionally narrow: `GET /status` is available only
when both `enabled = true` and `public_status_enabled = true`. The payload omits
database paths, caller addresses, secrets, and audit rows.

## HTTP And TLS Boundary

OxideBBS does not implement HTTPS or TLS for `[admin_web]`. The listener speaks
plain HTTP only, even when `require_tls = true`. That setting is an operator
policy flag for proxied deployments; it does not make OxideBBS accept native TLS
connections.

Keep `bind = "127.0.0.1:8080"` and place OxideBBS behind a local reverse proxy
when HTTPS is needed. The reverse proxy, such as Caddy, nginx, or Traefik,
terminates TLS and forwards plain HTTP to the loopback OxideBBS listener.
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
- `allowed_origins` entries must be exact `https://` or loopback-only
  `http://` origins without paths, whitespace, userinfo, or `*`.
- enabled remote monitoring must bind to a loopback address. OxideBBS does not
  serve native HTTPS/TLS on this listener.
- `behind_reverse_proxy = true` requires a loopback bind and `require_tls = true`
  so HTTPS is terminated by a local reverse proxy before plain HTTP is forwarded
  to OxideBBS.
- `read_only = false` is rejected; remote mutations are not enabled in v1.2.

The real sysop surfaces remain the local CLI, local Unix control socket, and
local Ratatui sysop TUI. Remote mutations are intentionally unavailable.
