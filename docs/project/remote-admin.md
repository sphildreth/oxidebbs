# Remote Admin

Remote admin is disabled by default. OxideBBS validates the security-sensitive
`[admin_web]` configuration and can start a loopback HTTP admin surface when
explicitly enabled.

The public HTTP surface is intentionally narrow: `GET /status` is available only
when both `enabled = true` and `public_status_enabled = true`. The payload omits
database paths, caller addresses, secrets, and audit rows.

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
- enabled remote admin must bind to a loopback address until native remote-admin
  TLS support exists.
- `behind_reverse_proxy = true` requires a loopback bind and `require_tls = true`
  so TLS termination stays inside a local proxy boundary.
- `read_only = false` is rejected; remote mutations are not enabled in v1.2.

The mutating admin surfaces remain the local sysop CLI, local Unix control
socket, and local Ratatui sysop TUI.
