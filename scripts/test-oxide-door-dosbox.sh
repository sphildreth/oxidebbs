#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

if ! command -v dosbox >/dev/null 2>&1; then
  printf 'SKIP: dosbox not found\n'
  exit 77
fi

if [[ "${OXIDE_DOOR_INTERACTIVE:-0}" != "1" ]]; then
  printf 'SKIP: set OXIDE_DOOR_INTERACTIVE=1 to run the interactive DOSBox smoke test\n'
  exit 77
fi

DOOR_DIR="$REPO_ROOT/tools/doors/oxide-door-check/dist"
DOOR_EXE="$DOOR_DIR/OXIDECHK.EXE"
RUNTIME_DIR="$REPO_ROOT/target/oxide-door-dosbox/node-001"

if [[ ! -f "$DOOR_EXE" ]]; then
  printf 'missing %s\n' "$DOOR_EXE" >&2
  exit 1
fi

rm -rf "$RUNTIME_DIR"
mkdir -p "$RUNTIME_DIR"

cat > "$RUNTIME_DIR/DORINFO1.DEF" <<'EOF'
OxideBBS
Sysop
COM1
38400 BAUD,N,8,1
0
Test
Caller
Localhost
100
30
EOF

printf 'node=1\r\n' > "$RUNTIME_DIR/OXNODE.TXT"

cat <<EOF
Launching Oxide Door Check under DOSBox.

Inside the DOSBox window:
  1. Press I to view node info.
  2. Press R to write OXIDECHK.RPT.
  3. Press Q to return and close DOSBox.

EOF

dosbox \
  -c "mount c $DOOR_DIR" \
  -c "mount d $RUNTIME_DIR" \
  -c "d:" \
  -c "C:\\OXIDECHK.EXE" \
  -c "exit"

