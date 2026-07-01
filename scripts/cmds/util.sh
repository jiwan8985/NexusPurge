#!/usr/bin/env bash
set -euo pipefail

# [유틸리티] open-data / log

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
source "$SCRIPT_DIR/../lib/output.sh"
cd "$ROOT"

sub="${1:-help}"

case "$sub" in
  open-data)
    if [[ "$(uname)" == "Darwin" ]]; then
      data_dir="$HOME/Library/Application Support/com.nexuspurge.app"
      opener="open"
    else
      data_dir="$HOME/.local/share/com.nexuspurge.app"
      opener="xdg-open"
    fi

    if [[ ! -d "$data_dir" ]]; then
      echo "   데이터 폴더 없음: $data_dir"
      echo "   (앱을 한 번 실행하면 생성됩니다)"
    else
      step "데이터 폴더 열기: $data_dir"
      "$opener" "$data_dir"
    fi
    ;;

  log)
    step "git log (최근 20)"
    git log --oneline --graph --decorate -20
    ;;

  *)
    echo "util.sh: unknown sub-command '$sub' (open-data|log)" >&2
    exit 1
    ;;
esac
