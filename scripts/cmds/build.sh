#!/usr/bin/env bash
set -euo pipefail

# [빌드] fe / debug / release

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
source "$SCRIPT_DIR/../lib/output.sh"
cd "$ROOT"

sub="${1:-help}"

case "$sub" in
  fe)
    step "프론트엔드 빌드  (tsc + vite build)"
    pnpm build
    ok "dist/ 생성 완료"
    ;;

  debug)
    step "Tauri 디버그 빌드"
    pnpm tauri build --debug
    ok "src-tauri/target/debug/bundle/ 확인"
    ;;

  release)
    step "Tauri 릴리즈 빌드"
    pnpm tauri build
    ok "src-tauri/target/release/bundle/ 확인"
    ;;

  *)
    echo "build.sh: unknown sub-command '$sub' (fe|debug|release)" >&2
    exit 1
    ;;
esac
