#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

FPC_VERSION="3.2.2"
TOOLCHAIN_DIR="$REPO_ROOT/target/fpc-i8086-msdos"
COMPILER="$TOOLCHAIN_DIR/bin/ppcross8086"
UNIT_BASE="$TOOLCHAIN_DIR/stage/usr/lib64/fpc/${FPC_VERSION}/units/msdos/8086-small"
DOOR_DIR="$REPO_ROOT/tools/doors/oxide-door-check"
SOURCE="$DOOR_DIR/src/oxidechk.pas"
BUILD_DIR="$REPO_ROOT/target/oxidechk-door-build"
DIST_DIR="$DOOR_DIR/dist"
OUTPUT="$DIST_DIR/OXIDECHK.EXE"

if [[ ! -x "$COMPILER" ]]; then
  printf 'missing staged Free Pascal i8086-msdos compiler; run ./scripts/bootstrap-fpc-i8086-msdos.sh\n' >&2
  exit 1
fi

if [[ ! -d "$UNIT_BASE/rtl" || ! -d "$UNIT_BASE/rtl-console" ]]; then
  printf 'staged Free Pascal i8086-msdos units are incomplete under %s\n' "$UNIT_BASE" >&2
  exit 1
fi

rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR" "$DIST_DIR"

"$COMPILER" \
  -n \
  -Tmsdos \
  -Mtp \
  -Wmsmall \
  -CX \
  -XX \
  -Fu"$UNIT_BASE/rtl" \
  -Fu"$UNIT_BASE/rtl-console" \
  -FE"$BUILD_DIR" \
  -FU"$BUILD_DIR" \
  -oOXIDECHK.EXE \
  "$SOURCE"

cp "$BUILD_DIR/OXIDECHK.EXE" "$OUTPUT"

(
  cd "$DOOR_DIR"
  sha256sum dist/OXIDECHK.EXE > SHA256SUMS
  sha256sum -c SHA256SUMS
)

printf 'rebuilt %s with Free Pascal %s i8086-msdos\n' "$OUTPUT" "$("$COMPILER" -iV)"

