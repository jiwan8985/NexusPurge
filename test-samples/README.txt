NexusPurge — Purge 테스트 샘플 파일
=====================================

[파일 목록]

purge-test-01.txt  : CDN Purge 기본 동작 확인
purge-test-02.txt  : Overwrite 후 자동 Purge 확인
purge-test-03.txt  : 개별(선택) Purge 확인
purge-test-04.txt  : 일괄(전체) Purge 확인 - 01~05 함께 사용
purge-test-05.txt  : 일괄(전체) Purge 확인 - 01~05 함께 사용
overwrite-target.txt    : 덮어쓰기 테스트 (Version 1 먼저 업로드)
overwrite-target-v2.txt : 덮어쓰기 테스트 (Version 2로 재업로드)
skip-target.txt    : Skip 동작 확인 (동일 파일 재업로드 시 건너뜀)

[기본 테스트 순서]

1. 기본 업로드 & Purge
   - purge-test-01~05.txt 모두 선택 → 업로드
   - 업로드 완료 후 "선택 Purge" 실행
   - LogPanel CDN Purge 결과 확인

2. 자동 Purge (Overwrite)
   - overwrite-target.txt 업로드
   - overwrite-target-v2.txt 파일명을 overwrite-target.txt로 변경 후 재업로드
   - Smart Sync가 "교체" 분류 + 자동 Purge 실행 확인

3. Skip 확인
   - skip-target.txt 업로드
   - 수정 없이 동일 파일 재업로드
   - LogPanel에 "건너뜀" 표시 확인

4. 전체 경로 Purge
   - 위 파일 전부 업로드
   - "전체 Purge" 버튼으로 현재 경로 일괄 무효화
