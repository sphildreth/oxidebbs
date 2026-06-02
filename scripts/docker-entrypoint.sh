#!/usr/bin/env bash
set -euo pipefail

APP_ROOT="/srv/oxidebbs"
SHARE_ROOT="/opt/oxidebbs/share"
CONFIG_PATH="${OXIDEBBS_CONFIG:-$APP_ROOT/config/oxidebbs.toml}"
DEFAULT_PASSWORD="change-this-before-going-live"
SYSOP_PASSWORD="${OXIDEBBS_SYSOP_PASSWORD:-$DEFAULT_PASSWORD}"

copy_tree_if_missing() {
  local source="$1"
  local destination="$2"

  if [[ -d "$destination" ]] && [[ -n "$(find "$destination" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
    return
  fi

  mkdir -p "$destination"
  cp -a "$source"/. "$destination"/
}

copy_config_examples() {
  mkdir -p "$APP_ROOT/config"
  cp -n "$SHARE_ROOT/config/"*.toml "$APP_ROOT/config/" 2>/dev/null || true
}

bootstrap_runtime_tree() {
  mkdir -p \
    "$APP_ROOT/config" \
    "$APP_ROOT/data" \
    "$APP_ROOT/doors" \
    "$APP_ROOT/logs" \
    "$APP_ROOT/runtime"

  copy_tree_if_missing "$SHARE_ROOT/assets" "$APP_ROOT/assets"
  copy_tree_if_missing "$SHARE_ROOT/tools" "$APP_ROOT/tools"
  copy_tree_if_missing "$SHARE_ROOT/scripts" "$APP_ROOT/scripts"
  copy_config_examples
}

first_boot_setup() {
  if [[ -f "$CONFIG_PATH" ]]; then
    return
  fi

  if [[ -z "$SYSOP_PASSWORD" ]]; then
    printf 'error: OXIDEBBS_SYSOP_PASSWORD is required for first boot\n' >&2
    exit 1
  fi

  if [[ "$SYSOP_PASSWORD" == "$DEFAULT_PASSWORD" ]]; then
    printf 'warning: using default Docker sysop password; set OXIDEBBS_SYSOP_PASSWORD before exposing this board\n' >&2
  fi

  local setup_args=(
    --data "$APP_ROOT/data/oxidebbs.ddb"
    setup
    --output "$CONFIG_PATH"
    --board-name "${OXIDEBBS_BOARD_NAME:-OxideBBS}"
    --sysop-alias "${OXIDEBBS_SYSOP_ALIAS:-sysop}"
    --sysop-password "$SYSOP_PASSWORD"
    --telnet-port "${OXIDEBBS_TELNET_PORT:-2323}"
    --nodes "${OXIDEBBS_NODES:-4}"
  )

  if [[ "${OXIDEBBS_ENABLE_TEST_DOOR:-1}" == "1" ]]; then
    setup_args+=(--enable-example-door)
  fi

  oxidebbs-server "${setup_args[@]}"
}

run_config_check() {
  if [[ "${OXIDEBBS_SKIP_CONFIG_CHECK:-0}" == "1" ]]; then
    return
  fi

  oxidebbs-server --config "$CONFIG_PATH" check
}

bootstrap_runtime_tree
first_boot_setup

case "${1:-}" in
  "")
    set -- serve
    ;;
esac

case "$1" in
  ansi|audit|check|config|db|doors|logs|messages|nodes|serve|setup|status|sysop|users)
    run_config_check
    exec oxidebbs-server --config "$CONFIG_PATH" "$@"
    ;;
  oxidebbs-server)
    shift
    exec oxidebbs-server "$@"
    ;;
  *)
    exec "$@"
    ;;
esac
