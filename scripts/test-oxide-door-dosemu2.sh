#!/usr/bin/env bash
set -euo pipefail
trap '' HUP

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

DOSEMU_BIN="${DOSEMU_BIN:-dosemu}"
DOOR_DIR="$REPO_ROOT/tools/doors/oxide-door-check/dist"
DOOR_EXE="$DOOR_DIR/OXIDECHK.EXE"
mkdir -p "$REPO_ROOT/target"
RUNTIME_ROOT="$(mktemp -d "$REPO_ROOT/target/oxide-door-dosemu2-XXXXXX")"
RUNTIME_DIRS=()
RUNTIME_REPORTS=()
RUNTIME_SERIAL_LOGS=()
MULTI_NODE="${OXIDE_DOOR_MULTI_NODE:-0}"
PTY_TIMEOUT_SECONDS=20
REPORT_TIMEOUT_SECONDS=20

cleanup_runtimes() {
  local rc=$?
  rm -rf "$RUNTIME_ROOT"
  return "$rc"
}

trap 'cleanup_runtimes' EXIT

if [[ "${OXIDE_DOOR_INTERACTIVE:-0}" != "1" ]]; then
  printf 'SKIP: set OXIDE_DOOR_INTERACTIVE=1 to run the interactive DOSEMU2 smoke test\n'
  exit 77
fi

if ! command -v "$DOSEMU_BIN" >/dev/null 2>&1; then
  printf 'SKIP: DOSEMU executable %s not found\n' "$DOSEMU_BIN"
  exit 77
fi

DOSEMU_VERSION_FIRST_LINE="$("$DOSEMU_BIN" --version 2>&1 | sed -n '1p' || true)"
if [[ "$DOSEMU_VERSION_FIRST_LINE" == dosemu-1.* ]]; then
  printf 'SKIP: DOSEMU2 is required; %s is legacy %s\n' "$DOSEMU_BIN" "$DOSEMU_VERSION_FIRST_LINE"
  exit 77
fi

if [[ ! -f "$DOOR_EXE" ]]; then
  printf 'missing %s\n' "$DOOR_EXE" >&2
  exit 1
fi

write_crlf_file() {
  local path="$1"
  shift
  {
    for line in "$@"; do
      printf '%s\r\n' "$line"
    done
  } > "$path"
}

wait_for_file() {
  local path="$1"
  local timeout_seconds="$2"
  local elapsed_ms=0
  while [[ ! -e "$path" && $elapsed_ms -lt $((timeout_seconds * 10)) ]]; do
    sleep 0.1
    elapsed_ms=$((elapsed_ms + 1))
  done

  [[ -e "$path" ]]
}

run_node_smoke() {
  local node_id="$1"
  local runtime_dir="$2"

  local pty_path="$runtime_dir/OXCOM1.PTY"
  local conf_path="$runtime_dir/OXDOSEMU2.CONF"
  local dorinfo_path="$runtime_dir/DORINFO1.DEF"
  local oxnode_path="$runtime_dir/OXNODE.TXT"
  local runtime_exe="$runtime_dir/OXIDECHK.EXE"
  local stdin_fifo="$runtime_dir/dosemu.stdin"
  local serial_log="$runtime_dir/serial.log"
  local report_path="$runtime_dir/OXIDECHK.RPT"
  local door_log="$runtime_dir/dosemu.log"
  local dosemu_pid=""
  local reader_pid=""
  local stdin_fd=""
  local pty_open=0

  cleanup_node() {
    if [[ "$pty_open" -eq 1 ]]; then
      exec 3<&- 4>&- || true
      pty_open=0
    fi

    if [[ -n "$stdin_fd" ]]; then
      eval "exec ${stdin_fd}>&-" || true
      stdin_fd=""
    fi

    if [[ -n "$reader_pid" ]]; then
      kill "$reader_pid" >/dev/null 2>&1 || true
      wait "$reader_pid" >/dev/null 2>&1 || true
      reader_pid=""
    fi

    if [[ -n "$dosemu_pid" ]]; then
      kill "$dosemu_pid" >/dev/null 2>&1 || true
      wait "$dosemu_pid" >/dev/null 2>&1 || true
      dosemu_pid=""
    fi
  }

  trap 'cleanup_node' RETURN

  rm -rf "$runtime_dir"
  mkdir -p "$runtime_dir"
  rm -f "$report_path"
  cp "$DOOR_EXE" "$runtime_exe"
  mkfifo "$stdin_fifo"

  write_crlf_file "$dorinfo_path" \
    "OxideBBS" \
    "Sysop" \
    "COM1" \
    "38400 BAUD,N,8,1" \
    "0" \
    "Caller-$node_id" \
    "Caller" \
    "Localhost" \
    "100" \
    "30"

  write_crlf_file "$oxnode_path" "node=$node_id"

  cat > "$conf_path" <<EOCONF
\$_cpu_vm = "emulated"
\$_cpu_vm_dpmi = "emulated"
\$_sound = (off)
\$_mouse_internal = (off)
\$_joy_device = ""
\$_pktdriver = (off)
\$_tcpdriver = (off)
\$_ttylocks = ""
\$_com1 = "pts $pty_path"
EOCONF

  printf 'Launching OXIDECHK.EXE under DOSEMU2 for node %d\n' "$node_id"
  "$DOSEMU_BIN" -dumb -quiet -K "$runtime_dir" -f "$conf_path" -E OXIDECHK.EXE <"$stdin_fifo" >"$door_log" 2>&1 &
  dosemu_pid=$!
  exec {stdin_fd}>"$stdin_fifo"

  if ! wait_for_file "$pty_path" "$PTY_TIMEOUT_SECONDS"; then
    printf 'node %d: timed out waiting for %s\n' "$node_id" "$pty_path" >&2
    return 1
  fi

  if command -v stty >/dev/null 2>&1; then
    stty -F "$pty_path" raw -echo -icanon -isig -icrnl -ixon -ixoff min 1 time 0 >/dev/null 2>&1 || true
  fi

  exec 3<>"$pty_path"
  exec 4<>"$pty_path"
  pty_open=1
  cat <&3 > "$serial_log" 2>/dev/null &
  reader_pid=$!

  sleep 0.5
  printf 'I\r' >&4
  sleep 0.5
  printf 'R\r' >&4
  sleep 0.5
  printf 'Q\r' >&4

  if ! wait_for_file "$report_path" "$REPORT_TIMEOUT_SECONDS"; then
    return 1
  fi

  if ! grep -a -q 'Oxide Door Check' "$serial_log"; then
    printf 'node %d: expected output not observed in %s\n' "$node_id" "$serial_log" >&2
    return 1
  fi

  RUNTIME_DIRS+=("$runtime_dir")
  RUNTIME_REPORTS+=("$report_path")
  RUNTIME_SERIAL_LOGS+=("$serial_log")

  return 0
}

printf 'Using DOSEMU binary: %s\n' "$DOSEMU_BIN"

if [[ "$MULTI_NODE" == "1" ]]; then
  printf 'Running multi-node DOSEMU2 smoke test\n'
  run_node_smoke 1 "$RUNTIME_ROOT/node-001"
  run_node_smoke 2 "$RUNTIME_ROOT/node-002"

  if [[ ${#RUNTIME_REPORTS[@]} -ne 2 ]]; then
    printf 'expected two node reports, got %d\n' "${#RUNTIME_REPORTS[@]}" >&2
    exit 1
  fi

  if [[ ! -f "${RUNTIME_REPORTS[0]}" || ! -f "${RUNTIME_REPORTS[1]}" ]]; then
    printf 'missing node report output\n' >&2
    exit 1
  fi

  if [[ "${RUNTIME_REPORTS[0]}" == "${RUNTIME_REPORTS[1]}" ]]; then
    printf 'node reports overlapped same path\n' >&2
    exit 1
  fi

  if ! grep -a -q 'Oxide Door Check' "${RUNTIME_SERIAL_LOGS[0]}" || ! grep -a -q 'Oxide Door Check' "${RUNTIME_SERIAL_LOGS[1]}"; then
    printf 'one or more nodes did not emit expected output\n' >&2
    exit 1
  fi

  printf 'multi-node DOSEMU2 smoke test passed\n'
else
  run_node_smoke 1 "$RUNTIME_ROOT/node-001"
  printf 'DOSEMU2 smoke test passed\n'
fi
