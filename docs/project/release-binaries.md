# Release Binaries

OxideBBS publishes packaged server binaries for sysops who want to run directly
on a host without compiling Rust.

Use Docker instead if you want the easiest cross-platform door setup with
DOSEMU2 already inside the runtime image.

## Download

For each release, the release workflow builds these archives:

| Platform | Archive |
| --- | --- |
| Linux x86_64 GNU | `oxidebbs-<version>-linux-x86_64-gnu.tar.gz` |
| macOS x86_64 | `oxidebbs-<version>-macos-x86_64.tar.gz` |
| Windows x86_64 MSVC | `oxidebbs-<version>-windows-x86_64-msvc.zip` |

Each archive has a matching `.sha256` checksum file on the same GitHub release.

## What Is Included

Release archives include:

- `oxidebbs-server` or `oxidebbs-server.exe`
- `README.md`, `LICENSE`, `NOTICE`, and `SECURITY.md`
- default `assets/` and `config/`
- an empty `doors/` directory
- the Oxide-owned `oxide-check` test door fixture
- door helper source under `tools/doors/`

Third-party DOS doors are not bundled.

## Verify The Download

Linux:

```bash
sha256sum -c oxidebbs-<version>-linux-x86_64-gnu.tar.gz.sha256
```

macOS:

```bash
shasum -a 256 -c oxidebbs-<version>-macos-x86_64.tar.gz.sha256
```

Windows PowerShell:

```powershell
Get-FileHash .\oxidebbs-<version>-windows-x86_64-msvc.zip -Algorithm SHA256
Get-Content .\oxidebbs-<version>-windows-x86_64-msvc.zip.sha256
```

Compare the hash values before extracting.

## Extract And Run

Linux or macOS:

```bash
tar -xzf oxidebbs-<version>-linux-x86_64-gnu.tar.gz
cd oxidebbs-<version>-linux-x86_64-gnu
./oxidebbs-server --version
./oxidebbs-server setup
./oxidebbs-server --config config/oxidebbs.toml check
./oxidebbs-server --config config/oxidebbs.toml serve
```

Windows PowerShell:

```powershell
Expand-Archive .\oxidebbs-<version>-windows-x86_64-msvc.zip
cd .\oxidebbs-<version>-windows-x86_64-msvc\oxidebbs-<version>-windows-x86_64-msvc
.\oxidebbs-server.exe --version
.\oxidebbs-server.exe setup
.\oxidebbs-server.exe --config config\oxidebbs.toml check
.\oxidebbs-server.exe --config config\oxidebbs.toml serve
```

## Host Requirements

The release binary does not require Rust, Cargo, Node.js, or libclang at runtime.
Those are build-time tools only.

For live DOS doors on a native Linux host, install DOSEMU2 separately. Docker is
still the simplest path when DOSEMU2 setup is the main concern.

For browser terminal and LAN monitoring, enable `[admin_web]` in `oxidebbs.toml`
and see [Remote Monitoring](./remote-admin.md).

## Suggested Service Layout

A typical Linux install uses separate config, data, runtime, log, and door paths:

```text
/etc/oxidebbs/oxidebbs.toml
/srv/oxidebbs/data/
/srv/oxidebbs/runtime/
/srv/oxidebbs/logs/
/srv/oxidebbs/doors/
```

Use [Deployment and Operations](./deployment.md) for a systemd service example.
