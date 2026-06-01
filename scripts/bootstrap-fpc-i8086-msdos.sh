#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

FPC_VERSION="3.2.2"
RPM_NAME="fpc-i8086-msdos-${FPC_VERSION}-1.x86_64.rpm"
RPM_URL="https://downloads.freepascal.org/fpc/dist/${FPC_VERSION}/x86_64-linux/${RPM_NAME}"
RPM_SHA256="34e9d71bd3d05e0d87713b56089e5286169d9d008de3593d4851880cc074227c"

TOOLCHAIN_DIR="$REPO_ROOT/target/fpc-i8086-msdos"
DOWNLOAD_DIR="$TOOLCHAIN_DIR/downloads"
EXTRACT_DIR="$TOOLCHAIN_DIR/stage"
BIN_DIR="$TOOLCHAIN_DIR/bin"
RPM_PATH="$DOWNLOAD_DIR/$RPM_NAME"
COMPILER_REAL="$EXTRACT_DIR/usr/lib64/fpc/${FPC_VERSION}/ppcross8086"
COMPILER_WRAPPER="$BIN_DIR/ppcross8086"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'missing required command: %s\n' "$1" >&2
    exit 1
  fi
}

need_cmd curl
need_cmd sha256sum
need_cmd rpm2cpio
need_cmd cpio

mkdir -p "$DOWNLOAD_DIR" "$BIN_DIR"

if [[ ! -f "$RPM_PATH" ]]; then
  printf 'downloading %s\n' "$RPM_URL"
  curl -fL --retry 3 --retry-delay 2 -o "$RPM_PATH" "$RPM_URL"
fi

printf '%s  %s\n' "$RPM_SHA256" "$RPM_PATH" | sha256sum -c -

if [[ ! -x "$COMPILER_REAL" ]]; then
  rm -rf "$EXTRACT_DIR"
  mkdir -p "$EXTRACT_DIR"
  (
    cd "$EXTRACT_DIR"
    rpm2cpio "$RPM_PATH" | cpio -idmu --quiet
  )
fi

if [[ ! -x "$COMPILER_REAL" ]]; then
  printf 'failed to stage Free Pascal i8086-msdos compiler at %s\n' "$COMPILER_REAL" >&2
  exit 1
fi

cat > "$COMPILER_WRAPPER" <<EOF
#!/usr/bin/env bash
exec "$COMPILER_REAL" "\$@"
EOF
chmod +x "$COMPILER_WRAPPER"

if ! "$COMPILER_WRAPPER" -i | grep -q 'MSDOS: MS-DOS 16-bit real mode'; then
  printf 'staged compiler does not advertise the required i8086-msdos target\n' >&2
  exit 1
fi

printf 'staged Free Pascal %s i8086-msdos compiler:\n' "$("$COMPILER_WRAPPER" -iV)"
printf '  %s\n' "$COMPILER_WRAPPER"

