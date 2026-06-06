# Oxide Test Door Implementation Plan

This document defines the implementation plan for a bundled OxideBBS-owned DOS
test door. The goal is to give developers and sysops a known-good door program
that exercises the same DOSEMU2 path that real v1 doors will use, without
redistributing copyrighted or abandonware software.

The plan is intentionally prescriptive. Coding agents implementing this work
should follow the decisions in this document and should not substitute a native
Rust helper, shell script, batch file, external freeware door, or different
runtime contract unless this document is updated first.

## Phase Map

Status values:

- `TODO`: not started on the current branch.
- `IN PROGRESS`: actively being implemented on the current branch.
- `COMPLETE`: implemented, documented, and validated according to this
  document's Definition of Done.

| Phase | Status | Goal | Required Output |
| --- | --- | --- | --- |
| Phase 0 - Planning Baseline | COMPLETE | Capture the product and engineering decisions for the test door. | This document. |
| Phase 1 - Repository Layout And License Boundary | COMPLETE | Add a clear, source-owned home for the DOS test door and prevent licensing ambiguity. | `tools/doors/oxide-door-check/` layout, `SHA256SUMS`, maintainer rebuild scripts. |
| Phase 2 - DOS Door Program | COMPLETE | Implement the actual DOS test program. | Free Pascal source, checked-in `OXIDECHK.EXE` fixture, checksum, deterministic behavior. |
| Phase 3 - DOSEMU2 Runtime Contract | COMPLETE | Make OxideBBS launch DOS doors with the drop file visible inside DOSEMU2. | Updated `oxidebbs-door` plan generation and validation tests. |
| Phase 4 - Config And Sysop CLI Integration | COMPLETE | Make the test door easy to configure and exercise from existing commands. | Example config, setup guidance, `doors check/test/dropfile` compatibility. |
| Phase 5 - Testing Automation | COMPLETE | Cover the fixture without making CI depend on DOSEMU2. | Rust unit/integration tests plus an optional DOSEMU2 smoke script. |
| Phase 6 - Documentation And Changelog | COMPLETE | Document sysop usage and record the user-visible behavior change. | Updated design docs, operator docs, task list, and changelog. |
| Phase 7 - Final Validation | COMPLETE | Prove the branch is ready to merge. | `./scripts/dev-check.sh`, docs build, whitespace check, optional DOSEMU2 smoke result. |

## Definition Of Done

The Oxide test door is complete only when all of the following are true:

1. A project-authored DOS door exists in the repository with source and a
   checked-in DOS executable fixture.
2. The door artifact is clearly licensed for redistribution by OxideBBS.
3. The door runs under DOSEMU2 using the same `oxidebbs-door` launch path as
   third-party DOS doors.
4. The generated `DORINFO1.DEF` or `DOOR.SYS` file is visible to the DOS
   program in its current DOS directory.
5. The test door can be exercised through existing sysop CLI commands without
   adding a special one-off runner path.
6. `SHA256SUMS` verifies the checked-in DOS executable fixture.
7. Maintainer-only scripts exist to bootstrap the Free Pascal `i8086-msdos`
   cross compiler locally and rebuild the fixture when needed.
8. Normal Cargo build/test and `./scripts/dev-check.sh` do not require Free
   Pascal, DOSEMU2, or the staged `i8086-msdos` toolchain.
9. Optional/manual validation exists for systems that do have DOSEMU2 installed.
10. User-facing docs explain how to install DOSEMU2, configure the test door, run
   a dry run, validate COM1 serial transport expectations, and run a live test.
11. `docs/about/changelog.md` is updated under `Unreleased`.
12. `design/TASKS.md` is updated with completed work when implementation is
    finished.
13. The required validation commands pass:

    ```bash
    ./scripts/dev-check.sh
    npm run docs:build
    git diff --check
    ```

## Fixed Decisions

These decisions are part of the implementation contract.

- The bundled test door name is `Oxide Door Check`.
- The door key used in examples is `oxide-check`.
- The DOS executable filename is `OXIDECHK.EXE`.
- The DOS session-report filename written by the program is `OXIDECHK.RPT`.
- The Oxide-owned node metadata filename is `OXNODE.TXT`.
- The implementation language is Pascal.
- The compiler is Free Pascal.
- The required target is `i8086-msdos` unless this document is updated.
- The binary format is a DOS `.EXE`.
- The source file is `tools/doors/oxide-door-check/src/oxidechk.pas`.
- The checked-in binary is
  `tools/doors/oxide-door-check/dist/OXIDECHK.EXE`.
- The checked-in checksum manifest is
  `tools/doors/oxide-door-check/SHA256SUMS`.
- The checked-in executable is a conformance-test fixture, not a mandatory
  Cargo build artifact.
- The Free Pascal bootstrap script is
  `scripts/bootstrap-fpc-i8086-msdos.sh`.
- The test-door rebuild script is
  `scripts/build-oxidechk-door.sh`.
- The canonical runner is DOSEMU2, configured as `runner = "dosemu"`.
- The v1 runtime contract uses a per-door PTY bridge:
  - OxideBBS starts a run-local PTY bridge listener for the caller transport.
  - DOSEMU2 maps `COM1` with
    `$_com1 = "pts <absolute_path_to_runtime/node-001/OXCOM1.PTY>"`
    to that listener.
  - `OXIDECHK.EXE` communicates over `COM1` UART-style, not through console
    stdio.
- Generated DOSEMU2 configs include quiet runtime settings:
  `startup_verbosity=quiet`, `waitonerror=false`, `pause_when_inactive=false`,
  and `mute_when_inactive=true`.
- DOSEMU2 is headless in this deployment model and does not require a GUI wrapper.
- The v1 bridge is not a Rust FOSSIL TSR. A FOSSIL driver is a DOS-side
  interrupt/API component inside the emulated machine; OxideBBS v1 validates the
  host side by providing a real COM1/UART path that DOSEMU2 exposes to the door.
  A bundled DOS-side FOSSIL-compatible shim can be designed after v1 if needed.
- The canonical drop-file format for the example config is `DORINFO1.DEF`.
- The test program must also support `DOOR.SYS` so both supported drop-file
  writers are exercised by tests.
- The test door reads drop files from its current DOS directory.
- The test door reads `OXNODE.TXT` from its current DOS directory when present.
- The test door writes `OXIDECHK.RPT` to its current DOS directory.
- The test door must be multi-node aware. It must display the node number and
  include the node number in its report file.
- The OxideBBS DOSEMU2 launch plan must make the node runtime directory the
  current DOS directory before invoking the door executable.
- Mandatory Rust CI must not invoke DOSEMU2.
- Mandatory Rust CI must not require Free Pascal.
- Mandatory Rust CI must not require the staged `i8086-msdos` cross compiler.
- Do not add a Rust dependency for command-line parsing or DOS path handling
  for this work. Implement the small helpers directly in `oxidebbs-door`.
- Do not bundle third-party door source, third-party door binaries, abandonware,
  shareware, freeware door packages, or assets copied from other BBS packages.

## Toolchain Rationale

Free Pascal is the selected v1 toolchain for the bundled test door.

Reasons:

- Pascal is historically appropriate for BBS and door software.
- Free Pascal is packaged by Fedora, making it easier for Fedora-based
  maintainers to install than OpenWatcom.
- Free Pascal licensing is a better fit for a free-software-focused project
  than the OpenWatcom license.
- Free Pascal officially supports DOS targets, including 16-bit DOS and
  32-bit DPMI paths.
- The checked-in `OXIDECHK.EXE` means sysops still need only DOSEMU2 at runtime.
- The checked-in `OXIDECHK.EXE` also means normal Cargo validation does not
  need to build the door from source.
- Only maintainers changing `oxidechk.pas` need the Free Pascal `i8086-msdos`
  cross compiler.
- The cross compiler is staged locally by
  `scripts/bootstrap-fpc-i8086-msdos.sh`; it is not installed system-wide and
  is not part of the normal Rust build.

Relevant upstream/package references:

- Free Pascal project: `https://www.freepascal.org/fpc.html.en`
- Fedora Free Pascal package: `https://packages.fedoraproject.org/pkgs/fpc/fpc/`
- Free Pascal GO32/DPMI reference:
  `https://docs.freepascal.org/docs-html/rtl/go32/index.html`

The `go32v2` target remains explicitly excluded for v1 unless this document is
updated, because it can introduce a DPMI runtime requirement. Do not switch to
`go32v2` just to simplify maintainer onboarding. The v1 diagnostic door should
be a single checked-in DOS executable plus files generated by OxideBBS.

## Non-Goals

This project does not need to solve every DOS door compatibility issue in this
work item.

- Do not add alternate DOS runtimes in this plan. DOSEMU2 remains the v1 test
  path.
- Do not add physical serial port or modem hardware integration in v1.
- Do not add a remote admin API.
- Do not add new drop-file formats beyond the existing `DORINFO1.DEF` and
  `DOOR.SYS`.
- Do not add a full door installer/downloader.
- Do not make DOSEMU2 a required dependency for `./scripts/dev-check.sh`.
- Do not use a DOS batch file as the primary fixture. A batch file is too weak
  to validate binary execution, file reads, file writes, and exit behavior.
- Do not use a native Rust executable as the primary fixture. Native execution
  does not dogfood DOSEMU2, DOS-visible paths, or classic door assumptions.

## Phase 0 - Planning Baseline

Status: `COMPLETE`

### Objective

Record the decisions required to implement the test door without making coding
agents rediscover the runtime model.

### Completed Work

- Chose a DOS-based fixture instead of a native helper.
- Chose DOSEMU2 as the canonical runner.
- Chose an OxideBBS-authored source and binary artifact.
- Chose Free Pascal output so the door source is maintainable, historically
  aligned with BBS and door software, while keeping the generated executable as
  a checked-in fixture so normal development does not depend on cross-compiler
  availability.
- Identified the runtime contract gap: OxideBBS must keep the node runtime
  directory as the current directory while resolving the door executable from
  the configured host `working_dir`.

### Notes For Implementers

Do not re-open the language, binary format, or runner decision while
implementing later phases. If a technical blocker is found, document it in this
file before changing the plan.

## Phase 1 - Repository Layout And License Boundary

Status: `COMPLETE`

### Objective

Create a repository home for the test door that makes ownership,
redistribution, and rebuild behavior obvious.

### Required Layout

Create:

```text
tools/
  doors/
    oxide-door-check/
      README.md
      LICENSE.md
      SHA256SUMS
      src/
        oxidechk.pas
      dist/
        OXIDECHK.EXE
scripts/
  bootstrap-fpc-i8086-msdos.sh
  build-oxidechk-door.sh
```

### File Requirements

`tools/doors/oxide-door-check/README.md` must include:

- Purpose: known-good DOSEMU2 door fixture for OxideBBS.
- License: OxideBBS-authored and redistributable under Apache-2.0.
- Fixture model: `OXIDECHK.EXE` is checked in as a conformance-test fixture.
- Checksum: `SHA256SUMS` verifies the checked-in fixture.
- Build requirement: Free Pascal is required only for maintainers regenerating
  the `.EXE`.
- Target requirement: the intended target is `i8086-msdos`.
- Runtime requirement: DOSEMU2 is required to run the door through OxideBBS.
- Quick commands:

  ```bash
  (cd tools/doors/oxide-door-check && sha256sum -c SHA256SUMS)
  ./scripts/bootstrap-fpc-i8086-msdos.sh
  ./scripts/build-oxidechk-door.sh
  ```

- A short explanation that normal OxideBBS CI uses the checked-in binary and
  does not rebuild it.
- A short explanation that only maintainers changing `oxidechk.pas` need the
  staged Free Pascal cross compiler.

`tools/doors/oxide-door-check/LICENSE.md` must state that this test door is
part of OxideBBS and is distributed under the repository's Apache-2.0 license.
Do not copy a full third-party license text into this subdirectory unless the
repository already has the canonical full license file and the wording points
to it.

`tools/doors/oxide-door-check/SHA256SUMS` must:

- Be generated by `sha256sum`.
- Use paths relative to `tools/doors/oxide-door-check/`.
- Include exactly the checked-in executable fixture:

  ```text
  dist/OXIDECHK.EXE
  ```

Validation must work with:

```bash
cd tools/doors/oxide-door-check
sha256sum -c SHA256SUMS
```

`scripts/bootstrap-fpc-i8086-msdos.sh` must:

- Use `#!/usr/bin/env bash`.
- Use `set -euo pipefail`.
- Resolve paths relative to the repository root.
- Stage the official Free Pascal `i8086-msdos` cross compiler locally under:

  ```text
  target/fpc-i8086-msdos/
  ```

- Avoid `sudo`, `dnf`, system package installation, and writes outside the
  repository.
- Be idempotent. Re-running it must reuse or refresh the staged toolchain
  safely.
- Pin the Free Pascal version used for the door fixture in the script.
- Download only from official Free Pascal project distribution locations or
  mirrors documented by the Free Pascal project.
- Verify downloaded archive checksums before using them. If upstream checksum
  files are used, verify those first according to the implementation's chosen
  source.
- Produce or expose a compiler command usable by the build script for:

  ```text
  i8086-msdos
  ```

- Fail with a clear message if the official toolchain cannot be staged.
- Print the staged compiler path and version on success.

`scripts/build-oxidechk-door.sh` must:

- Use `#!/usr/bin/env bash`.
- Use `set -euo pipefail`.
- Resolve paths relative to the repository root.
- Require the staged compiler from `target/fpc-i8086-msdos/`.
- If the staged compiler is missing, print:

  ```text
  missing staged Free Pascal i8086-msdos compiler; run ./scripts/bootstrap-fpc-i8086-msdos.sh
  ```

  and exit non-zero.

- Create a temporary build directory under:

  ```text
  target/oxidechk-door-build/
  ```

- Compile `tools/doors/oxide-door-check/src/oxidechk.pas` with Turbo Pascal
  compatibility and `i8086-msdos` target semantics.
- Copy the rebuilt executable to:

  ```text
  tools/doors/oxide-door-check/dist/OXIDECHK.EXE
  ```

- Regenerate:

  ```text
  tools/doors/oxide-door-check/SHA256SUMS
  ```

  with a relative `dist/OXIDECHK.EXE` entry.

- Run `(cd tools/doors/oxide-door-check && sha256sum -c SHA256SUMS)` before
  exiting.
- Print the compiler version used for the rebuilt fixture.

### Acceptance Criteria

- The directory layout exists exactly as specified.
- The license note makes clear that no abandonware, shareware, or freeware door
  package has been copied into the repository.
- `SHA256SUMS` validates the checked-in `OXIDECHK.EXE`.
- `scripts/bootstrap-fpc-i8086-msdos.sh` stages the maintainer-only compiler
  locally.
- `scripts/build-oxidechk-door.sh` rebuilds the checked-in fixture when the
  staged compiler exists.
- Neither script runs as part of `./scripts/dev-check.sh`.
- `git diff --check` passes.

## Phase 2 - DOS Door Program

Status: `COMPLETE`

### Objective

Implement `OXIDECHK.EXE`, a tiny Pascal DOS door that proves OxideBBS can
generate a drop file, launch a DOS binary under DOSEMU2, pass caller metadata,
accept keyboard input, write a file, and exit cleanly.

### Pascal Requirements

The source file must be:

```text
tools/doors/oxide-door-check/src/oxidechk.pas
```

Implementation rules:

- Use Free Pascal source.
- Use Turbo Pascal compatibility mode (`-Mtp`) for the build.
- Keep the code Pascal-focused and simple. Do not use Object Pascal classes,
  generics, exceptions, RTTI, Lazarus units, or Delphi-only features.
- Target `i8086-msdos` with Free Pascal.
- Use simple Pascal text-file I/O and console I/O.
- Do not use external libraries.
- Do not embed third-party code snippets.
- Use ASCII text and CRLF line endings for output.
- Keep all filenames DOS 8.3 compatible.
- Read and write only in the current DOS directory.
- Do not inspect host paths.
- Do not require ANSI escape support.
- Do not require mouse, graphics, sound, timers, or extended memory.
- The generated `.EXE` must run in DOSEMU2 without requiring a separate DPMI
  host, extender, DLL, overlay, Pascal runtime file, or unit file.
- Record the Free Pascal version and target used to build the checked-in binary
  in `tools/doors/oxide-door-check/README.md`.

### Target Requirements

Free Pascal has multiple DOS-related target paths. The v1 test door must use
the simplest runtime path:

- Required target: `i8086-msdos`.
- Build command target flags: `-Pi8086 -Tmsdos`.
- Required language mode: `-Mtp`.
- Disallowed implicit fallback: `go32v2`.

Do not silently fall back to `go32v2`. `go32v2` is a 32-bit DPMI target and may
require a DPMI runtime such as `CWSDPMI.EXE`. A `go32v2` build can be used only
as an explicit future design change after documenting the runtime file,
licensing, installation, and DOSEMU2 validation story.

If the staged Free Pascal cross compiler cannot produce `i8086-msdos` output,
`scripts/build-oxidechk-door.sh` must fail with a clear message that says the
staged Free Pascal toolchain cannot build the required target.

### Runtime Behavior

On launch, `OXIDECHK.EXE` must:

1. Print a title:

   ```text
   Oxide Door Check
   ```

2. Try to open `DORINFO1.DEF` in the current directory.
3. If `DORINFO1.DEF` is not present, try to open `DOOR.SYS`.
4. If neither file is present, print:

   ```text
   ERROR: no supported drop file found
   ```

   and exit with code `2`.

5. Parse a minimal caller summary from the drop file.
6. Parse the node number from `OXNODE.TXT` when present.
7. If `OXNODE.TXT` is not present and `DOOR.SYS` is the active drop file, parse
   the node number from `DOOR.SYS` line 4.
8. Print the detected drop-file type.
9. Print the parsed caller and node summary.
10. Prompt:

   ```text
   [I]nfo  [R]eport  [Q]uit:
   ```

11. Accept single-key input, case-insensitive.
12. On `I`, reprint the caller and node summary.
13. On `R`, write `OXIDECHK.RPT` in the current directory and print:

    ```text
    Report file written
    ```

14. On `Q`, print:

    ```text
    Returning to OxideBBS
    ```

    and exit with code `0`.

15. On any other key, reprint the prompt.

### Drop-File Parsing Requirements

The parser does not need to implement every field in every legacy drop-file
variant. It must parse only the fields OxideBBS already writes.

For `DORINFO1.DEF`, OxideBBS writes:

```text
line 1: board name
line 2: sysop first name
line 3: sysop last name
line 4: COM port
line 5: baud string
line 6: reserved/zero
line 7: caller first name
line 8: caller last name
line 9: caller location
line 10: ANSI/graphics flag
line 11: caller security level
line 12: caller minutes remaining
```

`OXIDECHK.EXE` must display:

- board name from line 1
- sysop name from line 2
- caller name from lines 6 and 7 joined with one space
- caller location from line 8
- security level from line 9
- minutes remaining from line 10
- node number from `OXNODE.TXT`

For `DOOR.SYS`, OxideBBS writes:

```text
line 1: COM port with colon
line 2: baud rate
line 3: data bits
line 4: node number
line 5: minutes remaining
line 6: caller alias
line 7: caller real name
line 8: caller location
line 9: caller security level
```

`OXIDECHK.EXE` must display:

- node number from line 4
- minutes remaining from line 5
- caller alias from line 6
- caller real name from line 7
- caller location from line 8
- security level from line 9

If a required field is blank or missing, display `unknown` for that field and
continue. Missing optional fields must not crash the program.

### Node Awareness Requirements

`OXIDECHK.EXE` is a multi-node test fixture. It must prove that two simultaneous
door sessions do not share mutable runtime files.

OxideBBS must write this helper file beside the drop file for every door run:

```text
OXNODE.TXT
```

The file content must be ASCII with CRLF line endings:

```text
node=<node number>
```

Examples:

```text
node=1
```

```text
node=12
```

This file is OxideBBS-specific. It must not replace standard drop-file content
and it must not be required by third-party doors. Third-party DOS doors should
ignore it because it is just an extra file in the per-node runtime directory.

`OXIDECHK.EXE` must:

- read `OXNODE.TXT` when present
- display the parsed node number
- include the parsed node number in `OXIDECHK.RPT`
- fall back to `DOOR.SYS` line 4 if `OXNODE.TXT` is missing and `DOOR.SYS` is
  the active drop file
- display `unknown` for the node number if neither source is available

### Report File Requirements

When the caller chooses `R`, write:

```text
OXIDECHK.RPT
```

The file content must be ASCII with CRLF line endings and include:

```text
Oxide Door Check
drop_file=<DORINFO1.DEF or DOOR.SYS>
node=<parsed node number or unknown>
caller=<parsed caller display name>
result=report
```

If the report file cannot be created or written, print:

```text
ERROR: report file write failed
```

and exit with code `3`.

### Exit Codes

Exit codes are part of the fixture contract:

- `0`: normal quit after successfully reading a drop file
- `2`: neither `DORINFO1.DEF` nor `DOOR.SYS` was found
- `3`: report file write failed

Do not add random or time-dependent exit codes.

### Acceptance Criteria

- The checked-in `dist/OXIDECHK.EXE` fixture exists.
- `SHA256SUMS` validates the checked-in binary.
- `scripts/build-oxidechk-door.sh` can reproduce `dist/OXIDECHK.EXE` when the
  staged Free Pascal cross compiler exists.
- The binary is small enough to remain reviewable as a generated artifact.
  There is no hard size limit, but it should remain small enough that reviewers
  can reason about the fixture.
- The source contains comments for non-obvious DOS/Free Pascal compatibility
  choices.
- A local DOSEMU2 run can show the prompt and exit with `Q`.

## Phase 3 - DOSEMU2 Runtime Contract

Status: `COMPLETE`

### Objective

Fix the DOSEMU2 launch plan so generated drop files are visible to DOS programs
in a predictable current directory. This phase is required before the test door
can be considered useful.

### Current Behavior To Replace

At the time this plan was written, `oxidebbs-door` writes the drop file to the
node runtime directory, then builds a DOSEMU2 plan that must keep the node
runtime directory as the current directory while resolving the executable from
the configured host `working_dir`.

That makes the drop file invisible to a normal DOS program that expects
`DORINFO1.DEF` or `DOOR.SYS` in the current directory.

### Required Runtime Contract

For DOSEMU2 runs:

- DOSEMU2 runs in the node runtime directory, and runtime-generated files remain
  in that node directory.
- A per-door PTY bridge is started for the run.
- `$_com1` is configured to the runtime PTY in `OXDOSEMU2.CONF`.
- Drop files and `OXNODE.TXT` are written into the same node runtime directory.
- Door-created per-run files, including `OXIDECHK.RPT`, are written into that same
  directory.
- The configured command is resolved from the door `working_dir` on the host,
  staged into the node runtime directory, and then launched by DOS filename
  with DOSEMU2 `-E`. DOSEMU2 starts from the node runtime directory with `-K`,
  so drop-file reads and report writes stay rooted in the runtime directory.
- DOSEMU2 maps COM1 to the run-local PTY path with
  `$_com1 = "pts <absolute_path_to_runtime/node-001/OXCOM1.PTY>"`
  semantics.

The generated DOSEMU2 command sequence must include a run-local config file:

```text
-f <node runtime dir>/OXDOSEMU2.CONF
```

`OXDOSEMU2.CONF` must contain:

```ini
$_cpu_vm = "emulated"
$_cpu_vm_dpmi = "emulated"
$_sound = (off)
$_mouse_internal = (off)
$_joy_device = ""
$_pktdriver = (off)
$_tcpdriver = (off)
$_ttylocks = ""
$_com1 = "pts <absolute_path_to_runtime/node-001/OXCOM1.PTY>"
```

The generated DOSEMU2 command sequence must then run:

```text
<dosemu_bin> -f <node runtime dir>/OXDOSEMU2.CONF -dumb -quiet -K <node runtime dir> -E "<runtime-staged command and args>"
```

For the Oxide test door example:

```toml
command = "OXIDECHK.EXE"
```

the resolved DOS command must be:

```text
OXIDECHK.EXE
```

The source executable is resolved from the configured host `working_dir` and
staged into the node runtime directory before launch.

### Command Resolution Rules

Implement a small helper in `oxidebbs-door`; suggested name:

```rust
fn resolve_dosemu2_command(working_dir: &Path, command: &str) -> Dosemu2Command
```

Rules:

1. Trim leading and trailing ASCII whitespace.
2. If the command is empty, validation must reject the door config before plan
   generation.
3. Split the command into the first token and the remaining argument string at
   the first ASCII whitespace character.
4. If the first token contains `:` or `\` or `/`, reject it for v1 with a clear
   error. Set `working_dir` and use a bare DOS 8.3 filename instead.
5. Otherwise, resolve the first token against the configured host
   `working_dir`.
6. Preserve remaining tokens as the argument vector.

Examples:

| Configured Command | Resolved Host Executable | Arguments |
| --- | --- | --- |
| `OXIDECHK.EXE` | `<working_dir>/OXIDECHK.EXE` | `[]` |
| `LORD.EXE /N1` | `<working_dir>/LORD.EXE` | `["/N1"]` |
| `C:\LORD\START.BAT` | rejected | rejected |
| `UTILS\DOOR.EXE` | rejected | rejected |

Do not add quoted-path support in this phase. DOS 8.3 paths should be used for
door commands. If a configured command starts with a quote, validation should
return a clear unsupported-command error.

### Plan Structure Requirements

Update:

```text
crates/oxidebbs-door/src/lib.rs
```

`DoorRunPlan` must continue to include:

- `program`
- `args`
- `working_dir`
- `drop_file_path`
- `timeout`

The meaning of `working_dir` for DOSEMU2 plans must be updated to the host node
runtime directory, because that is the directory where the child process should
start from the host perspective. If DOSEMU2 itself does not require a host
current directory, using the runtime directory is still the least surprising
choice because all generated run files live there.

`prepare_door_run` must continue to write the drop file before constructing the
plan.

`dosemu2_plan` must use `request.runtime_dir` for run-local artifacts and PTY path
construction. If the current function signature does not include enough data,
change it rather than deriving paths from `drop_file_path`.

### Validation Requirements

Update server-side door validation in:

```text
crates/oxidebbs-server/src/door_session.rs
```

Validation must continue to ensure:

- door working directory exists
- configured runner exists on `PATH` or as an executable path
- drop-file format is supported
- time limit is positive
- node runtime directory is writable

Validation of the configured command must use the first token before DOSEMU2
resolution. For `command = "OXIDECHK.EXE"`, validate that:

```text
<working_dir>/OXIDECHK.EXE
```

exists as a file.

For `command = "LORD.EXE /N1"`, validate:

```text
<working_dir>/LORD.EXE
```

Do not validate argument files in this phase.

If a command begins with a quote, validation must fail with wording that says
quoted DOS commands are not supported yet and that DOS 8.3 paths should be
used.

### Acceptance Criteria

- `DORINFO1.DEF` and `DOOR.SYS` are generated under the host node runtime
  directory.
- DOSEMU2 plans use the node runtime directory as the host working directory
  and `-K` directory.
- DOSEMU2 plans resolve bare commands from the configured host `working_dir`.
- Path-like DOS commands are rejected for v1 with a clear error.
- Existing dry-run behavior still writes drop files and does not launch DOSEMU2.
- Existing live door bridge behavior still tracks start/finish/timeout records.

## Phase 4 - Config And Sysop CLI Integration

Status: `COMPLETE`

### Objective

Make the test door easy for a developer or sysop to use without inventing a
new command group.

### Config Files

Update:

```text
config/doors.example.toml
```

Replace the current third-party example with the Oxide-owned test door:

```toml
[[definitions]]
key = "oxide-check"
name = "Oxide Door Check"
runner = "dosemu"
working_dir = "./doors/oxide-door-check/dist"
command = "OXIDECHK.EXE"
drop_file = "DORINFO1.DEF"
exclusive = false
time_limit_minutes = 5
enabled = true
```

Update:

```text
config/oxidebbs.example.toml
```

Replace the placeholder `Example Door` definition with:

```toml
[[doors.definitions]]
key = "oxide-check"
name = "Oxide Door Check"
runner = "dosemu"
working_dir = "./doors/oxide-door-check/dist"
command = "OXIDECHK.EXE"
drop_file = "DORINFO1.DEF"
exclusive = false
time_limit_minutes = 5
enabled = false
```

The main example config must keep the test door disabled by default. This keeps
fresh configuration validation from requiring DOSEMU2 to be installed before the
operator intentionally enables door testing. The dedicated `doors.example.toml`
may keep it enabled because that file is specifically for door setup examples.

### Setup Flow

Review:

```text
crates/oxidebbs-server/src/setup.rs
```

If setup currently generates a fictional or third-party placeholder door,
replace it with the disabled `oxide-check` definition above. Setup-generated
configs should not require DOSEMU2 for a clean first `check` unless the sysop
enables the door.

### Sysop CLI Commands

Do not create a new `oxide-test-door` command.

The existing commands must work with `oxide-check`:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors list
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors check oxide-check
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors dropfile oxide-check --user sysop --node 1 --format DORINFO1.DEF
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors test oxide-check --user sysop --dry-run
```

Live execution runs through a caller session:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml serve
```

Connect over telnet, log in as a caller, open the caller `Doors` menu, and
select `oxide-check`.

If DOSEMU2 is not installed, launch must fail with a clear missing-runner error.
That is acceptable. The failure must not look like an OxideBBS internal error.

### Acceptance Criteria

- Example configs reference only the Oxide-owned test door.
- The main example config does not require DOSEMU2 for default setup.
- The dedicated doors example is ready to copy into a test setup.
- Existing sysop CLI commands can list, check, generate drop files, dry-run,
  and live-run the test door.

## Phase 5 - Testing Automation

Status: `COMPLETE`

### Objective

Add deterministic automated tests for the OxideBBS side of the contract, and
add an optional smoke test for hosts with DOSEMU2 installed.

### Rust Unit Tests

Update tests in:

```text
crates/oxidebbs-door/src/lib.rs
```

Add or update tests for:

- `prepare_door_run` writes `DORINFO1.DEF` into `request.runtime_dir`.
- `prepare_door_run` writes `DOOR.SYS` into `request.runtime_dir`.
- `prepare_door_run` writes `OXNODE.TXT` into `request.runtime_dir`.
- `OXNODE.TXT` contains the request node number with CRLF line endings.
- `dosemu2_plan` keeps the node runtime directory as the door working directory.
- `OXDOSEMU2.CONF` includes the PTY-compliant COM1 mapping and runtime settings.
- `dosemu2_plan` resolves command arguments and executes within the node runtime
  directory.
- `dosemu2_plan` resolves `OXIDECHK.EXE` against the configured host
  `working_dir`.
- `resolve_dosemu2_command("LORD.EXE /N1")` preserves `/N1` as an argument.
- commands with a drive or path are rejected for v1.
- empty commands are rejected by validation before plan generation.

Update tests in:

```text
crates/oxidebbs-server/src/door_session.rs
```

Add or update tests for:

- command validation accepts `OXIDECHK.EXE` when it exists in `working_dir`.
- command validation accepts `LORD.EXE /N1` when `LORD.EXE` exists in
  `working_dir`.
- command validation rejects quoted commands with a clear message.
- dry-run still creates a drop file under the runtime directory and cleans it up
  according to existing behavior.
- dry-run creates `OXNODE.TXT` under the runtime directory before cleanup.

### CLI Tests

If the existing test structure has CLI integration tests, add coverage for:

- `doors list` includes `oxide-check` when the example config is loaded.
- `doors dropfile oxide-check --format DORINFO1.DEF` writes a valid
  `DORINFO1.DEF`.
- `doors dropfile oxide-check --format DORINFO1.DEF` also writes node metadata
  when the command path exercises full run preparation.
- `doors test oxide-check --dry-run` succeeds without DOSEMU2.

If CLI integration tests are not already present for door commands, do not
create a large new test harness solely for this phase. Cover the behavior at
the `oxidebbs-door` and server command-helper level instead.

### Optional DOSEMU2 Smoke Script

Add:

```text
scripts/test-oxide-door-dosemu2.sh
```

The script must:

- Use `#!/usr/bin/env bash`.
- Use `set -euo pipefail`.
- Resolve paths relative to the repository root.
- Check for `dosemu` on `PATH`.
- Verify that the executable is DOSEMU2. A binary reporting `dosemu-1.x` is the
  legacy DOSEMU runtime and must be treated as unsupported for this smoke test,
  because it does not accept the run-local `pts <path>` COM1 mapping.
- If DOSEMU2 is missing or the available `dosemu` is legacy DOSEMU 1.x, print a
  clear `SKIP:` line and exit `77`.
- If the executable is missing, print:

  ```text
  SKIP: dosemu not found
  ```

  and exit `77`.

- Verify that `tools/doors/oxide-door-check/dist/OXIDECHK.EXE` exists.
- Create a temporary runtime directory under `target/`.
- Create a representative `DORINFO1.DEF` in that runtime directory.
- Create a representative `OXNODE.TXT` in that runtime directory.
- Launch DOSEMU2 using the same runtime-directory/current-directory pattern
  required in Phase 3.
- Use DOSEMU2 commands that run the door in the most automated way practical.

If fully automated key input is not reliable across DOSEMU2 versions, the script
may be interactive. In that case it must clearly print:

```text
Press I to view node info, R to write a report, then Q to return.
```

and must still be optional, not part of `./scripts/dev-check.sh`.

### Optional DOSEMU2 Integration Tests

If Rust integration tests are added that actually launch DOSEMU2, they must be
opt-in ignored tests. Use this pattern:

```rust
#[ignore = "requires DOSEMU2"]
```

Do not gate normal behavior behind a Cargo feature solely for this phase. The
required opt-in command for DOSEMU2-backed Rust tests must be documented as:

```bash
cargo test --workspace --locked -- --ignored
```

If this command would run unrelated ignored tests, document the narrower test
name instead. DOSEMU2-backed tests must skip or fail clearly when `dosemu` is not
on `PATH`, and they must never run during `./scripts/dev-check.sh`.

### CI Contract

Do not add the DOSEMU2 smoke script to mandatory CI in this phase.

It is acceptable to add a future optional GitHub Actions job that installs
DOSEMU2 and runs the script, but that job must be allowed to skip cleanly when
the environment cannot support DOSEMU2. This plan does not require that optional
job.

Normal Cargo build/test, `cargo test --workspace --locked`, and
`./scripts/dev-check.sh` must not require Free Pascal, DOSEMU2, or the staged
`i8086-msdos` toolchain.

### Acceptance Criteria

- Mandatory Rust tests pass without DOSEMU2 installed.
- Mandatory Rust tests pass without Free Pascal installed.
- Mandatory Rust tests pass without the staged `i8086-msdos` toolchain.
- The optional DOSEMU2 script skips with exit `77` when DOSEMU2 is missing.
- On a developer machine with DOSEMU2, the optional script can run the checked-in
  `OXIDECHK.EXE`.
- Any DOSEMU2-backed Rust integration tests are ignored by default and documented
  with an explicit opt-in command.

## Phase 6 - Documentation And Changelog

Status: `COMPLETE`

### Objective

Document the test door as both a developer fixture and a sysop-facing setup
tool.

### Design Documentation

Update:

```text
design/DOORS.md
```

Required additions:

- Mention `Oxide Door Check` as the bundled, Oxide-owned test door.
- Document that the test door source is Free Pascal and the required build
  target is `i8086-msdos`.
- Document that `OXIDECHK.EXE` is a checked-in conformance-test fixture, not a
  mandatory Cargo build artifact.
- Document that only maintainers changing `oxidechk.pas` need to run
  `scripts/bootstrap-fpc-i8086-msdos.sh` and
  `scripts/build-oxidechk-door.sh`.
- Document that the DOSEMU2 runner starts from the node runtime directory so
  drop files are visible and per-run reports stay isolated by node.
- Document that bare configured commands are resolved from the configured host
  `working_dir`.
- Document that OxideBBS writes `OXNODE.TXT` as Oxide-owned per-node metadata
  for diagnostics, and that third-party doors are not expected to consume it.
- Keep the legal note: no copyrighted or abandonware DOS doors are bundled.

Update:

```text
design/SPEC.md
```

Required additions:

- State that v1 includes a redistributable DOSEMU2 test door.
- State that the bundled test door is implemented in Free Pascal.
- State that the generated executable is committed as a fixture and verified
  with `SHA256SUMS`.
- State that the test door is multi-node aware and reports the active node
  number.
- State that the bundled test door is not a game content dependency and does
  not imply redistribution rights for third-party doors.

Update:

```text
design/TASKS.md
```

Required additions:

- Add a completed or in-progress section for `Oxide Door Check` when
  implementation begins.
- Mark tasks complete only after implementation and validation are complete.

### Operator Documentation

Update:

```text
docs/project/sysop-cli.md
```

Required additions:

- Show how to run `doors check oxide-check`.
- Show how to generate a `DORINFO1.DEF` drop file for `oxide-check`.
- Show how to dry-run the test door.
- Explain that live run requires DOSEMU2.

Update setup/getting-started documentation if present:

```text
docs/project/setup.md
docs/project/getting-started.md
docs/project/deployment.md
```

Only update files that exist. Required content somewhere in operator docs:

- Install DOSEMU2 before live door testing.
- Do not install Free Pascal or the `i8086-msdos` cross compiler for normal
  sysop use.
- Enable the `oxide-check` door in config.
- Run config validation.
- Run a dry run.
- Run a live local test.
- Connect to the BBS and launch the door from the caller `Doors` menu.

Use commands consistent with the existing docs style.

### Changelog

Update:

```text
docs/about/changelog.md
```

Add an `Unreleased` entry that mentions:

- bundled Oxide-owned DOSEMU2 test door
- Free Pascal source with a checked-in `OXIDECHK.EXE` fixture and `SHA256SUMS`
- DOSEMU2 launch-plan correction that makes drop files visible in the DOS
  current directory
- maintainer-only Free Pascal bootstrap/rebuild scripts
- optional DOSEMU2 smoke test script, if implemented

Follow `design/VERSIONING_GUIDE.md`. Do not bump crate versions unless this
work is being combined with an explicit release-version change.

### Acceptance Criteria

- Design docs and operator docs agree on the DOSEMU2 runtime-directory/current
  directory contract.
- Changelog mentions the user-visible behavior change.
- Documentation does not imply that OxideBBS ships third-party DOS games.
- Documentation commands use actual repo paths and command names.
- `npm run docs:build` passes.

## Phase 7 - Final Validation

Status: `COMPLETE`

### Objective

Finish the branch with a clean validation record and no hidden follow-up tasks
inside the implementation phases.

### Required Commands

Run:

```bash
(cd tools/doors/oxide-door-check && sha256sum -c SHA256SUMS)
./scripts/dev-check.sh
npm run docs:build
git diff --check
```

When changing the Pascal source or regenerating the fixture, also run:

```bash
./scripts/bootstrap-fpc-i8086-msdos.sh
./scripts/build-oxidechk-door.sh
(cd tools/doors/oxide-door-check && sha256sum -c SHA256SUMS)
```

If DOSEMU2 is installed, also run:

```bash
OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
```

If DOSEMU2 is missing, do not treat that as a failure. Record the skip in the
final implementation notes. If the Pascal source changed and the Free Pascal
bootstrap/build scripts fail, treat that as a blocker for completing this plan.

### Final Review Checklist

Before marking this plan complete:

- [x] All phase-map statuses are updated.
- [x] `tools/doors/oxide-door-check/dist/OXIDECHK.EXE` exists.
- [x] `tools/doors/oxide-door-check/SHA256SUMS` validates.
- [x] `scripts/bootstrap-fpc-i8086-msdos.sh` exists.
- [x] `scripts/build-oxidechk-door.sh` exists.
- [x] `config/doors.example.toml` references `oxide-check`.
- [x] `config/oxidebbs.example.toml` references disabled `oxide-check`.
- [x] The DOSEMU2 plan runs from the node runtime directory with `-K`.
- [x] The DOSEMU2 plan resolves the executable from the configured host
  `working_dir`.
- [x] The drop file path points into the node runtime directory.
- [x] Door command validation handles command arguments.
- [x] Mandatory tests do not require DOSEMU2.
- [x] Mandatory tests do not require Free Pascal.
- [x] Optional DOSEMU2 smoke testing is documented.
- [x] Operator docs explain how to enable and run the test door.
- [x] `docs/about/changelog.md` is updated.
- [x] `design/TASKS.md` is updated.
- [x] `./scripts/dev-check.sh` passes.
- [x] `npm run docs:build` passes.
- [x] `git diff --check` passes.

## Implementation Notes Template

When implementation begins, add notes here instead of leaving decisions in chat
history.

```text
Implementation started: 2026-06-01
Implementation completed: 2026-06-01
Free Pascal validation: `./scripts/bootstrap-fpc-i8086-msdos.sh` staged Free
Pascal 3.2.2 `ppcross8086`; `./scripts/build-oxidechk-door.sh` rebuilt
`dist/OXIDECHK.EXE`; `(cd tools/doors/oxide-door-check && sha256sum -c
SHA256SUMS)` passed.
DOSEMU2 smoke validation:
`OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh` passed with
`dosemu2-2.0pre9` after the host loader path exposed `libdj64.so.0`.
`OXIDE_DOOR_MULTI_NODE=1 OXIDE_DOOR_INTERACTIVE=1
./scripts/test-oxide-door-dosemu2.sh` also passed. The smoke test verified the
COM1 PTY bridge, scripted `I`/`R`/`Q` input, per-node `OXIDECHK.RPT` creation,
and separate node runtime directories.
Notable decisions: `OXIDECHK.EXE` is committed as a conformance-test fixture;
the Free Pascal i8086/MS-DOS cross compiler is staged under `target/` and is
required only when maintainers rebuild the door fixture.
```
