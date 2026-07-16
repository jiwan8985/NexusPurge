#!/usr/bin/env bash
set -euo pipefail

# [개발] install / dev / check / fmt / clippy

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
source "$SCRIPT_DIR/../lib/output.sh"
cd "$ROOT"

sub="${1:-help}"

case "$sub" in
  install)
    step "pnpm install"
    pnpm install
    ;;

  dev)
    step "pnpm tauri dev"
    pnpm tauri dev
    ;;

  check)
    step "TypeScript typecheck"
    pnpm exec tsc --noEmit
    step "cargo check"
    cargo check --manifest-path src-tauri/Cargo.toml
    ok "check 완료"
    ;;

  fmt)
    step "cargo fmt"
    cargo fmt --manifest-path src-tauri/Cargo.toml
    ok "포맷 완료"
    ;;

  clippy)
    step "cargo clippy"
    cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
    ;;

  *)
    echo "dev.sh: unknown sub-command '$sub' (install|dev|check|fmt|clippy)" >&2
    exit 1
    ;;
esac
