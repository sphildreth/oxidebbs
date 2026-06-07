# Release Process

This page is the maintainer runbook for publishing OxideBBS releases.

Use [Versioning](./project/versioning.md) to choose the next version and
[Release Binaries](./project/release-binaries.md) to describe what sysops
receive after publication.

## Release Model

OxideBBS publishes GitHub releases from an existing `v` tag. The release
workflow builds and smokes every artifact before the GitHub release is created.

The workflow is manual by design:

1. choose the next SemVer tag
2. run a dry run against the intended source ref
3. create and push the tag after approval
4. run the release workflow with `dry_run=false`
5. verify the hosted archives and checksums

Do not publish an empty GitHub release first and upload assets later. When
GitHub release immutability is enabled, assets must be attached before the
release is published. The workflow uses `gh release create` with the completed
asset list so the GitHub CLI creates a draft, uploads assets, and publishes only
after upload succeeds.

## Dry Run

Run `.github/workflows/release.yml` from GitHub Actions with:

| Input | Value |
| --- | --- |
| `tag_name` | the intended tag, such as `vX.Y.Z` |
| `source_ref` | `main`, a release branch, or a commit SHA |
| `dry_run` | `true` |

The dry run builds the Linux, macOS, and Windows packages, verifies checksums,
smokes each packaged binary, builds the VitePress documentation site, and
builds/smokes the Docker image. It uploads the generated archives as workflow
artifacts only. It does not create a GitHub release or publish a Docker image.

## Publish

After the dry run passes and publication is approved, create the release tag and
push it to GitHub:

```bash
git tag vX.Y.Z
git push origin refs/tags/vX.Y.Z
```

Then run `.github/workflows/release.yml` with:

| Input | Value |
| --- | --- |
| `tag_name` | the tag that already exists on GitHub |
| `source_ref` | leave blank, or set to the same tag |
| `dry_run` | `false` |

The publish run checks out the tag, rebuilds and smokes all release artifacts,
pushes the smoke-tested Docker image to GitHub Container Registry, then creates
the GitHub release with the artifacts and `.sha256` files attached. The publish
job uses `--verify-tag`, so it fails instead of creating a tag from the default
branch.

## Failure Recovery

If a dry run fails, fix the branch and run the dry run again.

If publication fails before the final `Publish GitHub release` step, no GitHub
release has been created. If the Docker publish step already ran, inspect the
GHCR tags before rerunning publication for the same tag.

If publication creates a release and then fails, treat the tag as spent when
release immutability is enabled. Do not delete and reuse that release tag. Make
a follow-up patch release instead.

## Post-Publish Checks

After the workflow completes:

- confirm the GitHub release shows Linux, macOS, and Windows archives
- confirm every archive has a matching `.sha256` file
- download at least one archive and repeat the checksum and `--version` smoke
- confirm `ghcr.io/sphildreth/oxidebbs:<version>` and
  `ghcr.io/sphildreth/oxidebbs:v<version>` are visible and pullable
- confirm the docs site still builds and deploys from the Pages workflow
- update `design/TASKS.md` if any approval-gated publication checks remain open
