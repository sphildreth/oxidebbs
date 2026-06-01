# Deployment and Operations

OxideBBS runs as a local binary (`oxidebbs-server`) plus a local CLI. Remote
admin access is intentionally not supported in this release.

## Runtime deployment workflow

1. Generate config and scaffold:

```bash
cargo run -p oxidebbs-server -- --data /srv/oxidebbs/data/oxidebbs.ddb setup \
  --output /etc/oxidebbs/oxidebbs.toml \
  --board-name "My BBS" \
  --sysop-alias sysop \
  --sysop-password "change-this"
```

2. Validate before first boot:

```bash
cargo run -p oxidebbs-server -- --config /etc/oxidebbs/oxidebbs.toml check
```

3. Start serving:

```bash
cargo run -p oxidebbs-server -- --config /etc/oxidebbs/oxidebbs.toml serve
```

4. Install DOSBox on hosts that will launch DOS doors:

```bash
sudo apt-get install -y dosbox
```

5. Validate and dry-run the bundled test door:

```bash
cargo run -p oxidebbs-server -- --config /etc/oxidebbs/oxidebbs.toml doors check oxide-check
cargo run -p oxidebbs-server -- --config /etc/oxidebbs/oxidebbs.toml doors dropfile oxide-check --user sysop --node 1 --format DORINFO1.DEF
cargo run -p oxidebbs-server -- --config /etc/oxidebbs/oxidebbs.toml doors test oxide-check --user sysop --dry-run
```

6. To run a live smoke test:

1. Keep DOSBox installed.
2. Enable `oxide-check` in the active config (it is intentionally disabled by
   default in `oxidebbs.example.toml`).
3. Start the server and launch `oxide-check` from the caller `Doors` menu.

7. Verify health:

```bash
cargo run -p oxidebbs-server -- --config /etc/oxidebbs/oxidebbs.toml status
cargo run -p oxidebbs-server -- --config /etc/oxidebbs/oxidebbs.toml nodes list
```

Live launch requires DOSBox and will fail with a clear external-runner error if
missing.

## Native build prerequisites

When compiling in fresh Debian/Ubuntu environments:

```bash
sudo apt-get install -y clang libclang-dev
```

## Local control socket in deployment

`serve` starts a Unix control socket at:

```text
<runtime path>/oxidebbs-control.sock
```

This socket enables:

- live `status` and `nodes` queries
- live node messaging/disconnect/broadcast/reset-stale
- stale node detection visibility

If the socket path already exists and is active, startup fails instead of silently
falling back to offline behavior.

If no process is listening, startup removes a stale socket file automatically.

For unexpected runtime-path permission or ownership issues, recover with:

```bash
systemctl stop oxidebbs
rm -f /srv/oxidebbs/runtime/oxidebbs-control.sock
install -d -o oxidebbs -g oxidebbs /srv/oxidebbs/runtime
systemctl start oxidebbs
```

## Stale node and operations checks

- `nodes list` shows stale states from heartbeat age and marks stale nodes as
  `stale`.
- `nodes reset-stale` asks the live runtime to disconnect stale nodes through the
  local control socket.
- `status` reports uptime from the live listener when available; otherwise marks
  it unavailable.

## Service layout examples

A systemd service should call the built binary with explicit config and writable
paths:

```ini
[Unit]
Description=OxideBBS telnet server
After=network-online.target

[Service]
Type=simple
User=oxidebbs
Group=oxidebbs
WorkingDirectory=/srv/oxidebbs
Environment=RUST_LOG=oxidebbs=info
ExecStart=/usr/local/bin/oxidebbs-server --config /etc/oxidebbs/oxidebbs.toml serve
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

## Documentation site deployment

The documentation site is independent of the runtime. Build and publish with VitePress
using existing GitHub workflow configuration:

```bash
npm ci
npm run docs:build
npm run docs:dev
```

The built output is `docs/.vitepress/dist` and is published by
`.github/workflows/pages.yml`.
