#!/usr/bin/env bash
set -euo pipefail

DOSBOX_BIN="${DOSBOX_BIN:-dosbox}"
XVFB_SERVER_ARGS="${XVFB_SERVER_ARGS:--screen 0 1024x768x24}"

if ! command -v xvfb-run >/dev/null 2>&1; then
  printf 'xvfb-run not found; install Xvfb to run DOSBox without a visible window\n' >&2
  exit 127
fi

if ! command -v "$DOSBOX_BIN" >/dev/null 2>&1; then
  printf 'DOSBox executable %s not found\n' "$DOSBOX_BIN" >&2
  exit 127
fi

export SDL_AUDIODRIVER="${SDL_AUDIODRIVER:-dummy}"

exec xvfb-run -a -s "$XVFB_SERVER_ARGS" "$DOSBOX_BIN" "$@"
