#!/usr/bin/env bash
set -euo pipefail

# 루트 nexus(bash) 가 사용하는 macOS/Linux용 명령어 디스패처.
# 실제 실행 로직은 scripts/cmds/*.sh 에 그룹별로 분리되어 있음.
# Windows 대응 버전: nexus.ps1 -> scripts/nexus-core.ps1 -> scripts/cmds/*.ps1

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CMDS_DIR="$SCRIPT_DIR/cmds"

usage() {
  cat <<'EOF'
사용법: ./nexus <command>      (예: ./nexus dev, ./nexus test:watch)
       nexus <command>        (PATH 등록 시)

  [개발]
  install       pnpm install
  dev           pnpm tauri dev
  check         tsc --noEmit  +  cargo check
  fmt           cargo fmt  (Rust 코드 포맷)
  clippy        cargo clippy  (Rust 린트)

  [테스트]
  test          pnpm test  (Vitest)
  test:watch    pnpm test:watch  (감시 모드)
  test:rust     cargo test
  test:all      pnpm test  +  cargo test

  [빌드]
  build:fe      tsc + vite build  (프론트엔드만)
  build:debug   pnpm tauri build --debug
  build         pnpm tauri build  (릴리즈)

  [정리]
  clean         dist/ + .vite/ + src-tauri/target/ 삭제
  clean:cache   dist/ + .vite/ 만 삭제  (target 보존)

  [유틸리티]
  open:data     앱 데이터 폴더 열기  (profiles.json 위치)
  log           git log --oneline -20
EOF
}

command="${1:-help}"
[[ $# -gt 0 ]] && shift

case "$command" in
  help|-h|--help) usage; exit 0 ;;

  install)     exec "$CMDS_DIR/dev.sh"   install "$@" ;;
  dev)         exec "$CMDS_DIR/dev.sh"   dev "$@" ;;
  check)       exec "$CMDS_DIR/dev.sh"   check "$@" ;;
  fmt)         exec "$CMDS_DIR/dev.sh"   fmt "$@" ;;
  clippy)      exec "$CMDS_DIR/dev.sh"   clippy "$@" ;;

  test)        exec "$CMDS_DIR/test.sh"  run "$@" ;;
  test:watch)  exec "$CMDS_DIR/test.sh"  watch "$@" ;;
  test:rust)   exec "$CMDS_DIR/test.sh"  rust "$@" ;;
  test:all)    exec "$CMDS_DIR/test.sh"  all "$@" ;;

  build:fe)    exec "$CMDS_DIR/build.sh" fe "$@" ;;
  build:debug) exec "$CMDS_DIR/build.sh" debug "$@" ;;
  build)       exec "$CMDS_DIR/build.sh" release "$@" ;;

  clean)       exec "$CMDS_DIR/clean.sh" all "$@" ;;
  clean:cache) exec "$CMDS_DIR/clean.sh" cache "$@" ;;

  open:data)   exec "$CMDS_DIR/util.sh"  open-data "$@" ;;
  log)         exec "$CMDS_DIR/util.sh"  log "$@" ;;

  *)
    echo "unknown command: $command" >&2
    usage
    exit 1
    ;;
esac
