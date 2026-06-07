# Versioning

OxideBBS uses Semantic Versioning.

- **Major** releases contain breaking changes after the stable `v1.0.0` line.
- **Minor** releases add backwards-compatible user-visible capabilities.
- **Patch** releases contain fixes, packaging changes, CI changes, and docs
  updates that do not change the public contract.

Before the historical `v1.0.0` release milestone, breaking changes were allowed,
but release notes should still call out changes that affect config files,
DecentDB data, ANSI assets, door definitions, or operator workflows.

The full maintainer checklist lives in
[`design/VERSIONING_GUIDE.md`](https://github.com/sphildreth/oxidebbs/blob/main/design/VERSIONING_GUIDE.md).
The publication runbook lives in [Release Process](../release.md).
Routine version bumps use the root `VERSION` file and
`scripts/bump-version.sh <version>` to keep Cargo metadata, docs package
metadata, and lockfiles aligned.
