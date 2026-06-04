# Repository Strategy

## Canonical host

GitHub is the canonical repository host for OxideBBS:

```text
https://github.com/sphildreth/oxidebbs
```

## Why GitHub?

GitHub is preferred for this project because the project is interested in outside contributors and benefits from:

- Contributor familiarity
- Issue and pull request workflows
- GitHub Actions
- Discoverability
- Integration with common Rust tooling
- Integration with coding agents and AI-assisted development workflows

## Codeberg

Codeberg is maintained as an optional mirror target while GitHub remains the
canonical repository host.

Mirror target:

```text
https://codeberg.org/sphildreth/oxidebbs
```

The `.github/workflows/codeberg-mirror.yml` workflow is manually dispatched and
defaults to `dry_run = true`, so maintainers can validate the ref update without
pushing. A real mirror update requires `dry_run = false` and configured
`CODEBERG_MIRROR_URL` plus `CODEBERG_MIRROR_SSH_KEY` secrets when SSH
authentication is needed.

Mirror failures do not change repository authority. Recovery is to inspect the
failed workflow logs, confirm the Codeberg remote and deploy key, rerun with
`dry_run = true`, then rerun with `dry_run = false` only after the dry-run output
matches the expected refs.
