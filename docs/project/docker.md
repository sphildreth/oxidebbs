# Docker Deployment

Docker is the supported cross-platform way to run OxideBBS on Windows, macOS,
and Linux while keeping the runtime target Linux.

The image includes `oxidebbs-server`, the bundled Oxide-owned `OXIDECHK.EXE`
test door fixture, and DOSEMU2. Host installs of DOSEMU2, Free Pascal, or the
Free Pascal `i8086-msdos` toolchain are not required for normal Docker use.

## What Docker Runs

The container starts in:

```text
/srv/oxidebbs
```

Persistent state is stored in Docker named volumes:

```text
/srv/oxidebbs/config
/srv/oxidebbs/data
/srv/oxidebbs/doors
/srv/oxidebbs/logs
/srv/oxidebbs/runtime
```

Do not bind-mount `data/` or `runtime/` from Windows or macOS for normal use.
DecentDB files, Unix control sockets, DOSEMU2 PTY paths, and DOS door runtime
files should live on Docker-managed Linux filesystems.

## First Boot

Pull the published image and start:

```bash
docker compose pull
OXIDEBBS_SYSOP_PASSWORD='choose-a-real-password' docker compose up -d
```

The default Compose file uses:

```text
ghcr.io/sphildreth/oxidebbs:1.2.2
```

To pin or test another published image tag:

```bash
OXIDEBBS_IMAGE_TAG=1.2.2 docker compose pull
OXIDEBBS_IMAGE_TAG=1.2.2 \
  OXIDEBBS_SYSOP_PASSWORD='choose-a-real-password' \
  docker compose up -d
```

The first boot creates:

- `config/oxidebbs.toml`
- `data/oxidebbs.ddb`
- the initial sysop account
- runtime/log/door directories
- the enabled bundled `oxide-check` door definition

The default sysop alias is `sysop`. Override first-boot values with:

```bash
OXIDEBBS_BOARD_NAME='My BBS' \
OXIDEBBS_SYSOP_ALIAS='cmdrtallen' \
OXIDEBBS_SYSOP_PASSWORD='choose-a-real-password' \
OXIDEBBS_NODES=4 \
docker compose up -d
```

The Compose file provides a local-evaluation default password so the stack can
start without extra files, but the container prints a warning when that default
is used. Change `OXIDEBBS_SYSOP_PASSWORD` before exposing the telnet port.

After `config/oxidebbs.toml` exists, changing these environment variables does
not rewrite the board. Use sysop CLI commands or remove/recreate the config and
data volumes for a clean first boot.

## Connect

The Compose file publishes the telnet listener on host port `2323` by default:

```bash
telnet localhost 2323
```

or connect with SyncTERM to:

```text
localhost:2323
```

OxideBBS generated config binds telnet to `127.0.0.1:2323` by default outside
Docker. The Docker entrypoint explicitly sets
`OXIDEBBS_TELNET_BIND=0.0.0.0:2323` so Compose host-port publishing works. That
is a plaintext deployment choice: telnet sends credentials and caller traffic
without encryption.

To change the host port:

```bash
OXIDEBBS_HOST_TELNET_PORT=2424 docker compose up -d
```

The container still listens on port `2323`; Compose remaps the host port.

## Sysop Commands

Run one-shot CLI commands through Compose:

```bash
docker compose run --rm oxidebbs status
docker compose run --rm oxidebbs nodes list
docker compose run --rm oxidebbs doors check oxide-check
docker compose run --rm oxidebbs doors test oxide-check --user sysop --dry-run
```

When the server container is already running, commands can also execute inside
it:

```bash
docker compose exec oxidebbs oxidebbs-server --config /srv/oxidebbs/config/oxidebbs.toml status
docker compose exec oxidebbs oxidebbs-server --config /srv/oxidebbs/config/oxidebbs.toml nodes list
```

## Doors In Docker

The Docker image includes DOSEMU2 and the checked-in `OXIDECHK.EXE` fixture.
The bundled test door is enabled by default in Docker first-boot setup so sysops
can test a known-good DOS door path.

The live byte path is unchanged from native Linux:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOSEMU2-emulated COM1 UART
  <-> DOS door program
```

Run the optional DOSEMU2 smoke test inside the image:

```bash
docker compose run --rm oxidebbs /bin/bash -lc \
  'cd /srv/oxidebbs && OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh'
```

For a multi-node fixture check:

```bash
docker compose run --rm oxidebbs /bin/bash -lc \
  'cd /srv/oxidebbs && OXIDE_DOOR_MULTI_NODE=1 OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh'
```

Third-party doors should be copied into Docker-managed Linux storage or baked
into a derivative image. Avoid Windows/macOS bind mounts for live door working
directories unless you are deliberately testing host-filesystem behavior.

## Volumes And Reset

Stop without deleting state:

```bash
docker compose down
```

Reset all Docker-managed OxideBBS state:

```bash
docker compose down -v
```

That deletes the generated config, DecentDB database, logs, runtime files, and
doors volume. The next `docker compose up` performs first-boot setup again.

Inspect volumes:

```bash
docker volume ls | grep oxidebbs
```

## Published Image Tags

Release publication pushes Docker images to GitHub Container Registry after the
image has built and passed the `oxidebbs-server --version` smoke test.

Preferred stable tags:

```text
ghcr.io/sphildreth/oxidebbs:1.2.2
ghcr.io/sphildreth/oxidebbs:v1.2.2
```

Pull a published image directly:

```bash
docker pull ghcr.io/sphildreth/oxidebbs:1.2.2
docker run --rm --entrypoint oxidebbs-server ghcr.io/sphildreth/oxidebbs:1.2.2 --version
```

Stable release publishes also move:

```text
ghcr.io/sphildreth/oxidebbs:latest
```

Prefer the exact version tag for production deployments. Use `latest` only when
you intentionally want to track the newest published stable image.

## Source Builds

Build locally from a source checkout:

```bash
docker compose -f compose.yaml -f compose.build.yaml up -d --build
```

Use a clean rebuild when validating dependency or DOSEMU2 package changes:

```bash
docker compose -f compose.yaml -f compose.build.yaml build --no-cache
```

## Windows And macOS Notes

Use Docker Desktop and connect to `localhost:<published-port>`.

The host does not need:

- Rust
- DecentDB native build headers
- DOSEMU2
- Free Pascal
- an i8086/MS-DOS cross compiler

Those are either inside the image or not part of normal runtime. The project is
still a Linux-targeted BBS engine; Docker Desktop supplies the Linux environment.
