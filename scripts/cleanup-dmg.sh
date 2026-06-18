#!/usr/bin/env bash
# 빌드 전 hdiutil이 남긴 스테일 임시 볼륨을 정리합니다.
for v in /Volumes/dmg.*; do
  [ -d "$v" ] && hdiutil detach "$v" 2>/dev/null && echo "Detached $v"
done
exit 0
