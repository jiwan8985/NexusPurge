# NexusPurge 보완/수정/추가 이슈 목록

> 분석 기준일: 2026-04-27  
> 최종 업데이트: 2026-04-28  
> 심각도: Critical / High / Medium / Low  
> 범위: React UI, Tauri IPC, Rust S3/CDN 백엔드, 프로필 저장소

---

## Critical [ 완료 ] ✅

### C-1. Smart Sync 업로드 후 신규 파일까지 CDN Purge가 실행됨 ✅

**파일**: `src/hooks/useTransfer.ts`, `src-tauri/src/commands/sync.rs`

프론트엔드는 `plan.toUpload`와 `plan.toOverwrite`를 합쳐 `start_uploads`로 전달하지만, 각 항목이 신규 업로드인지 덮어쓰기인지 구분하는 값이 없다. 백엔드 `start_uploads`는 업로드 성공 시 CDN 설정이 있으면 모든 항목에 Purge를 실행한다.

**영향**: 신규 파일에도 불필요한 CloudFront Invalidation이 발생해 비용과 API 사용량이 증가한다.

**수정 내용**
- `UploadItem`에 `isOverwrite` 필드 추가
- `plan.toOverwrite` 항목만 `isOverwrite: true`로 전달
- 백엔드는 `item.is_overwrite == true`인 경우에만 Purge 실행
- `s3.rs::upload_files`와 `sync.rs::start_uploads`의 역할 정리 완료

### C-2. 프로필 목록 상태가 훅 인스턴스별로 분리됨 ✅

**파일**: `src/hooks/useProfile.ts`, `src/App.tsx`, `src/components/layout/TitleBar.tsx`, `src/components/modals/ProfileModal.tsx`

`useProfile()` 내부가 `useState<S3Profile[]>`로 프로필 목록을 보관한다. `App`, `TitleBar`, `ProfileModal`이 각각 다른 훅 인스턴스를 만들기 때문에 `App`에서 `loadProfiles()`를 호출해도 `TitleBar`와 `ProfileModal`의 `profiles`는 자동으로 갱신되지 않는다.

**수정 내용**
- 프로필 목록을 Zustand 전역 상태로 이동
- `loadProfiles`, `saveProfile`, `deleteProfile` 후 전역 `profiles` 갱신
- 모달이 열릴 때도 목록 재조회

### C-3. S3 목록 페이지네이션 미구현 ✅

**파일**: `src-tauri/src/adapters/storage/s3.rs`, `src-tauri/src/commands/s3.rs`, `src/hooks/useS3.ts`

`ListObjectsV2`는 최대 1,000개만 반환하지만 `nextContinuationToken`을 사용한 추가 조회가 없다.

**수정 내용**
- 백엔드에서 `continuation-token` 반복 조회 구현 (`list_objects_all`)
- 1,000개 초과 오브젝트도 전체 반환

### C-4. Windows 다운로드 경로 조합 오류 ✅

**파일**: `src/hooks/useTransfer.ts`

다운로드 경로를 `local.path + "/" + fileName`으로 조합한다. Windows 경로에서는 `C:\folder/file.txt`처럼 혼합 구분자가 생긴다.

**수정 내용**
- `joinPath()` 헬퍼 구현 — `\` 포함 여부 감지 후 OS별 구분자 적용

---

## High [ 완료 ] ✅

### H-1. 툴바 파일 작업 버튼이 실제 기능과 연결되지 않음 ✅

**파일**: `src/components/layout/Toolbar.tsx`

`새 폴더`, `삭제`, `이름 변경` 버튼은 표시되지만 실제 선택 항목과 연결된 작업 흐름이 부족하다.

**수정 내용**
- 새 S3 폴더: `useS3.createDirectory()` 연결
- 로컬/S3 삭제: 선택 상태별 command 분기
- 이름 변경: S3는 CopyObject + DeleteObject(`rename_s3_object`), 로컬은 `rename_local_file` command 추가

### H-2. 파일 단위 동시 업로드 제한 없음 ✅

**파일**: `src-tauri/src/commands/sync.rs`, `src-tauri/src/commands/s3.rs`

**수정 내용**
- `tokio::sync::Semaphore`로 동시 파일 전송 4개 제한 (`MAX_CONCURRENT_FILES = 4`)

### H-3. 프로필 저장 시 연결 검증이 선택 사항으로도 제공되지 않음 ✅

**파일**: `src/components/modals/ProfileModal.tsx`, `src/hooks/useProfile.ts`

**수정 내용**
- "연결 테스트" 버튼 추가 (`test_s3_connection` command)
- 저장 전 연결 확인 UX 제공

### H-4. S3 HTTP 오류 응답을 정상 빈 목록처럼 처리할 수 있음 ✅

**파일**: `src-tauri/src/adapters/storage/s3.rs`

**수정 내용**
- `resp.status().is_success()` 확인 후 실패 시 status/body 포함 오류 반환

### H-5. S3 XML 응답 파서가 특수문자와 XML entity를 처리하지 않음 ✅

**파일**: `src-tauri/src/adapters/storage/s3.rs`

**수정 내용**
- `xml_unescape()` 함수 추가 — `&amp;` `&lt;` `&gt;` `&quot;` `&apos;` 디코딩

### H-6. CloudFront 외 CDN이 UI에서 선택 가능하지만 미구현 ✅

**파일**: `src/components/modals/ProfileModal.tsx`, `src-tauri/src/adapters/cdn/mod.rs`

**수정 내용**
- Akamai EdgeGrid 인증 + Purge API 구현
- LG U+/효성 ITX는 UI에서 비활성화 처리

### H-7. 앱 재시작 시 마지막 프로필 복원 없음 ✅

**파일**: `src/hooks/useProfile.ts`, `src/store/appStore.ts`

**수정 내용**
- `lastProfileId`를 `profiles.json`에 저장 (`save_last_profile_id` / `get_last_profile_id` command)
- 앱 시작 시 마지막 프로필 자동 선택 복원

---

## Medium [ 완료 ] ✅

### M-1. ProfileModal의 한글 문자열과 UX 정리 ✅

**파일**: `src/components/modals/ProfileModal.tsx`

**수정 내용**
- 모든 UI 문자열 정상 한글로 교체 ("프로파일" → "프로필" 등)
- 삭제 확인 ConfirmDialog 도입

### M-2. 삭제 확인에 브라우저 기본 `confirm()` 사용 ✅

**파일**: `src/components/panels/RemotePanel.tsx`

**수정 내용**
- 공용 `ConfirmDialog` 컴포넌트 신규 작성 (`src/components/common/ConfirmDialog.tsx`)
- RemotePanel, ProfileModal, TitleBar, TransferButtons에 적용

### M-3. 전송 중 앱 종료 경고 없음 ✅

**파일**: `src/components/layout/TitleBar.tsx`

**수정 내용**
- `isTransferring` 상태에서 닫기 클릭 시 ConfirmDialog 표시
- 확인 후에만 `appWindow.close()` 호출

### M-4. 대용량 업로드 확인 및 예상 비용 안내 없음 ✅

**파일**: `src/components/transfer/TransferButtons.tsx`

**수정 내용**
- 선택 파일 총 크기 계산
- 100MB 초과 시 ConfirmDialog 표시 (파일 수, 총 용량 안내)

### M-5. 재시도 로직 없음 ✅

**파일**: `src-tauri/src/adapters/storage/s3.rs`, `src-tauri/src/adapters/cdn/cloudfront.rs`

**수정 내용**
- `utils/retry.rs` — `is_retryable_status()` (429/500/502/503/504)
- 지수 백오프 최대 3회 재시도 (500ms → 1s → 2s)
- `list_objects_page`, `upload_single`, `upload_part`, CloudFront `create_invalidation`에 적용

### M-6. ErrorBoundary 없음 ✅

**파일**: `src/App.tsx`

**수정 내용**
- `src/components/ErrorBoundary.tsx` (class component) 신규 작성
- `getDerivedStateFromError` + 재시도 버튼 fallback UI
- `App.tsx` 최상위에 래핑

### M-7. 다운로드 대상 경로 선택 다이얼로그 없음 ✅

**파일**: `src/hooks/useTransfer.ts`

**수정 내용**
- `@tauri-apps/plugin-dialog` 설치
- `startDownload()` 시작 전 `open({ directory: true })` 호출
- 사용자 취소 시 다운로드 중단

### M-8. 빈 업로드 계획 처리 UX 부족 ✅

**파일**: `src/hooks/useTransfer.ts`

**수정 내용**
- `toUpload.length === 0 && toOverwrite.length === 0` 시 ProgressDialog 미표시
- "모든 파일이 최신 상태입니다." 로그 출력 후 종료

### M-9. 로컬 디렉터리 순회에서 심볼릭 링크 처리 정책 없음 ✅

**파일**: `src-tauri/src/commands/s3.rs`, `src-tauri/src/commands/sync.rs`

**수정 내용**
- `symlink_metadata()`로 링크 감지 후 기본 제외 (순환 링크 방지)
- `list_local_dir`(동기)와 `collect_local_files`(비동기) 모두 적용

### M-10. 프론트엔드 로그 메시지가 문자열 기반 분류에 의존 ✅

**파일**: `src/components/log/LogPanel.tsx`, `src/types/index.ts`

**수정 내용**
- `LogCategory = "transfer" | "cdn" | "profile" | "system"` 타입 추가
- `LogEntry.category` 필드 추가
- `addLog()` 시그니처: `(level, message, category?, metadata?)`
- LogPanel Purge 이력 탭: `log.category === "cdn"` 기준 필터링

---

## Low [ 완료 ] ✅

### L-1. `sync_preview` 기능이 UI와 연결되지 않음 ✅

**파일**: `src-tauri/src/commands/sync.rs`, `src/hooks/useTransfer.ts`, `src/components/transfer/TransferButtons.tsx`

백엔드에는 dry-run 비교 기능이 있으나 UI에서 볼 수 없었다.

**수정 내용**
- `src/types/index.ts`에 `FileEntry`, `SyncResult` 타입 추가
- `SyncPreviewDialog.tsx` 신규 작성 — new / modified / deleted / unchanged 4탭, 파일명·크기·경로 표시
- `useTransfer.buildPreview()` 추가 — `invoke("sync_preview", { profileId, localDir, remotePrefix })`
- `TransferButtons`에 "미리보기" 버튼 추가 (연결 + 로컬 경로 있을 때 활성화)

### L-2. S3Adapter가 command마다 새로 생성됨 ✅

**파일**: `src-tauri/src/commands/s3.rs`, `src-tauri/src/commands/sync.rs`, `src-tauri/src/lib.rs`

각 command가 `reqwest::Client`와 adapter를 새로 생성하여 연결 오버헤드가 발생했다.

**수정 내용**
- `src-tauri/src/utils/adapter_cache.rs` 신규 작성
  - `AdapterCache` — `RwLock<HashMap<String, S3Adapter>>` (double-checked locking)
  - `get_or_create(profile_id, factory)` / `invalidate(profile_id)`
- `lib.rs`에 `.manage(AdapterCache::new())` 등록
- S3 관련 모든 command (`list_s3_objects`, `delete_s3_objects`, `put_s3_object`, `get_presigned_url`, `rename_s3_object`, `upload_files`) + sync 3개 command에 `State<'_, AdapterCache>` 파라미터 추가
- `save_profile`, `delete_profile` 호출 시 해당 프로필 캐시 자동 무효화

### L-3. Presigned URL 만료 시간이 고정됨 ✅

**파일**: `src/components/panels/RemotePanel.tsx`

컨텍스트 메뉴의 "Presigned URL 복사"가 1시간(3600s)으로 고정되어 있었다.

**수정 내용**
- "URL 복사 (15분)", "URL 복사 (1시간)", "URL 복사 (24시간)" 3개 항목으로 교체
- 로그에 만료 시간 표기 포함

### L-4. 진행 속도 값이 항상 0으로 전달됨 ✅

**파일**: `src-tauri/src/commands/sync.rs`, `src-tauri/src/commands/s3.rs`

전송 progress payload의 `speed` 필드가 항상 `0`이었다.

**수정 내용**
- 각 파일 전송 task에서 `start_time = Instant::now()` 캡처
- progress callback에서 `speed = transferred / elapsed` 계산 (elapsed > 50ms 조건)
- `ProgressDialog`가 이미 `fmtSpeed` / `fmtEta` 표시 구현 완료 → 실제 속도/ETA 자동 반영

### L-5. 테스트 자동화 부재 ✅

**파일**: `package.json`, `vitest.config.ts`, `src/test/`, `src-tauri/src/utils/retry.rs`, `.github/workflows/ci.yml`

명시적인 테스트 스크립트와 CI 파이프라인이 없었다.

**수정 내용**

_프론트엔드_
- devDependencies 추가: `vitest`, `@testing-library/react`, `@testing-library/user-event`, `@testing-library/jest-dom`, `jsdom`
- `vitest.config.ts` — jsdom 환경, React 플러그인, setup 파일 지정
- `src/test/setup.ts` — `@testing-library/jest-dom` 로드
- `src/test/appStore.test.ts` — Zustand store 단위 테스트 (`addLog` 필드 검증, 1000건 상한, `addTransfer`/`updateTransfer`)
- `package.json` scripts: `test` (단일 실행), `test:watch` (감시 모드) 추가

_Rust_
- `src-tauri/src/utils/retry.rs` — `#[cfg(test)]` 모듈: retryable/non-retryable 코드 검증

_CI (GitHub Actions)_
- `.github/workflows/ci.yml` 신규 작성
  - `frontend` job: `pnpm typecheck` → `pnpm test` → `pnpm build`
  - `backend` job: `cargo check --release` → `cargo test`
  - push/PR to `main` 브랜치에서 트리거

> 설치: `pnpm install` 후 `pnpm test` (프론트엔드), `cd src-tauri && cargo test` (Rust)

---

## 우선순위 요약

| 우선순위 | 이슈 | 상태 |
|---|---|---|
| 1 | C-1, C-2 | ✅ 완료 |
| 2 | C-3, C-4 | ✅ 완료 |
| 3 | H-4, H-5 | ✅ 완료 |
| 4 | H-1, H-6 | ✅ 완료 |
| 5 | M-1 ~ M-10 | ✅ 완료 |
| 6 | L-1 ~ L-5 | ✅ 완료 |

**전체 이슈 26건 모두 완료** (2026-04-28 기준)
