#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

DOSBOX_BIN="${DOSBOX_BIN:-dosbox}"

if ! command -v "$DOSBOX_BIN" >/dev/null 2>&1; then
  printf 'SKIP: DOSBox executable %s not found\n' "$DOSBOX_BIN"
  exit 77
fi

if [[ "${OXIDE_DOOR_INTERACTIVE:-0}" != "1" ]]; then
  printf 'SKIP: set OXIDE_DOOR_INTERACTIVE=1 to run the interactive DOSBox smoke test\n'
  exit 77
fi

if ! command -v nc >/dev/null 2>&1; then
  printf 'SKIP: nc not found; serial smoke test needs a local TCP peer\n'
  exit 77
fi

DOOR_DIR="$REPO_ROOT/tools/doors/oxide-door-check/dist"
DOOR_EXE="$DOOR_DIR/OXIDECHK.EXE"
RUNTIME_DIR="$REPO_ROOT/target/oxide-door-dosbox/node-001"
SERIAL_PORT="${OXIDE_DOOR_TEST_PORT:-52323}"
SERIAL_LOG="$RUNTIME_DIR/serial.out"
DOSBOX_CONF="$RUNTIME_DIR/OXDOSBOX.CONF"

if [[ ! -f "$DOOR_EXE" ]]; then
  printf 'missing %s\n' "$DOOR_EXE" >&2
  exit 1
fi

rm -rf "$RUNTIME_DIR"
mkdir -p "$RUNTIME_DIR"

printf 'OxideBBS\r\nSysop\r\nCOM1\r\n38400 BAUD,N,8,1\r\n0\r\nTest\r\nCaller\r\nLocalhost\r\n100\r\n30\r\n' > "$RUNTIME_DIR/DORINFO1.DEF"

printf 'node=1\r\n' > "$RUNTIME_DIR/OXNODE.TXT"

cat > "$DOSBOX_CONF" <<EOF
[sdl]
waitonerror=false
pause_when_inactive=false
mute_when_inactive=true

[dosbox]
startup_verbosity=quiet

[serial]
serial1=nullmodem server:127.0.0.1 port:$SERIAL_PORT transparent:1 rxdelay:1000 txdelay:10
EOF

cat <<EOF
Launching Oxide Door Check under DOSBox with COM1 mapped to TCP port $SERIAL_PORT.

The script sends I, R, and Q over the serial path and records serial output at:
  $SERIAL_LOG

EOF

(
  sleep 8
  printf 'I\r'
  sleep 1
  printf 'R\r'
  sleep 1
  printf 'Q\r'
  sleep 5
) | nc -l 127.0.0.1 "$SERIAL_PORT" > "$SERIAL_LOG" &
NC_PID=$!
trap 'kill "$NC_PID" >/dev/null 2>&1 || true' EXIT

DOSBOX_CMD=(
  "$DOSBOX_BIN"
  --noprimaryconf
  --nolocalconf
  --conf "$DOSBOX_CONF"
  -c "mount c $DOOR_DIR" \
  -c "mount d $RUNTIME_DIR" \
  -c "path C:\\" \
  -c "d:" \
  -c "OXIDECHK.EXE" \
  -c "exit"
)

if command -v timeout >/dev/null 2>&1; then
  timeout 30s "${DOSBOX_CMD[@]}"
else
  "${DOSBOX_CMD[@]}"
fi

wait "$NC_PID" || true
trap - EXIT

if ! grep -q 'Oxide Door Check' "$SERIAL_LOG"; then
  printf 'serial smoke test did not capture expected door output\n' >&2
  exit 1
fi

if [[ ! -f "$RUNTIME_DIR/OXIDECHK.RPT" ]]; then
  printf 'serial smoke test did not create %s\n' "$RUNTIME_DIR/OXIDECHK.RPT" >&2
  exit 1
fi

printf 'serial DOSBox smoke test passed\n'
