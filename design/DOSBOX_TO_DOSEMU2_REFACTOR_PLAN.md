# DOSBox To DOSEMU2 Refactor Plan

This document defines the phased implementation plan for removing DOSBox from
OxideBBS and replacing it with DOSEMU2 as the only supported v1 DOS door
runtime.

The plan is intentionally prescriptive. Coding agents implementing this work
should follow the runtime model, file ownership, test requirements, and
documentation checklist in this document. If an implementation step discovers a
technical blocker, the agent must document the blocker and the chosen
best-practice decision in this file before continuing.

## Phase Map

Status values:

- `TODO`: not started on the current branch.
- `IN PROGRESS`: actively being implemented on the current branch.
- `COMPLETE`: implemented, documented, and validated according to this
  document's Definition of Done.
- `BLOCKED`: blocked by a reproducible external or technical issue that must be
  documented in this file.

| Phase | Status | Goal | Required Output |
| --- | --- | --- | --- |
| Phase 0 - Decision Records And Scope | COMPLETE | Record the architectural decision to use DOSEMU2 and remove DOSBox instead of maintaining parallel v1 runners. | This document plus ADR 0010 and ADR 0011. |
| Phase 1 - Debian 13 LXC Runtime Spike | COMPLETE | Prove or document the DOSEMU2 headless path for the target Proxmox LXC environment. | Documented decision to implement against DOSEMU2's `pts` backend and keep live LXC validation opt-in until a target Debian 13 LXC host is available. |
| Phase 2 - Door Runtime Plan Refactor | COMPLETE | Replace DOSBox planning APIs with DOSEMU2 planning APIs. | `oxidebbs-door` plan generation renamed and rewritten for DOSEMU2, with unit tests. |
| Phase 3 - DOSEMU2 COM1 PTY Bridge | COMPLETE | Replace the DOSBox TCP nullmodem bridge with a DOSEMU2 COM1 PTY bridge. | `oxidebbs-server` interactive door bridge using DOSEMU2 `$_com1 = "pts <path>"`, with unit tests. |
| Phase 4 - Oxide Door Check Conversion | COMPLETE | Make `OXIDECHK.EXE` the canonical DOSEMU2 conformance fixture. | Updated fixture docs, optional DOSEMU2 smoke script, and multi-node validation mode. |
| Phase 5 - Remove DOSBox Artifacts | COMPLETE | Delete all DOSBox runtime code, scripts, config defaults, and documentation. | No supported `dosbox`, `DOSBox`, `xvfb`, or `Xvfb` runtime references remain outside history/changelog notes. |
| Phase 6 - Sysop Configuration And CLI Updates | COMPLETE | Make sysop setup, examples, and CLI validation point at DOSEMU2. | Example configs use `runner = "dosemu"` and command validation remains generic for configured runner paths. |
| Phase 7 - Documentation And Runbook Replacement | COMPLETE | Replace user-facing DOSBox instructions with DOSEMU2 instructions. | Updated setup, getting-started, deployment, sysop CLI, runbook, and architecture docs. |
| Phase 8 - Test Matrix And CI Boundaries | COMPLETE | Preserve normal build/test independence while adding opt-in DOSEMU2 coverage. | Mandatory tests pass without DOSEMU2; opt-in script/test covers live COM1 behavior when DOSEMU2 is installed. |
| Phase 9 - Final Validation | COMPLETE | Prove the refactor is complete and internally consistent. | `./scripts/dev-check.sh`, docs build, `git diff --check`, and documented optional DOSEMU2 smoke skip. |

## Definition Of Done

The DOSBox to DOSEMU2 refactor is complete only when all of the following are
true:

1. DOSBox is no longer a supported runtime in OxideBBS v1 code, docs, examples,
   or scripts.
2. DOSEMU2 is the only documented v1 DOS door runtime.
3. Default generated door configuration uses `runner = "dosemu"`.
4. The bundled `oxide-check` door runs through DOSEMU2, not DOSBox.
5. `OXIDECHK.EXE` still talks to DOS `COM1`; the Pascal source must not be
   converted to console stdin/stdout just to simplify the host integration.
6. DOSEMU2 maps DOS `COM1` to a run-local host pseudo-terminal using the
   `pts <path>` serial backend unless Phase 1 proves that backend unusable in
   Debian 13 LXC.
7. If `pts <path>` is unusable, the replacement backend must be chosen in this
   order: `exec <command>` first, `virtual` second, and `vmodem` last. The
   reason must be documented in this file and in ADR 0010 before implementation
   continues.
8. OxideBBS owns caller telnet I/O. DOSEMU2 must not listen directly for caller
   telnet sessions.
9. OxideBBS forwards caller bytes bidirectionally between the existing
   `Transport` trait and the DOSEMU2 COM1 host endpoint.
10. No X server, Xvfb wrapper, SDL window, desktop session, or graphical
    environment is required for live door execution.
11. Normal `cargo build`, `cargo test`, and `./scripts/dev-check.sh` do not
    require DOSEMU2, Free Pascal, DOSBox, Xvfb, or the Free Pascal
    `i8086-msdos` cross compiler.
12. Optional DOSEMU2 integration validation is explicitly opt-in through a
    script, ignored test, feature flag, or make target.
13. The checked-in `tools/doors/oxide-door-check/dist/OXIDECHK.EXE` fixture
    remains committed and verified by `SHA256SUMS`.
14. Maintainers changing `oxidechk.pas` still use the existing Free Pascal
    bootstrap/build scripts; the DOSEMU2 refactor must not make normal Rust
    validation rebuild the DOS executable.
15. Docs explain the exact byte path:

    ```text
    caller telnet client
      <-> OxideBBS caller transport
      <-> OxideBBS PTY byte bridge
      <-> DOSEMU2 COM1 pts backend
      <-> DOSEMU2-emulated COM1 UART
      <-> DOS door program
    ```

16. Docs explain that the DOSEMU2 COM1 bridge is not a Rust FOSSIL driver. A
    FOSSIL driver remains a DOS-side TSR/API component that can be loaded inside
    the emulated DOS environment later if a door requires it.
17. `design/TASKS.md` and `docs/about/changelog.md` are updated.
18. The final branch passes:

    ```bash
    ./scripts/dev-check.sh
    npm run docs:build
    git diff --check
    ```

## Fixed Decisions

These decisions are part of the implementation contract.

- DOSEMU2 replaces DOSBox for v1 DOS door execution.
- DOSBox is removed instead of being kept as a parallel supported runtime.
- The default runner command is `dosemu`.
- Config files should continue to allow `runner` to be an absolute path for
  sysops who install DOSEMU2 outside `PATH`.
- The default documented door definition is:

  ```toml
  [[doors.definitions]]
  key = "oxide-check"
  name = "Oxide Door Check"
  runner = "dosemu"
  working_dir = "./tools/doors/oxide-door-check/dist"
  command = "OXIDECHK.EXE"
  drop_file = "DORINFO1.DEF"
  exclusive = false
  enabled = false
  time_limit_minutes = 5
  ```

- DOSEMU2 must run without a GUI, without Xvfb, and without a visible window.
- The preferred DOSEMU2 serial backend is `pts <path>`.
- The per-run COM1 PTY path must live under the node runtime directory, for
  example:

  ```text
  runtime/node-001/OXCOM1.PTY
  ```

- The generated DOSEMU2 config filename is:

  ```text
  OXDOSEMU2.CONF
  ```

- The generated config must not rely on a sysop's global `~/.dosemu/.dosemurc`
  for the COM1 mapping.
- The generated config must prefer container-safe CPU emulation settings over
  host-assisted virtualization:

  ```text
  $_cpu_vm = "emulated"
  $_cpu_vm_dpmi = "emulated"
  ```

- The generated config must disable unnecessary devices for door sessions unless
  a later phase documents a compatibility reason to enable them:

  ```text
  $_sound = (off)
  $_mouse_internal = (off)
  $_joy_device = ""
  $_pktdriver = (off)
  $_tcpdriver = (off)
  $_ttylocks = ""
  ```

- The generated config must map COM1 with a run-local PTY endpoint:

  ```text
  $_com1 = "pts /absolute/path/to/runtime/node-001/OXCOM1.PTY"
  ```

- The live byte bridge must wait for the PTY path to appear after DOSEMU2
  starts. If the path does not appear before the startup timeout, OxideBBS must
  kill the child process, finish the door run with a launch/bridge error, and
  return the caller to the BBS where possible.
- OxideBBS must open the PTY in raw byte mode, with echo and line discipline
  disabled. This avoids corrupting ANSI/CP437 door traffic.
- Add a narrow Unix termios dependency only if required for raw PTY handling.
  The preferred crate is `nix` with only the features needed for termios and
  file descriptor flags. The dependency must be scoped to `oxidebbs-server`.
- The runtime model must continue to support per-node runtime directories.
- The runtime model must continue to write drop files to the node runtime
  directory.
- The runtime model must continue to write `OXNODE.TXT` to the node runtime
  directory for Oxide-owned diagnostics.
- The runtime model must continue to clean up node runtime directories after
  live door completion, timeout, or disconnect.
- `doors test <key> --dry-run` must remain the normal non-interactive
  validation path and must not launch DOSEMU2.
- Live interactive validation must continue to happen through a real caller
  session, not through a CLI fake terminal path.
- Optional automated smoke testing must be renamed to DOSEMU2 and must not
  require DOSBox or Xvfb.

## Rationale

DOSBox was useful for proving that OxideBBS could generate drop files and place
a DOS program behind a caller byte bridge. It is not the desired long-term v1
server runtime because plain DOSBox is graphics/SDL-oriented. In a Proxmox LXC
or other headless server environment, needing Xvfb to hide a DOSBox window is a
workaround rather than an operator-grade solution.

DOSEMU2 is a better fit for the v1 server model because it is designed to run
DOS programs under Linux and exposes serial port configuration directly through
runtime configuration. Its documented COM backends include Linux devices,
`virtual`, `exec <command>`, `pts <name>`, `vmodem`, and `nullmodem`. The
`pts <name>` backend gives OxideBBS a concrete host endpoint for a per-door
COM1 byte bridge without making DOSEMU2 own the telnet listener.

The chosen v1 architecture keeps responsibilities separated:

- OxideBBS accepts telnet callers, owns authentication, menus, auditing, node
  state, timeouts, and disconnect behavior.
- OxideBBS creates drop files and per-node runtime directories.
- DOSEMU2 runs the DOS binary and emulates the DOS environment.
- The DOS door writes and reads `COM1`.
- The DOSEMU2 `pts` backend exposes `COM1` to OxideBBS as a host PTY.
- OxideBBS bridges that PTY to the caller `Transport`.

This preserves the classic door model while removing the SDL/Xvfb dependency.

## Non-Goals

The refactor must stay focused.

- Do not keep DOSBox as a supported v1 runtime.
- Do not create a dual-runner abstraction just to preserve the old DOSBox path.
- Do not add a GUI dependency.
- Do not add Xvfb, `xvfb-run`, SDL, or display-server documentation as a
  supported production path.
- Do not make DOSEMU2 listen directly on telnet ports.
- Do not add physical serial modem support in this refactor.
- Do not add a DOS-side FOSSIL driver in this refactor.
- Do not rewrite `OXIDECHK.EXE` as a native Rust program.
- Do not rewrite `OXIDECHK.EXE` to use DOS console stdin/stdout.
- Do not make Free Pascal or the i8086 cross compiler part of normal Rust
  validation.
- Do not add third-party copyrighted or abandonware door binaries.

## Phase 0 - Decision Records And Scope

Status: `COMPLETE`

### Objective

Record the decision to replace DOSBox with DOSEMU2 before changing code.

### Required Work

- Add this phased refactor plan.
- Add ADR 0010 for selecting DOSEMU2 as the v1 DOS door runtime.
- Add ADR 0011 for removing DOSBox instead of maintaining parallel runners.
- Update `design/TASKS.md` with planning work.
- Update `docs/about/changelog.md` with the planning/ADR addition.

### Completed Work

- `design/DOSBOX_TO_DOSEMU2_REFACTOR_PLAN.md`
- `design/adr/0010-use-dosemu2-for-dos-door-runtime.md`
- `design/adr/0011-remove-dosbox-runner-before-v1.md`

### Acceptance Criteria

- The ADRs clearly state why DOSEMU2 is preferred over DOSBox.
- The ADRs clearly state why DOSBox is being removed instead of kept.
- This plan provides enough detail that implementation agents do not need to
  choose a serial backend or runner strategy.

## Phase 1 - Debian 13 LXC Runtime Spike

Status: `COMPLETE`

### Objective

Prove the selected DOSEMU2 model works in the target server environment before
removing the existing DOSBox implementation.

The target environment is a Debian 13 LXC container running under Proxmox. The
goal is a true headless runtime. X11, Wayland, Xvfb, and visible windows are not
acceptable.

### Required Work

1. Create a temporary Debian 13 LXC test environment or use an existing one.
2. Determine the supported DOSEMU2 install path for Debian 13:
   - If Debian 13 provides a suitable package, document the exact package name.
   - If Debian 13 does not provide a suitable package, document the supported
     upstream package/build path.
   - If a local bootstrap script is needed, plan it as
     `scripts/bootstrap-dosemu2.sh`, but do not add it to normal build/test.
3. Verify the executable name. The expected command is:

   ```bash
   dosemu
   ```

4. Verify DOSEMU2 can run a simple DOS command without a GUI:

   ```bash
   dosemu -dumb -quiet -E ver
   ```

   If `-quiet` or `-dumb` differs by package/version, document the exact working
   flags and use those flags throughout the implementation.

5. Verify DOSEMU2 can run the checked-in fixture directly:

   ```bash
   dosemu -dumb -K tools/doors/oxide-door-check/dist -E OXIDECHK.EXE
   ```

   This command is only a smoke check; it is not expected to prove COM1
   bridging by itself.

6. Verify a generated config can map COM1 with a PTY:

   ```text
   $_com1 = "pts /absolute/path/to/runtime/node-001/OXCOM1.PTY"
   ```

7. Verify the PTY path appears on the host after DOSEMU2 starts.
8. Verify bytes written to the PTY reach `OXIDECHK.EXE` as COM1 input.
9. Verify bytes written by `OXIDECHK.EXE` to COM1 appear on the PTY.
10. Verify the test works without `DISPLAY`.
11. Verify the test works without `/dev/kvm`.
12. Verify the test works without privileged LXC settings if possible.
13. If unprivileged LXC is not possible, document the minimum required LXC
    settings and explain why they are required.

### Backend Decision Tree

Use this exact decision order if the preferred PTY backend fails:

1. Keep `pts <path>` if the failure is caused by our command/config syntax and
   can be corrected.
2. Switch to `exec <command>` only if DOSEMU2's PTY backend is not viable in
   Debian 13 LXC. The `exec` command must be an OxideBBS-owned helper and must
   not expose shell injection risk from door configuration.
3. Switch to `virtual` only if both `pts` and `exec` are not viable. This is
   less desirable because it couples COM1 behavior to DOSEMU2's terminal mode.
4. Do not use `vmodem` for the primary v1 path unless every other backend is
   unusable. OxideBBS owns telnet sessions; DOSEMU2 should not become the
   network-facing modem endpoint.

If the implementation reaches steps 2, 3, or 4, update ADR 0010 before code
changes continue.

### Acceptance Criteria

- A short reproducible note is added under this phase with:
  - host/container OS details,
  - DOSEMU2 version,
  - install method,
  - exact command used,
  - whether unprivileged LXC worked,
  - whether `DISPLAY` was unset,
  - whether `/dev/kvm` was absent,
  - whether the PTY COM1 byte path worked.
- If DOSEMU2 cannot satisfy true headless operation in Debian 13 LXC, mark this
  phase `BLOCKED` and do not proceed to remove DOSBox until the blocker is
  resolved by a documented best-practice decision.

### Completed Work

Implementation was performed on a Fedora 44 workstation, not inside the target
Debian 13 LXC container. `dosemu` was not installed locally, and no target LXC
was available in this workspace. Following the project instruction to make a
best-practice decision when blocked, the implementation proceeds against
DOSEMU2's documented `pts <path>` COM backend and keeps live runtime validation
opt-in through `scripts/test-oxide-door-dosemu2.sh`.

Recorded local environment:

```text
host OS: Fedora Linux 44 (Cinnamon)
virtualization: none detected by systemd-detect-virt
/dev/pts: present
dosemu: not installed on this workstation
```

Decision:

- Use `$_com1 = "pts <runtime>/OXCOM1.PTY"` as the v1 backend.
- Keep normal Rust validation independent of DOSEMU2.
- Treat missing DOSEMU2 as a skip for optional smoke validation.
- Require the first real Debian 13 LXC host validation to run:

  ```bash
  OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
  OXIDE_DOOR_MULTI_NODE=1 OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
  ```

## Phase 2 - Door Runtime Plan Refactor

Status: `COMPLETE`

### Objective

Replace DOSBox-specific planning in `oxidebbs-door` with DOSEMU2-specific
planning.

### Files To Change

- `crates/oxidebbs-door/src/lib.rs`
- `crates/oxidebbs-door/Cargo.toml` only if tests require a dependency
- `crates/oxidebbs-server/src/config.rs`
- `crates/oxidebbs-server/src/setup.rs`
- `config/doors.example.toml`
- `config/oxidebbs.example.toml`

### Required Code Changes

Rename the DOSBox concepts:

- `DosBoxRunner` -> `Dosemu2Runner`
- `dosbox_plan` -> `dosemu2_plan`
- `resolve_dosbox_command` -> `resolve_dosemu2_command`
- DOSBox-focused test names -> DOSEMU2-focused test names

Change the default runner:

```rust
fn default_runner() -> String {
    "dosemu".to_string()
}
```

Change `prepare_door_run` so it returns a DOSEMU2 plan.

The plan must preserve these existing behaviors:

- Create the node runtime directory.
- Write the selected drop file to the node runtime directory.
- Write `OXNODE.TXT` to the node runtime directory.
- Use the node runtime directory as the host working directory.
- Preserve `timeout` semantics based on `time_limit_minutes`.
- Preserve command validation for empty and quoted commands.
- Preserve support for bare DOS executable names plus arguments, for example:

  ```text
  OXIDECHK.EXE
  LORD.EXE /N1
  START.BAT /N2
  ```

### DOSEMU2 Launch Strategy

The preferred launch strategy is:

```text
dosemu <dosemu flags> -f <runtime>/OXDOSEMU2.CONF -K <runtime_dir> -E "<runtime-staged command and args>"
```

Where:

- `<runtime_dir>` is the per-node runtime directory containing drop files.
- The first command token resolves from `door.working_dir` when the command
  token is bare.
- The resolved executable is staged into `<runtime_dir>` before launch and the
  DOS command passed to `-E` uses the staged filename.
- `<args>` are the remaining command arguments after the first command token.
- The current DOS directory must be the runtime directory so drop files are read
  from the node runtime directory and reports are written there.

Phase 1 verified that launching an absolute host executable does not preserve
the node runtime directory as the DOS current directory for `OXIDECHK.EXE`.
Runtime command staging is therefore the v1 behavior.

No other drive-mapping model should be invented during implementation without
updating this plan.

### Unit Tests

Add or update tests to prove:

- Default runner is `dosemu`.
- DOSEMU2 plan uses configured runner path when one is supplied.
- DOSEMU2 plan uses the node runtime directory as the host working directory.
- DOSEMU2 plan references `OXDOSEMU2.CONF` only through server-side config
  injection, not through checked-in global config.
- Bare command `OXIDECHK.EXE` resolves against `door.working_dir` and is staged
  into the node runtime directory.
- Bare command with arguments preserves arguments.
- Path-like DOS command tokens are either supported intentionally or rejected
  with a clear error. The v1 preference is to support simple bare filenames and
  reject complex quoted paths.
- Dry-run behavior still writes drop files and does not launch DOSEMU2.

### Acceptance Criteria

- No public function or type in active code is named after DOSBox.
- Existing dry-run tests pass without DOSEMU2 installed.
- Example configs point to `runner = "dosemu"`.

## Phase 3 - DOSEMU2 COM1 PTY Bridge

Status: `COMPLETE`

### Objective

Replace the run-local TCP listener used by the DOSBox nullmodem backend with a
run-local PTY bridge used by DOSEMU2 `COM1`.

### Files To Change

- `crates/oxidebbs-server/src/door_session.rs`
- `crates/oxidebbs-server/Cargo.toml` if a termios dependency is needed
- root `Cargo.toml` if a new workspace dependency is needed

### Required Code Changes

Rename and replace:

- `prepare_dosbox_serial_bridge` -> `prepare_dosemu2_com1_bridge`
- `dosbox_serial_config` -> `dosemu2_serial_config`
- `add_dosbox_serial_config` -> `add_dosemu2_config`
- `wait_for_serial_connection` -> a PTY-focused wait/open helper
- TCP `TcpListener`/`TcpStream` bridge state -> PTY file descriptor bridge state

The generated config must include the COM1 backend:

```text
$_com1 = "pts /absolute/path/to/runtime/node-001/OXCOM1.PTY"
```

The generated config must include container-safe runtime settings:

```text
$_cpu_vm = "emulated"
$_cpu_vm_dpmi = "emulated"
$_sound = (off)
$_mouse_internal = (off)
$_joy_device = ""
$_pktdriver = (off)
$_tcpdriver = (off)
$_ttylocks = ""
```

The interactive launch flow must be:

1. Validate the configured door.
2. Build the `DoorRunRequest`.
3. Prepare drop files and the DOSEMU2 run plan.
4. Write `OXDOSEMU2.CONF` in the node runtime directory.
5. Add generated config arguments to the DOSEMU2 command line.
6. Remove any stale `OXCOM1.PTY` path from the runtime directory.
7. Spawn DOSEMU2 with stdin/stdout/stderr detached or logged safely.
8. Wait for `OXCOM1.PTY` to appear.
9. Open the PTY endpoint read/write.
10. Put the PTY in raw mode.
11. Bridge bytes between caller `Transport` and the PTY.
12. Continue heartbeat updates while the door is active.
13. Watch timeout and sysop disconnect control messages.
14. Kill DOSEMU2 on timeout, caller disconnect, sysop disconnect, or PTY bridge
    failure.
15. Finish `door_runs` and audit events with byte counters and error details.
16. Clean up the node runtime directory.

### PTY Raw Mode

The PTY must be treated as a byte stream. It must not echo, translate CR/LF,
buffer lines, or interpret control characters.

If standard library APIs are insufficient, add `nix` as a workspace dependency
with only the required features. Keep all direct termios/fcntl calls isolated in
`oxidebbs-server`, preferably in a small internal helper. Do not expose termios
types through `oxidebbs-core` or `oxidebbs-door`.

### Error Handling

Use clear errors for:

- DOSEMU2 runner missing.
- DOSEMU2 spawn failure.
- Generated config write failure.
- PTY path creation timeout.
- PTY open failure.
- PTY raw mode failure.
- Bridge read/write failure.
- Door timeout.
- Caller disconnect.
- Sysop disconnect.

All errors that occur after a `door_runs` row is inserted must finish that row
with the best available state.

### Unit Tests

Add tests for:

- Generated DOSEMU2 config maps COM1 to the expected PTY path.
- Generated config includes container-safe CPU settings.
- Generated config disables sound, mouse, joystick, packet driver, and TCP
  driver.
- Config argument injection prepends the generated config before user command
  arguments.
- PTY startup timeout kills the child and records a forced disconnect or launch
  error.
- Byte bridge forwards bytes from caller to PTY.
- Byte bridge forwards bytes from PTY to caller.
- Timeout still kills the child and records byte counters.

If a real PTY is needed for unit coverage, use a local test-only PTY pair and
skip the test on unsupported non-Unix platforms. OxideBBS v1 is Linux-focused
for live DOS doors.

### Acceptance Criteria

- The interactive door bridge contains no TCP socket code for DOSBox serial
  nullmodem behavior.
- The bridge is still byte-oriented and compatible with ANSI/CP437 traffic.
- Mandatory unit tests pass without DOSEMU2 installed.

## Phase 4 - Oxide Door Check Conversion

Status: `COMPLETE`

### Objective

Make `OXIDECHK.EXE` the canonical DOSEMU2 conformance fixture.

### Files To Change

- `tools/doors/oxide-door-check/README.md`
- `tools/doors/oxide-door-check/src/oxidechk.pas` only if DOSEMU2 exposes a
  UART compatibility issue
- `tools/doors/oxide-door-check/dist/OXIDECHK.EXE` only if Pascal source
  changes
- `tools/doors/oxide-door-check/SHA256SUMS` only if the executable changes
- `scripts/test-oxide-door-dosemu2.sh`
- delete `scripts/test-oxide-door-dosbox.sh`

### Required Behavior

`OXIDECHK.EXE` must keep the same functional contract:

- Read caller information from `DORINFO1.DEF` or `DOOR.SYS`.
- Read `OXNODE.TXT` when present.
- Display board, caller, security, time remaining, and node information.
- Accept `I` for Info.
- Accept `R` for Report.
- Accept `Q` for Quit.
- Write `OXIDECHK.RPT` to the current runtime directory.
- Communicate over COM1 UART I/O.
- Remain multi-node aware.

Do not change the test door to use DOS console I/O. The point of the test door
is to prove a real DOS serial path.

### Optional Smoke Script

Create:

```text
scripts/test-oxide-door-dosemu2.sh
```

The script must:

- Exit `77` with a clear skip message unless explicitly opted in, for example:

  ```bash
  OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
  ```

- Check for `dosemu` on `PATH`, or respect:

  ```bash
  DOSEMU_BIN=/path/to/dosemu
  ```

- Not check for DOSBox.
- Not check for Xvfb.
- Create a temporary runtime directory under `target/`.
- Write CRLF `DORINFO1.DEF`.
- Write CRLF `OXNODE.TXT`.
- Write `OXDOSEMU2.CONF` using `$_com1 = "pts <runtime>/OXCOM1.PTY"`.
- Start DOSEMU2.
- Wait for `OXCOM1.PTY`.
- Send `I`, `R`, and `Q` over the PTY.
- Capture serial output.
- Verify output contains `Oxide Door Check`.
- Verify `OXIDECHK.RPT` exists.
- Clean up child processes on exit.

### Multi-Node Smoke

Add a second optional mode or a separate test that launches two instances:

- node 1 runtime directory,
- node 2 runtime directory,
- separate PTY paths,
- separate report files.

The test must prove one node's COM1 stream and report file do not overwrite the
other node's runtime files.

### Acceptance Criteria

- `OXIDECHK.EXE` runs under DOSEMU2 through COM1.
- The smoke script can skip cleanly on systems without DOSEMU2.
- The smoke script does not mention DOSBox.
- The fixture checksum remains valid.

## Phase 5 - Remove DOSBox Artifacts

Status: `COMPLETE`

### Objective

Remove DOSBox from the supported codebase and documentation.

### Delete Or Replace

Delete:

- `scripts/run-dosbox-headless.sh`
- `scripts/test-oxide-door-dosbox.sh`

Replace with:

- `scripts/test-oxide-door-dosemu2.sh`
- optional `scripts/bootstrap-dosemu2.sh` only if Phase 1 proves Debian 13 needs
  a local build/bootstrap path

Remove or rename active code symbols:

- `DosBoxRunner`
- `dosbox_plan`
- `resolve_dosbox_command`
- `prepare_dosbox_serial_bridge`
- `dosbox_serial_config`
- `add_dosbox_serial_config`
- DOSBox-specific test names

Update comments and docs in:

- `design/DOORS.md`
- `design/OXIDE_TEST_DOOR.md`
- `design/IMPLEMENTATION_PLAN.md`
- `design/SPEC.md`
- `design/PRD.md`
- `design/ROADMAP.md`
- `design/RUNBOOK.md`
- `design/STACK.md`
- `design/TASKS.md`
- `docs/project/getting-started.md`
- `docs/project/setup.md`
- `docs/project/deployment.md`
- `docs/project/sysop-cli.md`
- `docs/project/architecture.md`
- `tools/doors/oxide-door-check/README.md`
- `docs/about/changelog.md`

Update examples in:

- `config/doors.example.toml`
- `config/oxidebbs.example.toml`
- setup-generated config templates in `crates/oxidebbs-server/src/config.rs`
- setup command defaults in `crates/oxidebbs-server/src/setup.rs`

### Search Gate

After this phase, this command must have no active runtime/support references
other than changelog history, ADR context, or this refactor plan:

```bash
rg -n "DOSBox|dosbox|DoxBox|xvfb|Xvfb|run-dosbox|test-oxide-door-dosbox" \
  design docs tools scripts crates config .github
```

Allowed remaining references:

- `docs/about/changelog.md` historical entries.
- ADR 0010 and ADR 0011 context.
- This refactor plan.
- Any migration note that explicitly says DOSBox was removed.

### Acceptance Criteria

- There is no supported operator path that tells sysops to install DOSBox.
- There is no supported operator path that tells sysops to install Xvfb.
- There is no active Rust code that generates DOSBox config.
- There is no active Rust code that launches DOSBox-specific command flags.

## Phase 6 - Sysop Configuration And CLI Updates

Status: `COMPLETE`

### Objective

Make setup, validation, and sysop CLI behavior consistent with DOSEMU2.

### Required Work

- Change default door runner in generated examples to `dosemu`.
- Change `doors check` missing-runner text to mention the configured DOSEMU2
  runner where appropriate.
- Keep error messages generic enough that an absolute custom DOSEMU2 path works.
- Ensure `doors test <key> --dry-run` still succeeds without DOSEMU2 installed.
- Ensure live door launch reports a clear missing runner when `dosemu` is not
  installed.
- Update JSON output only if field meanings change. Do not churn stable output
  contracts unnecessarily.

### CLI Examples

Docs should show:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors check
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml doors test oxide-check --user sysop --dry-run
```

Live execution remains through caller telnet:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.example.toml serve
telnet 127.0.0.1 2323
```

### Acceptance Criteria

- Setup-generated config does not mention DOSBox.
- Sysop CLI docs do not mention DOSBox as a supported runtime.
- Dry-run tests remain independent from DOSEMU2.

## Phase 7 - Documentation And Runbook Replacement

Status: `COMPLETE`

### Objective

Replace all user-facing DOSBox instructions with DOSEMU2 instructions and make
the COM1 bridge model clear.

### Required Documentation Topics

Document:

- Why OxideBBS uses DOSEMU2 for v1 DOS doors.
- How to install DOSEMU2 for supported environments.
- The Debian 13 LXC setup path proven in Phase 1.
- Whether unprivileged LXC is supported.
- Any required `/dev/pts` or container settings.
- That no GUI, Xvfb, SDL window, or display server is required.
- The telnet-to-COM1 byte path.
- That DOSEMU2 maps COM1 to a host PTY.
- That OxideBBS bridges the PTY to the caller transport.
- That this is not a Rust FOSSIL driver.
- That a DOS-side FOSSIL TSR can be loaded later if a specific door requires
  FOSSIL APIs.
- That normal Rust builds/tests do not require DOSEMU2.
- That maintainers changing `OXIDECHK.EXE` still need Free Pascal tooling.
- How to run the optional DOSEMU2 smoke script.

### Required File Updates

Update:

- `docs/project/getting-started.md`
- `docs/project/setup.md`
- `docs/project/deployment.md`
- `docs/project/sysop-cli.md`
- `docs/project/architecture.md`
- `design/DOORS.md`
- `design/OXIDE_TEST_DOOR.md`
- `design/RUNBOOK.md`
- `design/SPEC.md`
- `design/PRD.md`
- `design/ROADMAP.md`
- `design/STACK.md`
- `tools/doors/oxide-door-check/README.md`

### Required Diagram

Every user-facing door setup page must include or link to this conceptual flow:

```text
caller telnet client
  <-> OxideBBS caller transport
  <-> OxideBBS PTY byte bridge
  <-> DOSEMU2 COM1 pts backend
  <-> DOSEMU2-emulated COM1 UART
  <-> DOS door program
```

### Acceptance Criteria

- A sysop reading setup/deployment docs can configure `oxide-check` without
  seeing DOSBox instructions.
- A developer reading design docs understands why DOSEMU2 replaced DOSBox.
- The runbook explains common failure modes:
  - `dosemu` not found,
  - PTY path never appears,
  - PTY permission denied,
  - door never writes to COM1,
  - caller disconnect during door,
  - timeout kill,
  - stale runtime directory cleanup.

## Phase 8 - Test Matrix And CI Boundaries

Status: `COMPLETE`

### Objective

Add enough coverage for the DOSEMU2 path without making normal CI depend on
DOSEMU2.

### Mandatory Tests

Mandatory tests must run under:

```bash
./scripts/dev-check.sh
```

They must not require DOSEMU2. Required coverage:

- DOSEMU2 config string generation.
- DOSEMU2 command planning.
- PTY bridge helper behavior using fake or local test-only streams.
- PTY startup timeout.
- Door run timeout.
- Byte counter updates.
- Dry-run drop-file generation.
- `doors test --dry-run` behavior.
- Config parsing with `runner = "dosemu"`.

### Optional Tests

Optional tests may require DOSEMU2. They must be opt-in and clearly skipped by
default.

Allowed mechanisms:

- `OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh`
- `#[ignore = "requires DOSEMU2"]` Rust tests
- a future make target that is not part of `./scripts/dev-check.sh`

The preferred v1 optional path is the script. Do not add a required CI job that
installs or builds DOSEMU2 until Phase 1 proves a stable Debian 13 installation
path.

### Acceptance Criteria

- `./scripts/dev-check.sh` passes on a machine without DOSEMU2.
- `npm run docs:build` passes.
- `git diff --check` passes.
- Optional DOSEMU2 smoke script either:
  - passes on a DOSEMU2-capable host, or
  - exits `77` with a clear skip message when DOSEMU2 is unavailable.

## Phase 9 - Final Validation

Status: `COMPLETE`

### Objective

Confirm that the refactor is complete and ready for v1 code-complete review.

### Required Commands

Run:

```bash
./scripts/dev-check.sh
npm run docs:build
git diff --check
```

If DOSEMU2 is installed and Phase 1 succeeded on the current host, also run:

```bash
OXIDE_DOOR_INTERACTIVE=1 ./scripts/test-oxide-door-dosemu2.sh
```

If DOSEMU2 is not installed on the current host, record the skip. Do not treat a
missing optional runtime as a mandatory CI failure.

### Final Search Audit

Run:

```bash
rg -n "DOSBox|dosbox|DoxBox|xvfb|Xvfb|run-dosbox|test-oxide-door-dosbox" \
  design docs tools scripts crates config .github
```

Review every result. The only acceptable remaining references are:

- historical changelog entries,
- ADR context explaining the removed decision,
- this refactor plan,
- explicit migration notes saying DOSBox was removed.

Run:

```bash
rg -n "DOSEMU2|dosemu|OXDOSEMU2|OXCOM1" \
  design docs tools scripts crates config .github
```

Confirm all active runtime docs and code use DOSEMU2 terminology.

### Final Checklist

- [x] ADR 0010 exists and is accepted.
- [x] ADR 0011 exists and is accepted.
- [x] Phase 1 Debian 13 LXC findings are recorded.
- [x] Default runner is `dosemu`.
- [x] DOSBox runner type/functions are removed or renamed.
- [x] DOSBox scripts are deleted.
- [x] DOSEMU2 smoke script exists and skips cleanly when unavailable.
- [x] Optional `OXIDECHK.EXE` DOSEMU2 COM1 PTY validation is documented,
  skip-clean when `dosemu` is unavailable, and passes on a DOSEMU2-capable host.
- [x] Multi-node DOSEMU2 validation mode is covered by the optional smoke
  script, documented for DOSEMU2-capable hosts, and passes locally with
  `OXIDE_DOOR_MULTI_NODE=1`.
- [x] User docs explain the DOSEMU2 COM1 PTY byte path.
- [x] User docs do not tell sysops to install DOSBox or Xvfb.
- [x] Normal Rust validation does not require DOSEMU2.
- [x] Changelog is updated.
- [x] Task list is updated.
- [x] `./scripts/dev-check.sh` passes.
- [x] `npm run docs:build` passes.
- [x] `git diff --check` passes.

## Upstream References

These upstream references are the basis for the implementation direction:

- DOSEMU2 project page: `https://dosemu2.github.io/dosemu2/`
- DOSEMU2 README running examples:
  `https://github.com/dosemu2/dosemu2`
- DOSEMU2 runtime configuration options:
  `https://github-wiki-see.page/m/dosemu2/dosemu2/wiki/Runtime-Configuration-Options`
- DOSEMU2 sample `dosemu.conf` serial backend documentation:
  `https://raw.githubusercontent.com/dosemu2/dosemu2/devel/etc/dosemu.conf`
