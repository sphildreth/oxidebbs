# OxideBBS Runbook

## Local development startup

```bash
cargo run -p oxidebbs-server -- serve
```

The CLI uses `config/oxidebbs.toml` when it exists, otherwise it falls back to
`config/oxidebbs.example.toml`. Pass `--config <path>` to force a file.

## Health checks

Planned commands:

```bash
oxidebbs-server check
oxidebbs-server --config config/oxidebbs.example.toml check
oxidebbs-server doors test lord --user sysop
```

## Logs

Use structured logging.

Recommended env during development:

```bash
RUST_LOG=oxidebbs=debug,tower=info
```

## Backups

DecentDB backup strategy is TBD. Do not assume SQLite/PostgreSQL tooling.

## Troubleshooting checklist

- Config file exists
- Data directory is writable
- Runtime directory is writable
- Telnet bind address is available
- ANSI assets are readable
- Door working directory exists
- DOSBox/DOSEMU command exists
- Node runtime directories are writable
