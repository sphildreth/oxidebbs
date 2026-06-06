#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

VERSION_VALUE="${1:-$(tr -d '[:space:]' < VERSION)}"

if [[ ! "$VERSION_VALUE" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.-]+)?$ ]]; then
  printf 'invalid version %s; use SemVer without a leading v\n' "$VERSION_VALUE" >&2
  exit 1
fi

printf '%s\n' "$VERSION_VALUE" > VERSION

for manifest in crates/*/Cargo.toml; do
  perl -0pi -e "s/^version = \"[^\"]+\"/version = \"$VERSION_VALUE\"/m" "$manifest"
done

VERSION_VALUE="$VERSION_VALUE" node --input-type=module <<'NODE'
import fs from 'node:fs';

const version = process.env.VERSION_VALUE;

for (const path of ['package.json', 'package-lock.json']) {
  const doc = JSON.parse(fs.readFileSync(path, 'utf8'));
  doc.version = version;
  if (doc.packages?.['']) {
    doc.packages[''].version = version;
  }
  fs.writeFileSync(path, `${JSON.stringify(doc, null, 2)}\n`);
}
NODE

perl -0pi -e "s/default: v[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.-]+)?/default: v$VERSION_VALUE/" \
  .github/workflows/release.yml

if command -v cargo >/dev/null 2>&1; then
  cargo metadata --no-deps --format-version 1 >/dev/null
else
  printf 'warning: cargo not found; Cargo.lock was not refreshed\n' >&2
fi

if command -v npm >/dev/null 2>&1; then
  npm install --package-lock-only --ignore-scripts >/dev/null
else
  printf 'warning: npm not found; package-lock.json may need refresh\n' >&2
fi

if ! rg -q '^## \[Unreleased\]' docs/about/changelog.md; then
  printf 'docs/about/changelog.md is missing the Unreleased changelog placeholder\n' >&2
  exit 1
fi

printf 'Updated OxideBBS release metadata to %s\n' "$VERSION_VALUE"
