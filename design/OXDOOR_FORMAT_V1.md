# Oxide Door Package Format v1 (`.oxdoor`)

Status: Draft v1  
Audience: OxideBBS maintainers, door package authors, and sysops  
Primary producer: `oxidebbs-door-lab`  
Primary consumer: `oxidebbs-server doors package ...`

## Quick format summary

An `.oxdoor` file is a ZIP archive with UTF-8 text metadata and file payload sections:

```text
oxide-door.toml
checksums.sha256
files/
docs/             optional
tests/            optional
```

`oxide-door.toml` must include:
- `package.format = "oxide-door-package-v1"`
- `package.kind = "full"`

All files under `files/`, `docs/`, and `tests/` should be listed in `checksums.sha256` using:

```text
<sha256-hex>  <relative-path>
```

## 1. Purpose

An Oxide Door Package (`.oxdoor`) is a portable, declarative package for installing a DOS BBS door into an OxideBBS system.

The goal is to make door setup repeatable without asking sysops to manually copy archives, guess launch commands, hand-edit door records, or re-discover drop-file requirements every time a door is installed.

A v1 full package may contain:

- door metadata
- source and legal metadata
- extracted door files
- file checksums
- OxideBBS runtime settings
- drop-file preferences
- persistence rules
- test hints
- menu/category hints

A v1 package must not contain arbitrary installer hooks or scripts that OxideBBS executes automatically.

## 2. Design principles

### 2.1 Declarative, not executable

The package describes what should be installed. It must not run installer scripts, key generators, shell scripts, batch files, or downloaded commands as part of import.

OxideBBS may copy files, create or update door definitions through its own internal services, validate package contents, generate drop files, and run supported dry-run checks. It must not execute package-provided installer code during import.

### 2.2 Safe by default

Imported doors should default to disabled unless the sysop explicitly passes an enable flag. A package import should not make a new third-party binary immediately caller-accessible by default.

### 2.3 No hidden writes

`--dry-run` import must perform validation and print planned changes without writing files, modifying DecentDB, enabling doors, or changing menus.

### 2.4 No path traversal

Package paths must be relative, normalized, and contained inside the package root. Importers must reject paths that are absolute, contain `..`, contain Windows drive prefixes, or otherwise escape the target install directory.

### 2.5 Reproducible and inspectable

Every packaged file under `files/` should appear in `checksums.sha256`. Importers should verify hashes before installation.

### 2.6 Separate public recipes from private full packages

A full `.oxdoor` package may contain third-party door binaries when the sysop has the right to store/use them privately.

A public recipe should not redistribute third-party binaries unless redistribution rights are clear. Public recipes may describe where to fetch a door and what hash to expect, but should avoid bundling copyrighted/shareware/abandonware binaries unless explicitly permitted.

## 3. File extension and container

A v1 `.oxdoor` package is a ZIP archive with the file extension `.oxdoor`.

The ZIP archive must use forward slash path separators. Files should be stored with deterministic relative paths where practical.

Required top-level entries:

```text
oxide-door.toml
checksums.sha256
files/
```

Optional top-level entries:

```text
docs/
tests/
```

Recommended layout:

```text
doradvnt.oxdoor
├── oxide-door.toml
├── checksums.sha256
├── files/
│   ├── DORADVNT.EXE
│   ├── DORADVNT.DOC
│   └── ...
├── docs/
│   ├── README.TXT
│   └── SYSOP.DOC
└── tests/
    └── smoke.md
```

## 4. Package kinds

v1 recognizes two package kinds:

```toml
kind = "full"
```

A full package contains the actual door files under `files/`.

```toml
kind = "recipe"
```

A recipe package contains metadata, source URLs, expected archive hashes, and setup instructions, but does not include third-party binaries under `files/`. Recipe support may be implemented after full package support.

For the first implementation, OxideBBS may support only `kind = "full"` and reject `recipe` with a clear message.

## 5. `oxide-door.toml`

`oxide-door.toml` is the package manifest. It must be UTF-8 TOML.

### 5.1 Minimal example

```toml
[package]
format = "oxide-door-package-v1"
kind = "full"
id = "doradvnt"
name = "Door Adventure"
version = "unknown"
created_by = "oxidebbs-door-lab"
created_at = "2026-06-07T00:00:00Z"

[legal]
status = "freeware_confirmed"
requires_key = false
redistributable = "operator_asserted"
notes = "User identified this package source as freeware and no registration key required."

[source]
name = "Fool's Quarter BBS Files"
url = "https://bbs.foolsquarter.com/files/doradvnt.zip"
archive_filename = "doradvnt.zip"
archive_sha256 = "TO_BE_FILLED_AFTER_DOWNLOAD"

[door]
id = "doradvnt"
name = "Door Adventure"
description = "Legacy DOS BBS door packaged for OxideBBS."
category = "Adventure"
runner = "local:dosemu2"
working_dir = "doradvnt"
command = "VERIFY_AFTER_INSPECTION"
preferred_dropfile = "DORINFO1.DEF"
supported_dropfiles = ["DORINFO1.DEF", "DOOR.SYS"]
exclusive = false
timeout_seconds = 900
enabled_after_import = false

[access]
min_security_level = 10

[persistence]
include = ["*.DAT", "*.CFG", "*.SCO", "*.IDX", "*.ANS", "*.TXT", "*.SCR", "*.RNX"]
exclude = ["DOOR.SYS", "DORINFO1.DEF", "CHAIN.TXT", "DOORFILE.SR", "PCBOARD.SYS", "CALLINFO.BBS", "OXNODE.TXT", "OXDOSEMU2.CONF", "OXCOM1.PTY"]

[test]
dry_run = true
expected_output = []
quit_sequence = ["Q", "ENTER"]

[menu]
category = "Adventure"
suggested_key = "A"
suggested_label = "Door Adventure"
```

## 6. Manifest fields

### 6.1 `[package]`

Required fields:

- `format`: must be `"oxide-door-package-v1"`.
- `kind`: `"full"` or `"recipe"`.
- `id`: package id. Should match `door.id` for simple packages.
- `name`: human-readable package name.
- `version`: package version, upstream door version, or `"unknown"`.
- `created_by`: tool or person that created the package.
- `created_at`: UTC timestamp in RFC 3339 format when possible.

Rules:

- `id` must be lowercase ASCII using only `a-z`, `0-9`, and `-`.
- `id` should be stable over time.
- `id` must not contain path separators.

### 6.2 `[legal]`

Required fields:

- `status`
- `requires_key`
- `redistributable`

Suggested `status` values:

- `freeware_confirmed`
- `author_released`
- `public_domain`
- `shareware_unregistered`
- `operator_provided`
- `unknown`
- `legal_hold`

Suggested `redistributable` values:

- `yes`
- `no`
- `unknown`
- `operator_asserted`

Rules:

- `legal_hold` packages must not be imported unless a sysop passes a future explicit override flag.
- `requires_key = true` is allowed, but v1 import must not run key generators.
- Key generators must never be executed automatically by package import.

### 6.3 `[source]`

Recommended fields:

- `name`
- `url`
- `archive_filename`
- `archive_sha256`
- `retrieved_at`
- `notes`

Rules:

- `archive_sha256` should be present for packages created from a known archive.
- If hash verification was not possible, use a clear placeholder and make the package builder fail unless an explicit `--allow-missing-source-hash` flag is supplied.

### 6.4 `[door]`

Required fields:

- `id`
- `name`
- `runner`
- `working_dir`
- `command`
- `preferred_dropfile`
- `supported_dropfiles`
- `exclusive`
- `timeout_seconds`
- `enabled_after_import`

Suggested fields:

- `description`
- `category`
- `environment`
- `notes`

Rules:

- `door.id` must use lowercase ASCII `a-z`, `0-9`, and `-`.
- `runner` v1 should support `local:dosemu2` first.
- `command` is the command OxideBBS should run from the installed door working directory.
- `working_dir` is relative to the configured OxideBBS door root unless OxideBBS explicitly supports another safe mapping.
- `enabled_after_import` should default to `false`.

Supported v1 drop-file values should match OxideBBS-supported formats:

- `DOOR.SYS`
- `DORINFO1.DEF`
- `CHAIN.TXT`
- `DOORFILE.SR`
- `PCBOARD.SYS`
- `CALLINFO.BBS`

### 6.5 `[access]`

Recommended fields:

- `min_security_level`

Rules:

- If omitted, OxideBBS should use its existing default door access behavior.
- Importers should reject negative security levels.

### 6.6 `[persistence]`

Recommended fields:

- `include`
- `exclude`

Purpose:

This section tells OxideBBS which door-owned files are expected to persist between per-node runtime sessions.

Rules:

- Generated drop files and OxideBBS runtime bridge files should be excluded.
- Patterns are advisory for v1 if OxideBBS already has built-in persistence behavior.
- Future versions may make persistence rules stricter.

### 6.7 `[test]`

Recommended fields:

- `dry_run`
- `expected_output`
- `quit_sequence`
- `notes`

Rules:

- `dry_run = true` means the package author expects `doors test --dry-run` to be meaningful.
- `expected_output` is intended for future live telnet smoke tests, not for v1 dry-run import.
- `quit_sequence` uses symbolic tokens such as `ENTER`, `ESC`, `CTRL_C`, or printable strings.

### 6.8 `[menu]`

Optional fields:

- `category`
- `suggested_key`
- `suggested_label`

Rules:

- v1 package import should treat menu data as hints only.
- v1 import should not rewrite caller menu files automatically.
- Future OxideBBS versions may use this section to assign door categories or generate safe menu entries.

## 7. `checksums.sha256`

`checksums.sha256` contains SHA-256 hashes for files packaged under `files/`, `docs/`, and `tests/` as appropriate.

Format:

```text
<sha256-hex>  <relative-path>
```

Example:

```text
8b2f...  files/DORADVNT.EXE
1c94...  files/DORADVNT.DOC
```

Rules:

- Paths must be relative to the package root.
- Paths must use forward slashes.
- Importers must reject checksum paths that are absolute or escape the package root.
- For `kind = "full"`, every regular file under `files/` must be listed.
- Importers should reject a full package if `files/` is empty.

## 8. Import behavior

### 8.1 Inspect

Command goal:

```bash
oxidebbs-server doors package inspect <path-to-package.oxdoor>
```

Expected behavior:

- Open the package as ZIP.
- Verify `oxide-door.toml` exists.
- Parse TOML.
- Verify `package.format`.
- Verify `checksums.sha256`.
- Validate required fields.
- Reject unsafe paths.
- Print a human-readable summary.
- Do not write files.
- Do not modify DecentDB.
- Do not enable a door.

### 8.2 Dry-run import

Command goal:

```bash
oxidebbs-server doors package import <path-to-package.oxdoor> --dry-run
```

Expected behavior:

- Perform all inspect validations.
- Compute target install directory.
- Detect existing door definition conflicts.
- Detect existing file/directory conflicts.
- Validate runner and drop-file values against OxideBBS-supported behavior.
- Print planned file copies.
- Print planned door definition fields.
- Print planned follow-up validation commands.
- Do not write files.
- Do not modify DecentDB.
- Do not enable a door.

### 8.3 Real import

Command goal:

```bash
oxidebbs-server doors package import <path-to-package.oxdoor>
oxidebbs-server doors package import <path-to-package.oxdoor> --replace
oxidebbs-server doors package import <path-to-package.oxdoor> --enable
```

Expected behavior:

- Perform all dry-run validations.
- Copy `files/` into the configured door root under the package door working directory.
- Create or update the OxideBBS door definition through existing door service code paths.
- Default to disabled unless an explicit sysop `--enable` flag is provided. A
  package `enabled_after_import` request is advisory and must not enable a door
  by itself.
- Run the same validation used by `doors check` when feasible.
- Print suggested next commands.

Conflict behavior:

- If a door definition already exists, fail unless `--replace` is provided.
- If a target directory already exists, fail unless `--replace` is provided.
- `--replace` should be conservative and should avoid deleting unknown existing files unless explicitly designed and tested.

## 9. Security requirements

Importers must reject:

- absolute package paths
- `..` path traversal
- Windows drive-prefixed paths such as `C:\...`
- symlinks, hard links, device files, and special files inside the ZIP
- package entries that resolve outside the target door directory
- unsupported package format versions
- unsupported runner values
- unsupported drop-file values
- missing required manifest fields
- checksum mismatches
- `legal.status = "legal_hold"` unless a future explicit override exists

Importers must not:

- execute package-provided scripts
- execute key generators
- execute batch files during import
- fetch remote files during v1 full-package import
- rewrite caller menus automatically in v1
- enable the door by default without explicit sysop intent

## 10. Door-lab responsibilities

`oxidebbs-door-lab` should be responsible for:

- downloading source archives
- hashing source archives
- extracting archives into staging
- inspecting docs and executable names
- producing candidate manifests
- building `.oxdoor` packages
- producing audit reports
- optionally running local dry-run/live smoke tests

`oxidebbs-door-lab` may generate packages that OxideBBS can import, but it should not be required at runtime by OxideBBS.

## 11. OxideBBS responsibilities

OxideBBS should be responsible for:

- validating `.oxdoor` packages
- safely copying package files into the configured doors root
- creating/updating door definitions in DecentDB through existing internal services
- running existing door checks and dry-run tests
- keeping caller menu routing safe
- defaulting imported third-party doors to disabled

## 12. Versioning and compatibility

The v1 package format string is:

```toml
format = "oxide-door-package-v1"
```

Future incompatible changes should use a new format string, such as:

```toml
format = "oxide-door-package-v2"
```

OxideBBS should reject unknown future versions with a clear error.

## 13. Initial implementation scope

The first implementation should support:

- full packages only
- ZIP container only
- TOML manifest only
- local DOSEMU2 doors only
- package inspect
- package import `--dry-run`
- package import disabled by default
- checksum verification
- path safety validation

Out of scope for v1:

- arbitrary post-install scripts
- keygen automation
- remote recipe fetching
- automatic menu rewriting
- dependency resolution
- disk-image-backed door installs
- automatic public redistribution decisions
