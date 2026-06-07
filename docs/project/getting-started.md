# Getting Started

OxideBBS is a BBS server for sysops who want ANSI/CP437 callers, telnet and
browser-terminal access, DOS doors, file areas, local sysop tools, and FTN or
OxideNet networking.

Choose the install path that matches how you want to operate the board:

| Path | Best for | Complexity |
| --- | --- | --- |
| [Docker](./docker.md) | Fastest cross-platform setup, including DOSEMU2 for doors. | Easiest |
| [Release binaries](./release-binaries.md) | Running directly on Linux, macOS, or Windows without compiling Rust. | Moderate |
| [Build from source](#build-from-source) | Development, custom patches, or unsupported targets. | Most involved |

Most new sysops should start with Docker. Use release binaries when you want a
normal service install on a host you manage. Build from source only when you are
working on OxideBBS itself or need to patch it.

## Option 1: Docker

Docker keeps DecentDB, door runtime files, and DOSEMU2 on a Linux filesystem even
when the host is Windows or macOS.

```bash
docker compose pull
OXIDEBBS_SYSOP_PASSWORD='choose-a-real-password' docker compose up -d
```

Connect with SyncTERM or another telnet client:

```text
localhost:2323
```

Run sysop commands through Compose:

```bash
docker compose run --rm oxidebbs status
docker compose run --rm oxidebbs nodes list
docker compose run --rm oxidebbs doors check oxide-check
```

See [Docker Deployment](./docker.md) for volume layout, reset steps, and door
notes.

## Option 2: Release binaries

Download the archive for your platform from the GitHub release page:

```text
oxidebbs-<version>-linux-x86_64-gnu.tar.gz
oxidebbs-<version>-macos-x86_64.tar.gz
oxidebbs-<version>-windows-x86_64-msvc.zip
```

Each archive has a matching `.sha256` file. Extract the archive, then run the
binary from the extracted directory:

```bash
./oxidebbs-server --version
./oxidebbs-server setup
./oxidebbs-server --config config/oxidebbs.toml check
./oxidebbs-server --config config/oxidebbs.toml serve
```

The release archive includes the server binary, default assets, example config,
and the Oxide-owned `oxide-check` test door fixture. Linux hosts that launch DOS
doors still need DOSEMU2 installed on the host unless you use Docker.

See [Release Binaries](./release-binaries.md) for install and service notes.

## Option 3: Build from source

Use this path when you are developing OxideBBS or need local patches.

Prerequisites:

```bash
sudo apt-get install -y clang libclang-dev
```

Build and run from the repository:

```bash
cargo build --release --locked -p oxidebbs-server --bin oxidebbs-server
./target/release/oxidebbs-server setup
./target/release/oxidebbs-server --config config/oxidebbs.toml check
./target/release/oxidebbs-server --config config/oxidebbs.toml serve
```

If you are changing OxideBBS source code, run the contributor quality gate before
opening a pull request:

```bash
./scripts/dev-check.sh
```

## First board setup

The setup wizard creates a board config, DecentDB database, initial sysop
account, directories, default ANSI/screen assets, and the starter message area:

```bash
oxidebbs-server setup
```

For unattended setup:

```bash
oxidebbs-server setup \
  --board-name "My BBS" \
  --sysop-alias sysop \
  --sysop-password "change-this" \
  --nodes 4
```

The generated config is usually:

```text
config/oxidebbs.toml
```

Validate it before first boot:

```bash
oxidebbs-server --config config/oxidebbs.toml check
oxidebbs-server --config config/oxidebbs.toml config check
```

## Start the BBS

```bash
oxidebbs-server --config config/oxidebbs.toml serve
```

The server accepts caller sessions, writes session/audit rows, and starts the
local control socket at:

```text
runtime/oxidebbs-control.sock
```

Use local sysop commands while the server is running:

```bash
oxidebbs-server --config config/oxidebbs.toml status
oxidebbs-server --config config/oxidebbs.toml nodes list
oxidebbs-server --config config/oxidebbs.toml nodes watch
```

## Enable browser terminal and LAN monitoring

When `[admin_web].enabled = true`, OxideBBS can serve `/terminal`, `/health`,
and `/status` from the same HTTP listener. Direct LAN HTTP is allowed; WAN or
public HTTPS should be handled by a reverse proxy.

See [Remote Monitoring](./remote-admin.md) for safe bind examples.

## Doors

OxideBBS includes an Oxide-owned `oxide-check` test door fixture for validating
the door path. Validate it without launching DOSEMU2:

```bash
oxidebbs-server --config config/oxidebbs.toml doors check oxide-check
oxidebbs-server --config config/oxidebbs.toml doors test oxide-check --user sysop --dry-run
```

Live DOS doors use this byte path:

```text
caller client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOS door program
```

Linux hosts running live DOS doors need DOSEMU2. Fedora sysops should use the
[DOSEMU2 on Fedora](./dosemu2-fedora.md) guide.

## File areas

Enable `[file_transfers]`, create a file area, then import files:

```bash
oxidebbs-server --config config/oxidebbs.toml files areas add main \
  --name "Main Files" \
  --root files/main \
  --read-level 0 \
  --download-level 0 \
  --upload-level 20

oxidebbs-server --config config/oxidebbs.toml files import main ./uploads/demo.zip \
  --description "Demo archive"
```

Callers can download with ZMODEM or XMODEM from the configured `files` menu.
Uploads are stored pending sysop review.

## Backups

```bash
oxidebbs-server --config config/oxidebbs.toml db backup backups/oxidebbs.ddb
oxidebbs-server --config config/oxidebbs.toml db export --format json > backups/oxidebbs.json
oxidebbs-server --config config/oxidebbs.toml db compact --output backups/oxidebbs-compacted.ddb
```

`db import --format json` is a full restore into a schema-only target. Stop the
server before manually replacing the active database file.

## Where to go next

- [Setup Wizard](./setup.md) explains generated config and starter assets.
- [Docker Deployment](./docker.md) covers Docker volumes and door runtime notes.
- [Release Binaries](./release-binaries.md) covers packaged installs.
- [Menus](./menus.md) explains caller menus and help screens.
- [Doors](./doors.md) covers local and remote door setup.
- [File Transfers](./file-transfers.md) covers ZMODEM and XMODEM behavior.
- [FTN Architecture](../ftn/architecture.md) covers echomail and netmail.
- [OxideNet](../oxidenet/overview.md) covers first-party OxideNet workflows.
