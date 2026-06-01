# OxideBBS Runbook

## Local development startup

```bash
cargo run -p oxidebbs-server -- serve
```

The CLI uses `config/oxidebbs.toml` when it exists, otherwise it falls back to
`config/oxidebbs.example.toml`. Pass `--config <path>` to force a file.

Running `serve` starts a local control listener on
`runtime/oxidebbs-control.sock` for sysop operations like:

- `oxidebbs-server status`
- `oxidebbs-server nodes list`
- `oxidebbs-server nodes show`
- `oxidebbs-server nodes disconnect`
- `oxidebbs-server nodes message`
- `oxidebbs-server nodes broadcast`

The socket is local-only. On Unix, startup removes a stale
`runtime/oxidebbs-control.sock` only when no process is listening on it. If
another server is still accepting connections on that socket, `serve` reports a
startup error instead of falling back to offline control behavior.

## Health checks

Useful commands:

```bash
oxidebbs-server check
oxidebbs-server status
oxidebbs-server db doctor
oxidebbs-server --config config/oxidebbs.example.toml check
oxidebbs-server doors check example
oxidebbs-server doors test example --user sysop --dry-run
```

## Logs

Use structured logging.

Recommended env during development:

```bash
RUST_LOG=oxidebbs=debug,tower=info
```

## Backups

Use the DecentDB-aware sysop command boundary:

```bash
oxidebbs-server db backup backups/oxidebbs.ddb
oxidebbs-server db export --format json > backups/oxidebbs.json
```

JSON import and compaction remain disabled until restore and compaction
semantics are specified for DecentDB.

## Troubleshooting checklist

- Config file exists
- Data directory is writable
- Runtime directory is writable
- Telnet bind address is available
- ANSI assets are readable
- Door working directory exists
- DOSBox/DOSEMU command exists
- Node runtime directories are writable
- Control socket directory exists/writable (`runtime/` by default), and stale
  `runtime/oxidebbs-control.sock` files are removed automatically on the next
  startup when no server is listening on them
