# NexusPurge — 2026-07-14 이후 진행 내역

`docs/WBS.md`의 마지막 갱신(2026-07-14 17:03, 커밋 `9b96a40`) 이후 진행된 작업만 정리한 문서입니다.
Phase 0~8 및 WBS 요약표는 `docs/WBS.md` 참고. 형식은 기존 WBS를 따릅니다.

---

## 1. 요약

| 항목 | 내용 |
|---|---|
| 대상 기간 | 2026-07-15 ~ 2026-07-22(현재, 커밋 미완료 작업 포함) |
| 커밋 수 | 9건 (`73f53ff` ~ `76709cb`) + 작업 중인 변경사항(미커밋) 1건 |
| 주요 테마 | Purge 실패 원인 추적성 강화, 프로필 인증정보 보안(keyring 이전), 로그 보관정책, 다중 인스턴스 안전성, 감사 로그 상세도 제어, 네트워크 상태 가시화 |

---

## 2. 완료된 작업 (커밋 완료)

| 코드 | 작업 항목 | 세부 내용 
|---|---|---|
| S.1 | CDN Purge 요청 단계 추적 | Purge 1건 내 실제 발생한 개별 HTTP 호출을 순서대로 캡처해 `cdn-*.log`에 단계별 상태코드·소요시간·응답 요약 기록 → 실패 원인을 로그 한 줄이 아닌 호출 단계 단위로 추적 가능 |
| S.2 | 대량 파일 로그 요약화 | 배치 파일 수 30건 초과 시 파일별 라인 대신 상태별 건수 + 시간범위로 요약, 실패/기타 상태 파일만 최대 50건 개별 나열 (전체 목록은 `operation_logs.json`에 무제한 보관) |
| S.3 | 우클릭 속성 다이얼로그 S3 전용화 | 고객사 요청으로 "속성" 다이얼로그에서 CDN 정보 섹션 제거, S3 기본정보 + HeadObject 응답 헤더만 표시 |
| S.4 | 로그 파일 30일 보관정책 | `logs/` 폴더의 system/transfer/cdn-*.log가 무한정 쌓이던 문제 해결. 앱 시작 시 30일 지난 typed 로그 자동 삭제(`cleanup_old_logs`), audit-*.log는 별도 관리 |
| S.5 | 프로필 인증정보 keyring 이전 | 기존 시크릿과 동일하게 OS keyring으로 이전, 앱 시작 시 레거시 평문 값 자동 마이그레이션. `.nexprofile` 가져오기 응답에서도 복호화된 인증값을 전부 제거(프론트엔드 미사용 값의 렌더러 프로세스 노출 차단) |
| S.7 | 다중 인스턴스 프로필 저장 잠금 | 두 NexusPurge 인스턴스가 동시에 프로필을 저장/삭제해도 유실되지 않도록 `profiles.json`에 크로스 프로세스 파일 잠금 + 원자적 쓰기 적용 (`fs4` 크레이트 도입) |

---

## 3. 진행 중 (미커밋 — 현재 작업 트리)

| 코드 | 작업 항목 | 세부 내용 | 산출물 | 상태 |
|---|---|---|---|---|
| W.1 | 감사 로그 상세 레벨 설정 | `audit-*.log`에 CDN 응답 본문을 남길지 여부를 설정 화면에서 토글 가능하게 변경. 기본은 요약 모드(메서드·URL·상태코드·소요시간만), 켜면 기존처럼 응답 본문 1,000자까지 기록. `AppSettings`에 `detailedAuditLog` 필드 추가, 앱 시작 시 저장된 값으로 초기화 | `utils/audit_level.rs`(신규), `commands/s3.rs::save_detailed_audit_log`/`get_app_settings`, `SettingsModal.tsx` | 🔧 작업 중 |
| W.2 | 네트워크 상태 위젯 | 상태바에 업로드/다운로드 속도를 분리 표시(기존엔 합산 1개 값)하고, 활성 S3 요청 수 · 최근 20건 평균 RTT를 2초 간격 이벤트(`network:stats`)로 push해 표시 | `utils/network_stats.rs`(신규, RAII `RttGuard`), `lib.rs`, `StatusBar.tsx`, `appStore.ts` | 🔧 작업 중 |
| W.3 | 로그 파일 압축 로테이션 | `cleanup_old_logs`를 확장해 오늘 날짜가 아닌 typed 로그(system/transfer/cdn-*.log)를 `.log.gz`로 자동 압축, 30일 경과분은 `.log`/`.log.gz` 상관없이 삭제 (`flate2` 크레이트 도입) — 디스크 사용량 절감 | `operation_log.rs::compress_log_file` | 🔧 작업 중 |
| W.4 | AppSettings read-modify-write 전환 | 기존엔 `last_profile_id` 저장 시 `AppSettings` 전체를 새로 만들어 덮어써서 다른 설정 필드가 유실될 수 있었던 구조적 결함을 `read_settings`/`write_settings` 기반 read-modify-write로 수정 (W.1의 `detailedAuditLog` 필드 추가를 계기로 발견) | `utils/config.rs` | 🔧 작업 중 |

> 커밋 전 상태이므로 세부 내용은 이후 커밋 시점에 변경될 수 있습니다.

---

## 4. 참고

- 이번 구간 작업은 대부분 **Purge 실패 진단성**(S.1, S.2)과 **보안/안정성 하드닝**(S.4~S.7, W.3, W.4)에 집중되어 있으며, `docs/WBS.md` Phase 9의 잔여 과제(효성 대용량 Purge, 고객 S3 로그 적재)와는 별개로 발생한 추가 개선입니다.
- W.1/W.2는 `docs/FEATURE_BACKLOG_INFRA.md`에서 논의된 인프라팀 관점 요구(운영 가시성 강화)에 대응하는 첫 구현으로 보입니다 — 백로그 문서와 교차 확인 권장.
