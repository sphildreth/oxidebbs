# Oxide Door Package Format v1 (`.oxdoor`)

See the canonical specification in
[`design/OXDOOR_FORMAT_V1.md`](https://github.com/sphildreth/oxidebbs/blob/main/design/OXDOOR_FORMAT_V1.md).

In short:

- `oxide-door.toml` + `checksums.sha256` are required.
- `package.format` must be `oxide-door-package-v1`.
- `package.kind` must be `full` for now.
- Supported payload roots are `files/` (required for full packages), `docs/`, and
  `tests/` (optional).
- Supported drop files are `DOOR.SYS`, `DORINFO1.DEF`, `CHAIN.TXT`,
  `DOORFILE.SR`, `PCBOARD.SYS`, `CALLINFO.BBS`.
- Imports are declarative only. OxideBBS does not run package-provided
  `postinstall.sh`, `install.bat`, keygens, downloaded scripts, or shell hooks.
- Imported doors default to disabled unless the sysop passes `--enable`.

Inspect packages with:

```bash
oxidebbs-server doors package inspect path/to/package.oxdoor
oxidebbs-server doors package import path/to/package.oxdoor --dry-run
oxidebbs-server doors package import path/to/package.oxdoor
oxidebbs-server doors package import path/to/package.oxdoor --replace
```

`inspect` validates metadata, checksums, and archive path safety without
installing files or writing DecentDB. `import --dry-run` prints the planned file
copies and door definition changes without writing files, creating a database, or
enabling the door. Real import copies only `files/` payloads under the configured
door root and creates or updates the door definition; existing target directories
or door definitions require `--replace`.

OxideBBS does not bundle third-party copyrighted, shareware, or abandonware DOS
doors. Operators are responsible for using door binaries they have rights to run.
