# Oxide Test Door Implementation Plan

This document defines the implementation plan for a bundled OxideBBS-owned DOS
test door. The goal is to give developers and sysops a known-good door program
that exercises the same DOSBox path that real v1 doors will use, without
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
| Phase 1 - Repository Layout And License Boundary | TODO | Add a clear, source-owned home for the DOS test door and prevent licensing ambiguity. | `tools/doors/oxide-door-check/` layout, license note, build script contract. |
| Phase 2 - DOS Door Program | TODO | Implement the actual 16-bit DOS test program. | OpenWatcom C source, checked-in `OXIDECHK.EXE`, checksum, deterministic behavior. |
| Phase 3 - DOSBox Runtime Contract | TODO | Make OxideBBS launch DOS doors with the drop file visible inside DOSBox. | Updated `oxidebbs-door` plan generation and validation tests. |
| Phase 4 - Config And Sysop CLI Integration | TODO | Make the test door easy to configure and exercise from existing commands. | Example config, setup guidance, `doors check/test/dropfile` compatibility. |
| Phase 5 - Testing Automation | TODO | Cover the fixture without making CI depend on DOSBox. | Rust unit/integration tests plus an optional DOSBox smoke script. |
| Phase 6 - Documentation And Changelog | TODO | Document sysop usage and record the user-visible behavior change. | Updated design docs, operator docs, task list, and changelog. |
| Phase 7 - Final Validation | TODO | Prove the branch is ready to merge. | `./scripts/dev-check.sh`, docs build, whitespace check, optional DOSBox smoke result. |

## Definition Of Done

The Oxide test door is complete only when all of the following are true:

1. A project-authored DOS door exists in the repository with source and a
   reproducible binary artifact.
2. The door artifact is clearly licensed for redistribution by OxideBBS.
3. The door runs under DOSBox using the same `oxidebbs-door` launch path as
   third-party DOS doors.
4. The generated `DORINFO1.DEF` or `DOOR.SYS` file is visible to the DOS
   program in its current DOS directory.
5. The test door can be exercised through existing sysop CLI commands without
   adding a special one-off runner path.
6. Normal CI does not require DOSBox or OpenWatcom to be installed.
7. Optional/manual validation exists for systems that do have DOSBox installed.
8. User-facing docs explain how to install DOSBox, configure the test door, run
   a dry run, and run a live test.
9. `docs/about/changelog.md` is updated under `Unreleased`.
10. `design/TASKS.md` is updated with completed work when implementation is
    finished.
11. The required validation commands pass:

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
- The implementation language is C.
- The DOS compiler is OpenWatcom C.
- The binary format is a 16-bit DOS `.EXE`.
- The source file is `tools/doors/oxide-door-check/src/oxidechk.c`.
- The checked-in binary is
  `tools/doors/oxide-door-check/dist/OXIDECHK.EXE`.
- The checked-in checksum is
  `tools/doors/oxide-door-check/dist/OXIDECHK.EXE.sha256`.
- The build script is
  `tools/doors/oxide-door-check/build.sh`.
- The canonical runner is DOSBox, configured as `runner = "dosbox"`.
- The canonical drop-file format for the example config is `DORINFO1.DEF`.
- The test program must also support `DOOR.SYS` so both supported drop-file
  writers are exercised by tests.
- The test door reads drop files from its current DOS directory.
- The test door reads `OXNODE.TXT` from its current DOS directory when present.
- The test door writes `OXIDECHK.RPT` to its current DOS directory.
- The test door must be multi-node aware. It must display the node number and
  include the node number in its report file.
- The OxideBBS DOSBox launch plan must make the node runtime directory the
  current DOS directory before invoking the door executable.
- Mandatory Rust CI must not invoke DOSBox.
- Mandatory Rust CI must not require OpenWatcom.
- Do not add a Rust dependency for command-line parsing or DOS path handling
  for this work. Implement the small helpers directly in `oxidebbs-door`.
- Do not bundle third-party door source, third-party door binaries, abandonware,
  shareware, freeware door packages, or assets copied from other BBS packages.

## Non-Goals

This project does not need to solve every DOS door compatibility issue in this
work item.

- Do not add DOSEMU support in this plan. DOSBox remains the v1 test path.
- Do not add serial/modem support.
- Do not add a remote admin API.
- Do not add new drop-file formats beyond the existing `DORINFO1.DEF` and
  `DOOR.SYS`.
- Do not add a full door installer/downloader.
- Do not make DOSBox a required dependency for `./scripts/dev-check.sh`.
- Do not use a DOS batch file as the primary fixture. A batch file is too weak
  to validate binary execution, file reads, file writes, and exit behavior.
- Do not use a native Rust executable as the primary fixture. Native execution
  does not dogfood DOSBox, DOS-visible paths, or classic door assumptions.

## Phase 0 - Planning Baseline

Status: `COMPLETE`

### Objective

Record the decisions required to implement the test door without making coding
agents rediscover the runtime model.

### Completed Work

- Chose a DOS-based fixture instead of a native helper.
- Chose DOSBox as the canonical runner.
- Chose an OxideBBS-authored source and binary artifact.
- Chose C and OpenWatcom output so the door source is maintainable while still
  producing a real DOS executable for DOSBox.
- Identified the runtime contract gap: OxideBBS currently writes the drop file
  to the node runtime directory while the DOSBox plan mounts and enters the
  door working directory.

### Notes For Implementers

Do not re-open the language, binary format, or runner decision while
implementing later phases. If a technical blocker is found, document it in this
file before changing the plan.

## Phase 1 - Repository Layout And License Boundary

Status: `TODO`

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
      build.sh
      src/
        oxidechk.c
      dist/
        OXIDECHK.EXE
        OXIDECHK.EXE.sha256
```

### File Requirements

`tools/doors/oxide-door-check/README.md` must include:

- Purpose: known-good DOSBox door fixture for OxideBBS.
- License: OxideBBS-authored and redistributable under Apache-2.0.
- Build requirement: OpenWatcom C is required only when regenerating the
  `.EXE`.
- Runtime requirement: DOSBox is required to run the door through OxideBBS.
- Quick commands:

  ```bash
  ./tools/doors/oxide-door-check/build.sh
  sha256sum -c tools/doors/oxide-door-check/dist/OXIDECHK.EXE.sha256
  ```

- A short explanation that normal OxideBBS CI uses the checked-in binary and
  does not rebuild it.

`tools/doors/oxide-door-check/LICENSE.md` must state that this test door is
part of OxideBBS and is distributed under the repository's Apache-2.0 license.
Do not copy a full third-party license text into this subdirectory unless the
repository already has the canonical full license file and the wording points
to it.

`tools/doors/oxide-door-check/build.sh` must:

- Use `#!/usr/bin/env bash`.
- Use `set -euo pipefail`.
- Resolve paths relative to the script location.
- Check for `wcl` on `PATH`.
- Print a clear message and exit non-zero if OpenWatcom C is missing.
- Compile:

  ```bash
  wcl -bt=dos -ms -q -fe=dist/OXIDECHK.EXE src/oxidechk.c
  ```

- Regenerate:

  ```bash
  sha256sum OXIDECHK.EXE > OXIDECHK.EXE.sha256
  ```

  The checksum file path should be relative to `dist/`, so it can be validated
  with `cd tools/doors/oxide-door-check/dist && sha256sum -c OXIDECHK.EXE.sha256`.

### Acceptance Criteria

- The directory layout exists exactly as specified.
- The license note makes clear that no abandonware, shareware, or freeware door
  package has been copied into the repository.
- `build.sh` works on a system with OpenWatcom C installed.
- `build.sh` does not run as part of `./scripts/dev-check.sh`.
- `git diff --check` passes.

## Phase 2 - DOS Door Program

Status: `TODO`

### Objective

Implement `OXIDECHK.EXE`, a tiny DOS door that proves OxideBBS can generate a
drop file, launch a DOS binary under DOSBox, pass caller metadata, accept
keyboard input, write a file, and exit cleanly.

### C Requirements

The source file must be:

```text
tools/doors/oxide-door-check/src/oxidechk.c
```

Implementation rules:

- Use C accepted by OpenWatcom C for 16-bit DOS targets.
- Keep the code C-focused and simple. Do not use C++.
- Target a 16-bit DOS `.EXE` binary with OpenWatcom.
- Use standard C file I/O and simple console I/O.
- Do not use external libraries.
- Do not embed third-party code snippets.
- Use ASCII text and CRLF line endings for output.
- Keep all filenames DOS 8.3 compatible.
- Read and write only in the current DOS directory.
- Do not inspect host paths.
- Do not require ANSI escape support.
- Do not require mouse, graphics, sound, timers, or extended memory.
- The generated `.EXE` must run in DOSBox without requiring a separate DPMI
  host, extender, DLL, overlay, or runtime file.
- Record the OpenWatcom version used to build the checked-in binary in
  `tools/doors/oxide-door-check/README.md`.

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
line 2: sysop name
line 3: COM port
line 4: baud string
line 5: reserved/zero
line 6: caller first name
line 7: caller last name
line 8: caller location
line 9: caller security level
line 10: caller minutes remaining
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

- `build.sh` produces `dist/OXIDECHK.EXE`.
- `dist/OXIDECHK.EXE.sha256` validates the checked-in binary.
- The binary is small enough to remain reviewable as a generated artifact.
  There is no hard size limit, but it should remain small enough that reviewers
  can reason about the fixture.
- The source contains comments for non-obvious DOS/OpenWatcom compatibility
  choices.
- A local DOSBox run can show the prompt and exit with `Q`.

## Phase 3 - DOSBox Runtime Contract

Status: `TODO`

### Objective

Fix the DOSBox launch plan so generated drop files are visible to DOS programs
in a predictable current directory. This phase is required before the test door
can be considered useful.

### Current Behavior To Replace

At the time this plan was written, `oxidebbs-door` writes the drop file to the
node runtime directory, then builds a DOSBox plan that mounts the door
`working_dir` as `C:` and switches to `C:` before running the command.

That makes the drop file invisible to a normal DOS program that expects
`DORINFO1.DEF` or `DOOR.SYS` in the current directory.

### Required Runtime Contract

For DOSBox runs:

- Host door working directory is mounted as DOS drive `C:`.
- Host node runtime directory is mounted as DOS drive `D:`.
- DOS drive `D:` is the current DOS directory when the door command runs.
- Drop files are written into the host node runtime directory.
- `OXNODE.TXT` is written into the host node runtime directory beside the drop
  file.
- Door-created per-run files, including `OXIDECHK.RPT`, are written into the
  host node runtime directory.
- The configured command is invoked from the current `D:` directory, but bare
  executable names are resolved from `C:`.

The generated DOSBox command sequence must be:

```text
-c "mount c <door working dir>"
-c "mount d <node runtime dir>"
-c "d:"
-c "<resolved door command>"
-c "exit"
```

For the Oxide test door example:

```toml
command = "OXIDECHK.EXE"
```

the resolved DOS command must be:

```text
C:\OXIDECHK.EXE
```

### Command Resolution Rules

Implement a small helper in `oxidebbs-door`; suggested name:

```rust
fn resolve_dosbox_command(command: &str) -> String
```

Rules:

1. Trim leading and trailing ASCII whitespace.
2. If the command is empty, validation must reject the door config before plan
   generation.
3. Split the command into the first token and the remaining argument string at
   the first ASCII whitespace character.
4. If the first token contains `:` or `\` or `/`, leave the first token
   unchanged.
5. Otherwise, prefix the first token with `C:\`.
6. Append the remaining argument string unchanged after one space if arguments
   exist.

Examples:

| Configured Command | Resolved Command |
| --- | --- |
| `OXIDECHK.EXE` | `C:\OXIDECHK.EXE` |
| `LORD.EXE /N1` | `C:\LORD.EXE /N1` |
| `C:\LORD\START.BAT` | `C:\LORD\START.BAT` |
| `UTILS\DOOR.EXE` | `UTILS\DOOR.EXE` |

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

The meaning of `working_dir` for DOSBox plans must be updated to the host node
runtime directory, because that is the directory where the child process should
start from the host perspective. If DOSBox itself does not require a host
current directory, using the runtime directory is still the least surprising
choice because all generated run files live there.

`prepare_door_run` must continue to write the drop file before constructing the
plan.

`dosbox_plan` must use `request.runtime_dir` in its mount commands. If the
current function signature does not include enough data, change it rather than
deriving paths from `drop_file_path`.

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

Validation of the configured command must use the first token before DOSBox
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
- DOSBox plans mount both the door working directory and node runtime
  directory.
- DOSBox plans switch to `D:` before running the command.
- Bare commands are prefixed with `C:\`.
- Existing dry-run behavior still writes drop files and does not launch DOSBox.
- Existing live door bridge behavior still tracks start/finish/timeout records.

## Phase 4 - Config And Sysop CLI Integration

Status: `TODO`

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
runner = "dosbox"
working_dir = "./tools/doors/oxide-door-check/dist"
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
runner = "dosbox"
working_dir = "./tools/doors/oxide-door-check/dist"
command = "OXIDECHK.EXE"
drop_file = "DORINFO1.DEF"
exclusive = false
time_limit_minutes = 5
enabled = false
```

The main example config must keep the test door disabled by default. This keeps
fresh configuration validation from requiring DOSBox to be installed before the
operator intentionally enables door testing. The dedicated `doors.example.toml`
may keep it enabled because that file is specifically for door setup examples.

### Setup Flow

Review:

```text
crates/oxidebbs-server/src/setup.rs
```

If setup currently generates a fictional or third-party placeholder door,
replace it with the disabled `oxide-check` definition above. Setup-generated
configs should not require DOSBox for a clean first `check` unless the sysop
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

Live execution remains:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors test oxide-check --user sysop
```

That command may fail with a clear missing-runner error when DOSBox is not
installed. That is acceptable. The failure must not look like an OxideBBS
internal error.

### Acceptance Criteria

- Example configs reference only the Oxide-owned test door.
- The main example config does not require DOSBox for default setup.
- The dedicated doors example is ready to copy into a test setup.
- Existing sysop CLI commands can list, check, generate drop files, dry-run,
  and live-run the test door.

## Phase 5 - Testing Automation

Status: `TODO`

### Objective

Add deterministic automated tests for the OxideBBS side of the contract, and
add an optional smoke test for hosts with DOSBox installed.

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
- `dosbox_plan` mounts the door working directory as `C:`.
- `dosbox_plan` mounts the node runtime directory as `D:`.
- `dosbox_plan` switches to `D:`.
- `dosbox_plan` resolves `OXIDECHK.EXE` to `C:\OXIDECHK.EXE`.
- `resolve_dosbox_command("LORD.EXE /N1")` returns `C:\LORD.EXE /N1`.
- commands with a drive or path are not prefixed.
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
- `doors test oxide-check --dry-run` succeeds without DOSBox.

If CLI integration tests are not already present for door commands, do not
create a large new test harness solely for this phase. Cover the behavior at
the `oxidebbs-door` and server command-helper level instead.

### Optional DOSBox Smoke Script

Add:

```text
scripts/test-oxide-door-dosbox.sh
```

The script must:

- Use `#!/usr/bin/env bash`.
- Use `set -euo pipefail`.
- Resolve paths relative to the repository root.
- Check for `dosbox` on `PATH`.
- If DOSBox is missing, print:

  ```text
  SKIP: dosbox not found
  ```

  and exit `77`.

- Verify that `tools/doors/oxide-door-check/dist/OXIDECHK.EXE` exists.
- Create a temporary runtime directory under `target/`.
- Create a representative `DORINFO1.DEF` in that runtime directory.
- Create a representative `OXNODE.TXT` in that runtime directory.
- Launch DOSBox using the same mount/current-directory pattern required in
  Phase 3.
- Use DOSBox commands that run the door in the most automated way practical.

If fully automated key input is not reliable across DOSBox versions, the script
may be interactive. In that case it must clearly print:

```text
Press I to view node info, R to write a report, then Q to return.
```

and must still be optional, not part of `./scripts/dev-check.sh`.

### CI Contract

Do not add the DOSBox smoke script to mandatory CI in this phase.

It is acceptable to add a future optional GitHub Actions job that installs
DOSBox and runs the script, but that job must be allowed to skip cleanly when
the environment cannot support DOSBox. This plan does not require that optional
job.

### Acceptance Criteria

- Mandatory Rust tests pass without DOSBox installed.
- Mandatory Rust tests pass without OpenWatcom installed.
- The optional DOSBox script skips with exit `77` when DOSBox is missing.
- On a developer machine with DOSBox, the optional script can run the checked-in
  `OXIDECHK.EXE`.

## Phase 6 - Documentation And Changelog

Status: `TODO`

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
- Document that the DOSBox runner mounts the door working directory as `C:` and
  the node runtime directory as `D:`.
- Document that DOS doors are launched with `D:` as the current directory so
  drop files are visible.
- Document that bare configured commands are resolved from `C:`.
- Document that OxideBBS writes `OXNODE.TXT` as Oxide-owned per-node metadata
  for diagnostics, and that third-party doors are not expected to consume it.
- Keep the legal note: no copyrighted or abandonware DOS doors are bundled.

Update:

```text
design/SPEC.md
```

Required additions:

- State that v1 includes a redistributable DOSBox test door.
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
- Explain that live run requires DOSBox.

Update setup/getting-started documentation if present:

```text
docs/project/setup.md
docs/project/getting-started.md
docs/project/deployment.md
```

Only update files that exist. Required content somewhere in operator docs:

- Install DOSBox before live door testing.
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

- bundled Oxide-owned DOSBox test door
- DOSBox launch-plan correction that makes drop files visible in the DOS
  current directory
- optional DOSBox smoke test script, if implemented

Follow `design/VERSIONING_GUIDE.md`. Do not bump crate versions unless this
work is being combined with an explicit release-version change.

### Acceptance Criteria

- Design docs and operator docs agree on the DOSBox mount/current-directory
  contract.
- Changelog mentions the user-visible behavior change.
- Documentation does not imply that OxideBBS ships third-party DOS games.
- Documentation commands use actual repo paths and command names.
- `npm run docs:build` passes.

## Phase 7 - Final Validation

Status: `TODO`

### Objective

Finish the branch with a clean validation record and no hidden follow-up tasks
inside the implementation phases.

### Required Commands

Run:

```bash
./scripts/dev-check.sh
npm run docs:build
git diff --check
```

If OpenWatcom is installed, also run:

```bash
./tools/doors/oxide-door-check/build.sh
cd tools/doors/oxide-door-check/dist
sha256sum -c OXIDECHK.EXE.sha256
```

If DOSBox is installed, also run:

```bash
./scripts/test-oxide-door-dosbox.sh
```

If OpenWatcom or DOSBox are missing, do not treat that as a failure. Record the
skip in the final implementation notes.

### Final Review Checklist

Before marking this plan complete:

- [ ] All phase-map statuses are updated.
- [ ] `tools/doors/oxide-door-check/dist/OXIDECHK.EXE` exists.
- [ ] `tools/doors/oxide-door-check/dist/OXIDECHK.EXE.sha256` validates.
- [ ] `config/doors.example.toml` references `oxide-check`.
- [ ] `config/oxidebbs.example.toml` references disabled `oxide-check`.
- [ ] The DOSBox plan mounts both `C:` and `D:`.
- [ ] The DOSBox plan switches to `D:` before running the command.
- [ ] The drop file path points into the node runtime directory.
- [ ] Door command validation handles command arguments.
- [ ] Mandatory tests do not require DOSBox.
- [ ] Mandatory tests do not require OpenWatcom.
- [ ] Optional DOSBox smoke testing is documented.
- [ ] Operator docs explain how to enable and run the test door.
- [ ] `docs/about/changelog.md` is updated.
- [ ] `design/TASKS.md` is updated.
- [ ] `./scripts/dev-check.sh` passes.
- [ ] `npm run docs:build` passes.
- [ ] `git diff --check` passes.

## Implementation Notes Template

When implementation begins, add notes here instead of leaving decisions in chat
history.

```text
Implementation started:
Implementation completed:
OpenWatcom validation:
DOSBox smoke validation:
Skipped validations:
Notable decisions:
```
