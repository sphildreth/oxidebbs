# Remote Admin

Remote admin is disabled by default. OxideBBS currently models and validates the
security-sensitive `[admin_web]` configuration, but it does not start a remote
HTTP admin surface yet.

Default configuration:

```toml
[admin_web]
enabled = false
bind = "127.0.0.1:8080"
require_tls = true
read_only = true
session_timeout_seconds = 900
csrf_token_ttl_seconds = 900
replay_window_seconds = 300
rate_limit_per_minute = 30
```

Validation rules:

- `bind` must be an IP socket address.
- timeout, CSRF, replay-window, and rate-limit values must be greater than zero.
- non-loopback binds require `require_tls = true`.
- `read_only = false` is rejected until authenticated remote mutations, CSRF,
  replay protection, and audit coverage are implemented.

The supported admin surfaces remain the local sysop CLI, local Unix control
socket, and local Ratatui sysop TUI.
