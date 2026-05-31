# OxideBBS Runbook

## Local development startup

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml
```

## Health checks

Planned commands:

```bash
oxidebbs-server check --config config/oxidebbs.example.toml
oxidebbs-server db doctor --config config/oxidebbs.example.toml
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
