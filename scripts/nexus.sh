#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="$ROOT_DIR/.logs"

usage() {
  cat <<'EOF'
NexusPurge helper

Usage:
  ./scripts/nexus.sh <command> [args]

Commands:
  install             Install frontend dependencies with pnpm
  dev                 Start the Vite dev server in the background
  tauri               Start the full Tauri desktop app in the background
  preview             Start the Vite preview server in the background
  stop [name|all]     Stop dev, tauri, preview, or all managed processes
  restart <name>      Restart dev, tauri, or preview
  status              Show managed process status
  logs [name] [-f]    Show logs for dev, tauri, preview, or latest log
  build               Run pnpm build
  tauri-build         Run pnpm tauri build
  test                Run pnpm test
  check               Run pnpm build and cargo test
  cargo-check         Run cargo check for the Tauri backend
  cargo-test          Run cargo test for the Tauri backend
  aws-check           Validate AWS identity and S3/CloudFront permissions
  localstack          Run the LocalStack integration script
  clean-logs          Remove managed log and pid files
  help                Show this help

Examples:
  ./scripts/nexus.sh install
  ./scripts/nexus.sh tauri
  ./scripts/nexus.sh logs tauri -f
  ./scripts/nexus.sh stop all
  ./scripts/nexus.sh aws-check --bucket my-bucket --region ap-northeast-2 --write-probe
EOF
}

ensure_log_dir() {
  mkdir -p "$LOG_DIR"
}

require_pnpm() {
  if command -v pnpm >/dev/null 2>&1; then
    return 0
  fi

  if command -v corepack >/dev/null 2>&1; then
    corepack enable pnpm
  fi

  if ! command -v pnpm >/dev/null 2>&1; then
    echo "pnpm is required. Install pnpm or enable it with corepack." >&2
    exit 1
  fi
}

pid_file() {
  printf '%s/nexus-%s.pid' "$LOG_DIR" "$1"
}

log_file() {
  printf '%s/nexus-%s.log' "$LOG_DIR" "$1"
}

is_running() {
  local pid="$1"
  [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1
}

read_pid() {
  local file
  file="$(pid_file "$1")"
  [[ -f "$file" ]] && tr -d '[:space:]' < "$file"
}

start_managed() {
  local name="$1"
  shift
  ensure_log_dir
  require_pnpm

  local pid
  pid="$(read_pid "$name" || true)"
  if is_running "$pid"; then
    echo "$name is already running: pid=$pid"
    echo "log: $(log_file "$name")"
    return 0
  fi

  (
    cd "$ROOT_DIR"
    nohup "$@" >"$(log_file "$name")" 2>&1 &
    echo $! >"$(pid_file "$name")"
  )

  pid="$(read_pid "$name")"
  echo "started $name: pid=$pid"
  echo "log: $(log_file "$name")"
}

stop_one() {
  local name="$1"
  local pid
  pid="$(read_pid "$name" || true)"

  if ! is_running "$pid"; then
    rm -f "$(pid_file "$name")"
    echo "$name is not running"
    return 0
  fi

  if command -v pkill >/dev/null 2>&1; then
    pkill -TERM -P "$pid" >/dev/null 2>&1 || true
  fi
  kill "$pid" >/dev/null 2>&1 || true
  sleep 1

  if is_running "$pid"; then
    if command -v pkill >/dev/null 2>&1; then
      pkill -KILL -P "$pid" >/dev/null 2>&1 || true
    fi
    kill -9 "$pid" >/dev/null 2>&1 || true
  fi

  rm -f "$(pid_file "$name")"
  echo "stopped $name"
}

stop_target() {
  local target="${1:-all}"
  case "$target" in
    all)
      stop_one dev
      stop_one tauri
      stop_one preview
      ;;
    dev|tauri|preview)
      stop_one "$target"
      ;;
    *)
      echo "unknown process: $target" >&2
      exit 1
      ;;
  esac
}

show_status() {
  ensure_log_dir
  local name pid
  for name in dev tauri preview; do
    pid="$(read_pid "$name" || true)"
    if is_running "$pid"; then
      printf '%-8s running pid=%s log=%s\n' "$name" "$pid" "$(log_file "$name")"
    else
      printf '%-8s stopped\n' "$name"
    fi
  done
}

show_logs() {
  ensure_log_dir
  local name="${1:-}"
  local follow="${2:-}"

  if [[ "$name" == "-f" || -z "$name" ]]; then
    follow="$name"
    name=""
  fi

  local file
  if [[ -n "$name" ]]; then
    file="$(log_file "$name")"
  else
    file="$(find "$LOG_DIR" -maxdepth 1 -name 'nexus-*.log' -type f -print0 2>/dev/null | xargs -0 ls -t 2>/dev/null | head -n 1 || true)"
  fi

  if [[ -z "${file:-}" || ! -f "$file" ]]; then
    echo "no log file found" >&2
    exit 1
  fi

  echo "log: $file"
  if [[ "$follow" == "-f" || "$follow" == "--follow" ]]; then
    tail -n 120 -f "$file"
  else
    tail -n 120 "$file"
  fi
}

run_in_root() {
  cd "$ROOT_DIR"
  "$@"
}

command="${1:-help}"
shift || true

case "$command" in
  install)
    require_pnpm
    run_in_root pnpm install
    ;;
  dev)
    start_managed dev pnpm run dev
    ;;
  tauri)
    start_managed tauri pnpm tauri dev
    ;;
  preview)
    start_managed preview pnpm run preview
    ;;
  stop)
    stop_target "${1:-all}"
    ;;
  restart)
    target="${1:-}"
    if [[ "$target" != "dev" && "$target" != "tauri" && "$target" != "preview" ]]; then
      echo "restart requires dev, tauri, or preview" >&2
      exit 1
    fi
    stop_one "$target"
    "$0" "$target"
    ;;
  status)
    show_status
    ;;
  logs)
    show_logs "${1:-}" "${2:-}"
    ;;
  build)
    require_pnpm
    run_in_root pnpm run build
    ;;
  tauri-build)
    require_pnpm
    run_in_root pnpm tauri build
    ;;
  test)
    require_pnpm
    run_in_root pnpm test
    ;;
  check)
    require_pnpm
    run_in_root pnpm run build
    run_in_root cargo test --manifest-path src-tauri/Cargo.toml
    ;;
  cargo-check)
    run_in_root cargo check --manifest-path src-tauri/Cargo.toml
    ;;
  cargo-test)
    run_in_root cargo test --manifest-path src-tauri/Cargo.toml
    ;;
  aws-check)
    run_in_root bash scripts/aws-permission-check.sh "$@"
    ;;
  localstack)
    run_in_root bash scripts/localstack-integration.sh
    ;;
  clean-logs)
    rm -f "$LOG_DIR"/nexus-*.log "$LOG_DIR"/nexus-*.pid 2>/dev/null || true
    echo "removed managed logs"
    ;;
  help|-h|--help)
    usage
    ;;
  *)
    echo "unknown command: $command" >&2
    usage >&2
    exit 1
    ;;
esac
