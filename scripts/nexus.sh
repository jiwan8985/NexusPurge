#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

case "${1:-help}" in
  install) pnpm install ;;
  dev)     pnpm tauri dev ;;
  build)   pnpm tauri build ;;
  check)
    pnpm exec tsc --noEmit
    cargo check --manifest-path src-tauri/Cargo.toml
    ;;
  test) pnpm test ;;
  help|-h|--help)
    echo "nexus.sh <install|dev|build|check|test>"
    ;;
  *)
    echo "unknown: $1" >&2; exit 1
    ;;
esac
