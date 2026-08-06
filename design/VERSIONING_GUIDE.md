# OxideBBS Versioning Guide

This guide defines how OxideBBS version jumps work and which files must be
updated when the project version changes.

## 1. Versioning policy

OxideBBS uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html):

- **Major (`X.0.0`)** for breaking changes to stable caller behavior, config
  format, database schema compatibility, door-runner contracts, sysop tooling
  commands, or other public compatibility boundaries.
- **Minor (`X.Y.0`)** for backwards-compatible feature additions.
- **Patch (`X.Y.Z`)** for backwards-compatible fixes, packaging adjustments, CI
  fixes, and documentation updates that do not change the public contract.

Before the historical `v1.0.0` milestone, OxideBBS may make breaking changes
more freely, but release notes should still call out anything that affects
config files, DecentDB data, ANSI assets, door definitions, or operator
workflows.

### Choosing the bump when a branch has mixed changes

Pick the **highest-impact** change class in the branch:

1. Any breaking compatibility change => **Major**
2. Otherwise, any new user-visible capability => **Minor**
3. Otherwise (fixes/tooling/docs only) => **Patch**

Examples:

- Feature + bug fix in one branch => **Minor** (not Patch)
- Docs + CI + packaging only => **Patch**
- Config key rename + docs update => **Major** after `v1.0.0`

### Public release line

The first compatibility-stable public release line begins at `v1.0.0`.

## 2. Source of truth

The root `VERSION` file is the current OxideBBS release version source of truth.
Keep all workspace crate versions aligned with it unless a crate is
intentionally split onto its own release line in a future ADR.

When the OxideBBS release version changes:

1. update `VERSION`
2. update all OxideBBS crate versions
3. update release-facing documentation
4. refresh lockfiles only when dependency metadata changes
5. re-scan for stale old-version strings

Use `scripts/bump-version.sh <version>` for routine version bumps. The script
updates `VERSION`, workspace crate manifests, docs package metadata, generated
lockfile metadata, and verifies that the changelog still has an `Unreleased`
placeholder.

### Rust workspace

- `Cargo.toml` (`[workspace.package] version`; all member crates use
  `version.workspace = true`)
- `Cargo.lock`
- `VERSION`
- `scripts/bump-version.sh`

The server binary makes `Cargo.lock` release-facing metadata. Commit lockfile
changes that result from legitimate dependency or package metadata updates.

### Documentation site

- `package.json`
- `package-lock.json`
- `docs/**`

The VitePress package metadata exists to build `https://oxidebbs.com`. It should
not drift accidentally, but frontend tooling dependency bumps are not OxideBBS
product releases by themselves.

### Documentation

- `docs/about/changelog.md`
  Add release notes under `Unreleased` or under the new version heading,
  depending on the release process being used.
- `README.md`
- `design/PRD.md`
- `design/SPEC.md`
- `design/ROADMAP.md`
- `design/TASKS.md`
- `design/VERSIONING_GUIDE.md`

Update docs when the release changes product scope, user-visible behavior,
operator workflows, or compatibility promises.

### Configuration and examples

- `config/oxidebbs.example.toml`
- `compose.yaml`
- `assets/ansi/**`

Update example config and bundled ANSI assets when a release changes the
expected config shape, default paths, terminal profile behavior, or included
starter screens.

### DecentDB dependency

- `Cargo.toml`
- `Cargo.lock`

DecentDB is an external dependency pinned by Git tag. Updating the DecentDB tag
is a dependency update, not an OxideBBS release version bump by itself. If the
new DecentDB tag changes schema behavior or data compatibility, document that in
`CHANGELOG.md` and `design/DECENTDB_SCHEMA.md`.

### Release automation

- `.github/workflows/ci.yml`
- `.github/workflows/pages.yml`
- `.github/workflows/release.yml`

CI and Pages workflows should not hard-code an OxideBBS release version unless a
future release process explicitly needs it.

The release-artifact workflow is manually dispatched. It builds Linux, macOS,
and Windows archives, verifies checksums, smokes the packaged binaries, builds
the documentation site, and builds/smokes the Docker image before any GitHub
release is created. In `publish` mode, it also publishes the smoke-tested
Docker image to GitHub Container Registry before the final job downloads the
completed workflow artifacts and creates the GitHub release with all archive and
checksum assets attached.

Do not publish an empty GitHub release and upload assets afterward. With GitHub
release immutability enabled, assets must be attached while the release is still
draft. The workflow relies on `gh release create <tag> <assets...>` so the
GitHub CLI creates a draft, uploads assets, and publishes only after upload
succeeds.

## 3. Files that usually do **not** need a version bump

Do **not** bump unrelated tooling or example metadata just to match the OxideBBS
release unless it surfaces the shipped OxideBBS version to users or participates
in release artifact validation.

Examples:

- third-party dependency versions in `Cargo.lock`
- third-party dependency versions in `package-lock.json`
- local development notes
- generated VitePress build output

Those files may contain version numbers, but they are not automatically part of
the OxideBBS release version.

Exception: if package metadata changes cause a lockfile to record the new local
package version, refresh and commit the lockfile.

## 4. Recommended version-bump procedure

1. Decide the next version according to SemVer, using the highest-impact rule
   above.
2. Run `scripts/bump-version.sh <version>`.
3. Update release notes in `docs/about/changelog.md`.
4. Update user-facing docs and examples that changed with the release.
5. Refresh `Cargo.lock` if package metadata or dependencies changed outside the
   bump script.
6. Refresh `package-lock.json` if documentation package metadata or dependencies
   changed.
7. Re-scan the repository for stale release-version strings.
8. Run the full Rust and docs validation commands.
9. Run the release workflow in `validate` mode against the intended release ref.
10. After publication approval, create and push the release tag.
11. Run the release workflow in `publish` mode for the existing tag.
12. Verify hosted archives, checksums, docs, Docker smoke results, and GHCR
    image publication.

## 5. Documentation-site procedure

The documentation site is built with VitePress from the `docs/` directory. After
changing `package.json`, refresh the lockfile with npm rather than hand-editing
it.

```bash
npm install --package-lock-only --ignore-scripts
```

For normal documentation validation:

```bash
npm ci
npm run docs:build
```

The published site uses the custom domain `https://oxidebbs.com`. GitHub Pages
must be configured to deploy from GitHub Actions, and DNS must point the domain
at GitHub Pages.

## 6. Validation checklist

After a version bump, verify:

- all OxideBBS crate versions have the intended version
- `Cargo.lock` reflects the intended package and dependency metadata
- `CHANGELOG.md` explains the release and important compatibility context
- `README.md`, `docs/**`, and relevant `design/**` files match the release
- `config/oxidebbs.example.toml` still represents a valid starter config
- no stale old-version references remain in release-facing files
- the GitHub Pages workflow still builds the documentation site

Useful commands:

```bash
cargo metadata --no-deps --format-version 1 >/dev/null
npm run docs:build

rg 'OLD_VERSION|vOLD_VERSION' \
  Cargo.toml \
  crates \
  Cargo.lock \
  package.json \
  package-lock.json \
  docs/about/changelog.md \
  README.md \
  docs \
  design \
  config \
  .github/workflows
```

Replace `OLD_VERSION` with the version you are replacing.

## 7. Release tag rules

When publishing, use Git tags with a leading `v`:

- Stable release: `v1.0.0`
- Release candidate: `v1.0.0-rc.1`
- Pre-1.0 release: `v0.1.0`

Release automation should convert those tags into package versions without the
leading `v` only where a downstream package format requires it.

GitHub release artifacts are built from the release tag by default. The workflow
builds with Rust target triples but names packages with friendlier platform
suffixes, such as `oxidebbs-<version>-linux-x86_64-gnu.tar.gz`. Each uploaded
archive should include a matching `.sha256` checksum file.

Manual release workflow dispatch defaults to `validate` mode. A validate run
checks out the supplied `source_ref`, or the workflow commit when `source_ref`
is blank, then builds and smokes the Linux, macOS, and Windows archives,
verifies each generated checksum file, builds the VitePress docs, and
builds/smokes the Docker image without creating a GitHub release or publishing
the image.

For publication, create and push the release tag before running the workflow in
`publish` mode. Leave `source_ref` blank; publish always builds the tag, and
the prepare-release job fails fast if the tag does not exist on GitHub. The
publish job uses `--verify-tag`, so it fails instead of creating a tag from the
default branch. If an immutable published release ever reserves a tag name, do
not delete and reuse that tag; publish the next patch tag instead.

The publish run pushes Docker images to:

- `ghcr.io/sphildreth/oxidebbs:<version>`
- `ghcr.io/sphildreth/oxidebbs:v<version>`
- `ghcr.io/sphildreth/oxidebbs:latest` for stable, non-prerelease tags
