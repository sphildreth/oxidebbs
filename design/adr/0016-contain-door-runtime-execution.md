# ADR 0016: Contain Door Runtime Execution

## Status

Accepted

## Context

OxideBBS runs DOS doors through a configured runner, currently DOSEMU2. Door
configuration controls the runner name, working directory, command, drop-file
format, time limit, and enabled state.

Door execution is inherently a wide surface area: it spawns a process, stages
files into node runtime directories, writes drop files, bridges caller I/O, and
writes logs. Misconfiguration can cause the BBS process to execute the wrong
program, write files in the wrong place, or leak stale runtime data between door
runs.

## Decision

For v1, live door execution is constrained to explicitly allowed DOSEMU2-style
runners and door directories under the configured door root.

### Runner Allowlist

Add a config value:

```toml
[doors]
allowed_runners = ["dosemu", "dosemu2"]
```

The default allowlist is exactly `["dosemu", "dosemu2"]`.

`validate_door` MUST reject a door whose `runner` is not in this allowlist.

The resolved runner executable MUST:

- resolve to a real filesystem path
- be a regular file
- be owned by root or by the effective UID of the OxideBBS process
- not be group-writable
- not be world-writable

### Working Directory Containment

Door `working_dir` MUST be canonicalized and MUST be under the canonical
`paths.doors` directory.

Symlink traversal outside `paths.doors` MUST be rejected.

### Time Limit Cap

Door `time_limit_minutes` MUST be in the range `1..=240`.

### Runtime Directory Cleanup

Door runtime directories MUST be cleaned through an RAII-style guard. Cleanup
must happen on success, validation failure after runtime preparation, bridge
failure, child timeout, and early return.

### Log File Permissions

Door stdout/stderr capture files MUST be created with mode `0600` on Unix.

Runtime node directories used for door staging MUST be mode `0700` on Unix.

### Command Handling

The existing v1 DOS command restrictions remain:

- quoted DOS commands are rejected
- path-like DOS commands are rejected unless the current implementation already
  supports them safely
- the command executable must exist under the contained working directory

Agents MUST NOT weaken these restrictions while implementing this ADR.

## Consequences

- Door execution stays under sysop-controlled storage.
- A compromised or mistaken door config cannot point at arbitrary filesystem
  paths without validation failure.
- Door output logs are not group-readable by default.
- Runtime staging data is not reused accidentally by later callers on the same
  node.
- Some previously accepted door configurations will become invalid and must be
  corrected by the sysop.

## Rejected Options

- Trusting arbitrary `working_dir` paths: too easy to misconfigure and too broad
  for a server process.
- Trusting any runner found in `PATH`: too broad for live caller execution.
- Raising the max door runtime above 240 minutes for v1: long-running external
  processes are a resource exhaustion risk.
