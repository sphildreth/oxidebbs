# Deployment and Operations

OxideBBS runs as one server binary (`oxidebbs-server`) plus local sysop
commands. The normal sysop install choices are:

| Path | Use when |
| --- | --- |
| [Docker](./docker.md) | You want the easiest cross-platform setup and bundled DOSEMU2 runtime. |
| [Release binaries](./release-binaries.md) | You want to run directly on a Linux, macOS, or Windows host without compiling. |
| Build from source | You are developing OxideBBS or need local patches. |

Remote monitoring and `/terminal` can be enabled for LAN use. Public/WAN HTTPS
should be handled by a reverse proxy. Remote mutation APIs remain disabled.

## Runtime deployment workflow

1. Generate config and scaffold:

```bash
oxidebbs-server --data /srv/oxidebbs/data/oxidebbs.ddb setup \
  --output /etc/oxidebbs/oxidebbs.toml \
  --board-name "My BBS" \
  --sysop-alias sysop \
  --sysop-password "change-this"
```

2. Validate before first boot:

```bash
oxidebbs-server --config /etc/oxidebbs/oxidebbs.toml check
```

3. Start serving:

```bash
oxidebbs-server --config /etc/oxidebbs/oxidebbs.toml serve
```

4. Install your distribution's DOSEMU2 package on hosts that will launch DOS
   doors. The executable is commonly named `dosemu`, but legacy `dosemu-1.x` is
   not supported because it does not accept OxideBBS's run-local
   `pts <path>` COM1 mapping:

```bash
dosemu --version
```

Fedora hosts should follow [DOSEMU2 on Fedora](/project/dosemu2-fedora). The
validated Fedora setup uses the `stsp/dosemu2` Copr packages and requires
`libdj64.so.0` to be visible through `ldconfig`; otherwise DOSEMU2 can fail while
booting `COMMAND.COM`.

`DOSEMU2` in the v1 path is headless and does not need SDL or a display server.

In Debian 13 Proxmox LXC deployments:

- ensure `/dev/pts` is mounted and writable in the container,
- verify DOSEMU2 sees a PTY path by checking `/dev/pts`,
- prefer unprivileged LXC when policy allows it, because DOSEMU2 is not tied to
  privileged-only settings for this model.

```bash
test -d /dev/pts && ls -ld /dev/pts
```

5. Validate and dry-run the bundled test door:

```bash
oxidebbs-server --config /etc/oxidebbs/oxidebbs.toml doors check oxide-check
oxidebbs-server --config /etc/oxidebbs/oxidebbs.toml doors dropfile oxide-check --user sysop --node 1 --format DORINFO1.DEF
oxidebbs-server --config /etc/oxidebbs/oxidebbs.toml doors test oxide-check --user sysop --dry-run
```

6. To run a live smoke test, keep DOSEMU2 installed, enable `oxide-check` in the
active config, start the server, and launch `oxide-check` from the caller `Doors`
menu.

The deployed byte path is:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOSEMU2-emulated COM1 UART
  <-> DOS door program
```

Live launch should produce per-node runtime data and report files under
`runtime/node-001/`:

- `OXDOSEMU2.CONF` is generated with `$_com1 = "pts .../node-001/OXCOM1.PTY"`
- `OXCOM1.PTY` appears while the door is active
- bridge ownership remains with OxideBBS, not by a serial daemon

7. Verify health:

```bash
oxidebbs-server --config /etc/oxidebbs/oxidebbs.toml status
oxidebbs-server --config /etc/oxidebbs/oxidebbs.toml nodes list
```

Live launch fails clearly when:

- `dosemu` is missing,
- the per-node PTY path never appears,
- the node runtime directory is not writable,
- the caller disconnects before the door exits,
- or the runtime timeout triggers and closes the child.

## Source build prerequisites

Release binaries do not require Rust, Cargo, Node.js, or libclang. When building
from source in fresh Debian/Ubuntu environments:

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

On Unix, the runtime directory is mode `0700`, the socket is mode `0600`, and
incoming control clients are rejected unless their peer UID matches the server
process UID.

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
- `status` reports uptime from the live listener when available; otherwise marks it
  unavailable.
- Live `status --json` includes `audit_write_failures`, the count of best-effort
  audit writes that failed during the current server run.

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

Keep the configured telnet bind on `127.0.0.1:2323` unless you are deliberately
exposing a plaintext telnet service. Public binds send credentials and caller
traffic without encryption.

Before public exposure, run a connection limit smoke test with
`max_connections + 1` callers. The extra caller should receive the busy message
while accepted nodes remain visible and stable through `nodes list`.

## Documentation site deployment

The documentation site is independent of the runtime. Build and publish with
VitePress using existing GitHub workflow configuration:

```bash
npm ci
npm run docs:build
npm run docs:dev
```

The built output is `docs/.vitepress/dist` and is published by
`.github/workflows/pages.yml`.
