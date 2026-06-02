# ADR 0012: Use Docker For Cross-Platform Deployment

## Status

Accepted

## Context

OxideBBS v1 targets Linux. The runtime depends on Unix process handling,
Unix-domain local control sockets, DecentDB native bindings, PTY-backed door
bridging, and DOSEMU2 for DOS doors.

Native Windows and macOS builds would require separate door-runtime decisions,
separate filesystem and process semantics, and platform-specific support for
features that are naturally Linux-oriented. Supporting those native targets now
would increase the project surface area before the Linux v1 runtime is stable.

At the same time, potential sysops commonly use Windows and macOS workstations.
They need a practical way to run and evaluate OxideBBS without maintaining a
native Linux server from day one.

## Decision

Support Docker and Docker Compose as the cross-platform deployment and
evaluation path.

The Docker image remains a Linux runtime image. It includes:

- the built `oxidebbs-server` binary,
- bundled ANSI/screen/config assets,
- the checked-in `OXIDECHK.EXE` test door fixture,
- DOSEMU2 and the Fedora package set required to run DOS doors headlessly,
- the DJ64 loader-cache configuration required by the validated Fedora DOSEMU2
  setup.

Docker Desktop on Windows and macOS is supported as a way to run this Linux
target. It is not a commitment to native Windows or macOS binaries.

Compose uses Docker named volumes for:

- `/srv/oxidebbs/config`
- `/srv/oxidebbs/data`
- `/srv/oxidebbs/doors`
- `/srv/oxidebbs/logs`
- `/srv/oxidebbs/runtime`

The runtime, data, and door state should stay on Docker-managed Linux
filesystems. Host bind mounts from Windows or macOS are not the default because
runtime PTYs, Unix sockets, DecentDB storage, and DOS door files all depend on
Linux filesystem behavior.

The container entrypoint performs first-boot setup only when
`/srv/oxidebbs/config/oxidebbs.toml` is missing. It creates the starter config,
initial sysop account, DecentDB path, runtime paths, and can enable the bundled
`oxide-check` test door. Once the config exists, environment variables no longer
silently rewrite board state.

Normal Rust builds and tests remain independent of Docker, DOSEMU2, DOSBox,
Free Pascal, and the i8086/MS-DOS cross compiler.

## Consequences

- Windows and macOS users can run OxideBBS through Docker Desktop while the
  project maintains one Linux runtime target.
- The DOSEMU2 door path can be tested inside the same container image used for
  deployment.
- Runtime support documentation can focus on Docker plus native Linux instead of
  native Windows/macOS variants.
- The container image is intentionally larger than a minimal Rust-only image
  because it includes a DOS door runtime.
- First-boot sysop password handling must be documented clearly. The Compose
  default is only for local evaluation and must be changed before exposing a
  board.
- Sysops who need custom third-party doors should add them through Linux-native
  container storage or controlled image/volume workflows, not through ad hoc
  host-path mounts on non-Linux filesystems.

## References

- DOSEMU2 runtime ADR: `design/adr/0010-use-dosemu2-for-dos-door-runtime.md`
- DOSBox removal ADR: `design/adr/0011-remove-dosbox-runner-before-v1.md`
- Docker deployment docs: `docs/project/docker.md`
