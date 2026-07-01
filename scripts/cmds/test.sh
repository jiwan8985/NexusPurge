#!/usr/bin/env bash
set -euo pipefail

# [테스트] run / watch / rust / all

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
source "$SCRIPT_DIR/../lib/output.sh"
cd "$ROOT"

sub="${1:-help}"

case "$sub" in
  run)
    step "Vitest"
    pnpm test
    ;;

  watch)
    step "Vitest (감시 모드)"
    pnpm test:watch
    ;;

  rust)
    step "cargo test"
    cargo test --manifest-path src-tauri/Cargo.toml
    ;;

  all)
    step "Vitest"
    pnpm test
    step "cargo test"
    cargo test --manifest-path src-tauri/Cargo.toml
    ok "전체 테스트 완료"
    ;;

  *)
    echo "test.sh: unknown sub-command '$sub' (run|watch|rust|all)" >&2
    exit 1
    ;;
esac
