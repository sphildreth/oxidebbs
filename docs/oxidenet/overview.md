# OxideNet

OxideNet is the first-party FTN-style network profile planned for OxideBBS. It
uses the shared `oxidebbs-network` model and is intended to sit on top of the
FTN/BinkP foundations instead of hard-coding network behavior into core caller
sessions.

Current foundation:

- `oxidebbs-oxidenet` defines default addressing, area, application lifecycle,
  node, and config-package data structures.
- Address helpers classify `42:1/1`, `42:1/2`, infrastructure, member,
  test/lab, and future-net ranges, and can allocate the next available
  top-level member address.
- Config-package structs model the planned `oxidenet.toml`, `areas.toml`,
  `nodelist.toml`, and `credentials.toml` contents with validation for
  addresses, hub settings, matching credentials, policy acceptance, and area
  tags.
- Shared network tables and types exist in `oxidebbs-network` and DecentDB.
- DecentDB schema `8` stores OxideNet application, assigned-node, and
  credential-hash registry rows for future onboarding and lifecycle workflows.
- Legacy FTN packet, kludge, duplicate, bundle-classification, BinkP frame, and
  nodelist foundations exist in adjacent crates.

Not release-ready yet:

- BBS-native application submission and admin review flows that populate the
  registry.
- Token-based join and credential rotation.
- Config-package generation/import wired to runtime state.
- Hub/member message flow, nodelist generation, BinkP polling, suspension, and
  public experimental network workflows.
- OxideNet TUI screens and daily-operations docs.

Until those gates are implemented, OxideNet is a design and data-model
foundation rather than an operational network.
