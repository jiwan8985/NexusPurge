#!/usr/bin/env bash
set -euo pipefail

# [정리] all / cache

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
source "$SCRIPT_DIR/../lib/output.sh"
cd "$ROOT"

sub="${1:-help}"

case "$sub" in
  all)
    step "dist/ + .vite/ + src-tauri/target/ 삭제"
    for p in dist .vite src-tauri/target; do
      if [[ -e "$ROOT/$p" ]]; then
        rm -rf "$ROOT/$p"
        ok "삭제: $p"
      fi
    done
    ok "clean 완료"
    ;;

  cache)
    step "dist/ + .vite/ 삭제  (target 보존)"
    for p in dist .vite; do
      if [[ -e "$ROOT/$p" ]]; then
        rm -rf "$ROOT/$p"
        ok "삭제: $p"
      fi
    done
    ok "cache clean 완료"
    ;;

  *)
    echo "clean.sh: unknown sub-command '$sub' (all|cache)" >&2
    exit 1
    ;;
esac
