# OxideBBS Runbook

## Local development startup

```bash
cargo run -p oxidebbs-server -- serve
```

The CLI uses `config/oxidebbs.toml` when it exists, otherwise it falls back to
`config/oxidebbs.example.toml`. Pass `--config <path>` to force a file.

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
