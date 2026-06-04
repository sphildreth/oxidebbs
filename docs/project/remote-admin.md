# Remote Admin

Remote admin is disabled by default. OxideBBS models and validates the
security-sensitive `[admin_web]` configuration and can start a read-only
loopback HTTP status endpoint when explicitly enabled.

The public HTTP surface is intentionally narrow: `GET /status` is available only
when both `enabled = true` and `public_status_enabled = true`. The payload omits
database paths, caller addresses, secrets, and audit rows. Authenticated browser
or API admin views are not implemented yet, and remote mutations remain blocked.

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
- `read_only = false` is rejected until authenticated remote mutations, CSRF,
  replay protection, and audit coverage are implemented.

The supported admin surfaces remain the local sysop CLI, local Unix control
socket, local Ratatui sysop TUI, and the optional read-only `/status` endpoint.
