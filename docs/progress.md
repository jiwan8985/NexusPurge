# NexusPurge 개발 진행 현황

> 기준일: 2026-06-18 / 최신 커밋: `a75dbf7`

---

## 개발 완료 요약

| Phase | 내용 | 상태 |
|-------|------|------|
| Phase 1 | 기본 구조, 프로필, UI | ✅ 완료 |
| Phase 2 | 업로드 / 다운로드 | ✅ 완료 |
| Phase 3 | Purge | ✅ 완료 |
| Phase 4 | 운영 기능 | ✅ 완료 |
| Phase 5 | 확장 기능 | ⏳ 미착수 |

---

## Phase 1 — 기본 구조

### 완료 항목

- **프로젝트 골격**
  - Tauri 2 (Rust) + React + TypeScript + Vite + Zustand 구성
  - FTP 스타일 좌(로컬) / 우(S3 원격) 듀얼 패널 레이아웃
  - Toolbar, TitleBar, StatusBar, LogPanel 레이아웃 컴포넌트

- **테마 시스템**
  - 다크 / 라이트 / 시스템 3단계 순환 테마
  - `data-theme` 어트리뷰트 + CSS custom property 방식
  - `@media (prefers-color-scheme: dark)` 시스템 테마 자동 연동
  - TitleBar 우측 테마 토글 버튼 (SVG 아이콘 sun / moon / monitor)

- **프로필 관리 (ProfileModal)**
  - 여러 프로필 추가 / 수정 / 삭제
  - 프로필 검색 (이름 / 버킷 실시간 필터)
  - S3 접속 정보, CDN Provider / Distribution ID, 기본 Cache-Control 설정
  - `secretAccessKey` OS Keyring(Windows Credential Manager / macOS Keychain) 저장
  - 메타데이터: `~/.local/share/cdn-upload-tool/profiles.json`

- **암호화 프로필 Import / Export**
  - `.nexprofile` 포맷 (AES-256-GCM + PBKDF2-HMAC-SHA256)
  - 비밀번호 기반 암호화 — Credential 포함 전체 프로필을 한 파일로 공유
  - Import 시 패스워드 입력 다이얼로그

- **로그 패널**
  - 3탭: 작업 로그 / 전송 큐 / 실패 항목
  - 로그 복사 / 파일 저장 / 지우기

- **개발 스크립트**
  - `scripts/nexus.ps1` / `nexus.sh` — install / dev / build / check / test / help 5개 명령

---

## Phase 2 — 업로드 / 다운로드

### 완료 항목

- **S3 업로드 (Smart Sync)**
  - 로컬 파일 크기 vs S3 ETag 크기 비교로 업로드 필요 여부 판단
  - `tokio::try_join!` 병렬 로컬 수집 + S3 목록 조회 → 빠른 미리보기
  - 분류: `toUpload` (신규) / `toOverwrite` (변경) / `toSkip` (동일)
  - `tokio::JoinSet` + `Semaphore`로 병렬 업로드 (동시 수 설정 가능)
  - Drag & Drop 업로드 지원

- **동기화 미리보기 (SyncPreviewDialog)**
  - 업로드 전 신규 / 변경 / 동일 파일 수 확인 팝업
  - MD5 없이 size 비교만 하여 수만 개 파일도 빠른 결과

- **업로드 옵션 모달 (UploadOptionsModal)**
  - Content-Type 프리셋 (auto / HTML / JSON / JS / CSS / PNG / JPEG / WebP / octet-stream)
  - Cache-Control 프리셋 (1년 immutable / 1일 / 1시간 / no-cache / no-store)
  - 커스텀 HTTP 헤더 (Key-Value 동적 추가)
  - S3 메타데이터 (Key-Value 동적 추가)
  - 초기화 버튼으로 기본값 복원

- **다운로드**
  - 원격 패널 선택 파일 → 로컬 폴더 선택 다이얼로그 후 다운로드
  - OS 경로 구분자 자동 처리 (Windows `\` / Unix `/`)

- **고객 S3 버킷 로그 적재**
  - `{prefix}/{yyyy-MM-dd}/{id}.json` 형식으로 JSON 업로드
  - 업로드 / 다운로드 / Purge 결과 포함
  - `OperationLog` 구조체 — 파일별 성공/실패, 시작/완료 시각

---

## Phase 3 — Purge

### 완료 항목

- **수동 Purge UI**
  - Toolbar "선택 Purge" 버튼: 원격 패널 선택 파일 경로 Purge
  - Toolbar "전체 Purge" 버튼: 현재 원격 경로 전체(`{path}/*`) Purge
  - CDN Provider가 설정된 프로필 연결 시에만 버튼 표시

- **PurgeDialog (확인 팝업)**
  - Purge 대상 경로 최대 8개 미리보기, 초과 시 "+N개 더" 표시
  - 경고 기준 초과 시 주황 배너, 배치 크기 초과 시 빨간 배너
  - 비동기 확인 처리 (Purge 중 로딩 상태)

- **자동 Purge**
  - 업로드 후 `toOverwrite` 항목 자동 CDN Purge
  - Toolbar 자동 Purge 토글 스위치 (켜면 신규 업로드도 Purge)
  - CloudFront: `InvalidationBatch` API, `caller_reference`로 멱등성 보장

- **CDN URL 반영 확인**
  - 업로드 완료 후 `verify_cdn_urls` 커맨드로 HTTP 응답 코드 확인
  - 전송 항목 상세에 CDN 반영 여부 / 상태 코드 표시

- **CloudFront Invalidation 상태 추적**
  - `get_purge_status` 커맨드로 Invalidation 완료 여부 폴링
  - 완료 여부에 따라 전송 항목 `cdnPurgeStatus` 업데이트

- **CDN 어댑터 구조**
  - `CdnAdapter` trait (`base.rs`)
  - CloudFront 구현체 (`cloudfront.rs`)
  - 미구현 CDN (Akamai, LG U+ 등) → Err 반환 + 로그 기록

---

## Phase 4 — 운영 기능

### 완료 항목

- **배치 / 성능 설정 (`SettingsModal` → "전송 성능" 섹션)**

  | 설정 항목 | 기본값 | 설명 |
  |-----------|--------|------|
  | 동시 전송 수 | 4 | 업로드/다운로드 병렬 처리 개수 (1~16) |
  | 파일 수 경고 기준 | 5,000 | 이 개수 이상 선택 시 주의 확인 창 |
  | 파일 수 제한 기준 | 10,000 | 이 개수 이상 선택 시 강한 경고 |
  | 대용량 파일 기준 (MB) | 100 | 이 크기 이상 업로드 시 확인 창 |
  | Purge 경고 기준 | 1,000 | 이 경로 수 이상 Purge 시 주의 표시 |
  | Purge 배치 크기 | 1,000 | 한 번 API 호출에 포함할 최대 경로 수 |

  - `src/utils/batch-settings.ts`로 중앙 관리 (localStorage 저장)
  - "기본값으로" 버튼 일괄 초기화

- **동적 임계값 적용**
  - `TransferButtons`, `PurgeDialog`, `usePurge`, `useTransfer` 모두 설정값 참조
  - Rust `upload_files` / `start_uploads` / `start_downloads` — `max_concurrent_files` 파라미터 수신 (1~32 clamp)

- **로그 레벨 필터**
  - 작업 로그 탭 상단에 필터 버튼: 전체 / 경고+ / 오류
  - 오류 개수 배지를 로그 탭 옆에 표시

- **실패 항목 탭 (전송 큐 분리)**
  - `status === "error"` 전송만 별도 탭에 표시
  - 실패 건수 배지 (빨간색)

- **실패 전송 재시도**
  - 실패 항목 우측 "재시도" 버튼
  - `useTransfer.retryTransfer(item)` → 단일 항목을 `pending`으로 재설정 후 재전송
  - 업로드/다운로드 방향 모두 지원

---

## 미완료 / Phase 5 예정

| 항목 | 비고 |
|------|------|
| Prefetch | 우선순위 낮음 |
| 파일 비교 고도화 | 현재 size 비교; MD5 정밀 비교 옵션 추가 가능 |
| AI LB 프로필 생성 시스템 연동 | 외부 시스템 연동 필요 |
| SSO / Vault 연동 | 외부 인증 시스템 연동 필요 |
| 자동 패치 (무중단 업데이트) | Tauri updater 연동 검토 필요 |
| Akamai / LG U+ / 효성 CDN 어댑터 | `CdnAdapter` trait 구조는 준비됨, 구현만 추가하면 됨 |
| Windows Server / Linux / Unix 지원 검증 | 빌드 및 설치 환경 테스트 필요 |

---

## 고객 확인 필요 사항 (미결)

- 로그 적재용 버킷 / Prefix / 보관 기간 / JSON 포맷 승인
- 대용량 기준 (5,000개 미권고 / 10,000개 제한) 운영 정책 최종 확인
- 프로필 단위 (프로젝트 단위 vs 사용자 단위) 결정
- 설치 방식 최종 결정 (설치형 / Portable)
- Windows Server 2019~2025 지원 범위 확인
- 자동 패치 강제 적용 여부
