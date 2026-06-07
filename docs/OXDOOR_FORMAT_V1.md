# Oxide Door Package Format v1 (`.oxdoor`)

See the canonical specification in
[`design/OXDOOR_FORMAT_V1.md`](../design/OXDOOR_FORMAT_V1.md).

In short:

- `oxide-door.toml` + `checksums.sha256` are required.
- `package.format` must be `oxide-door-package-v1`.
- `package.kind` must be `full` for now.
- Supported payload roots are `files/` (required for full packages), `docs/`, and
  `artifacts/` (optional).
- Supported drop files are `DOOR.SYS`, `DORINFO1.DEF`, `CHAIN.TXT`,
  `DOORFILE.SR`, `PCBOARD.SYS`, `CALLINFO.BBS`.

Inspect packages with:

```bash
oxidebbs-server doors package inspect path/to/package.oxdoor
```
