# NexusPurge 보완/수정/추가 이슈 목록

> 분석 기준일: 2026-04-27  
> 심각도: Critical / High / Medium / Low  
> 범위: React UI, Tauri IPC, Rust S3/CDN 백엔드, 프로필 저장소

---

## Critical [ 완료 ]

### C-1. Smart Sync 업로드 후 신규 파일까지 CDN Purge가 실행됨

**파일**: `src/hooks/useTransfer.ts`, `src-tauri/src/commands/sync.rs`

프론트엔드는 `plan.toUpload`와 `plan.toOverwrite`를 합쳐 `start_uploads`로 전달하지만, 각 항목이 신규 업로드인지 덮어쓰기인지 구분하는 값이 없다. 백엔드 `start_uploads`는 업로드 성공 시 CDN 설정이 있으면 모든 항목에 Purge를 실행한다.

**영향**: 신규 파일에도 불필요한 CloudFront Invalidation이 발생해 비용과 API 사용량이 증가한다.

**수정안**
- `UploadItem`에 `isOverwrite` 필드를 추가한다.
- `plan.toOverwrite` 항목만 `isOverwrite: true`로 전달한다.
- 백엔드는 `item.is_overwrite == true`인 경우에만 Purge한다.
- 현재 사용되지 않는 `s3.rs::upload_files`와 `sync.rs::start_uploads`의 역할을 정리한다.

### C-2. 프로필 목록 상태가 훅 인스턴스별로 분리됨

**파일**: `src/hooks/useProfile.ts`, `src/App.tsx`, `src/components/layout/TitleBar.tsx`, `src/components/modals/ProfileModal.tsx`

`useProfile()` 내부가 `useState<S3Profile[]>`로 프로필 목록을 보관한다. `App`, `TitleBar`, `ProfileModal`이 각각 다른 훅 인스턴스를 만들기 때문에 `App`에서 `loadProfiles()`를 호출해도 `TitleBar`와 `ProfileModal`의 `profiles`는 자동으로 갱신되지 않는다.

**영향**: 저장된 프로필이 드롭다운이나 모달 목록에 표시되지 않거나, 저장/삭제 후 화면이 서로 다르게 보일 수 있다.

**수정안**
- 프로필 목록을 Zustand 전역 상태로 이동한다.
- `loadProfiles`, `saveProfile`, `deleteProfile` 후 전역 `profiles`를 갱신한다.
- 모달이 열릴 때도 목록을 재조회하거나 캐시 무효화한다.

### C-3. S3 목록 페이지네이션 미구현

**파일**: `src-tauri/src/adapters/storage/s3.rs`, `src-tauri/src/commands/s3.rs`, `src/hooks/useS3.ts`

`ListObjectsV2`는 최대 1,000개만 반환하지만 `nextContinuationToken`을 사용한 추가 조회가 없다.

**영향**: 1,001번째 이후 객체가 UI에 표시되지 않는다.

**수정안**
- 백엔드에서 `continuation-token` 반복 조회를 구현한다.
- 또는 프론트엔드에서 `nextContinuationToken` 기반 “더 불러오기”를 제공한다.

### C-4. Windows 다운로드 경로 조합 오류

**파일**: `src/hooks/useTransfer.ts`

다운로드 경로를 `local.path + "/" + fileName`으로 조합한다. Windows 경로에서는 `C:\folder/file.txt`처럼 혼합 구분자가 생긴다.

**수정안**
- 프론트엔드에서 경로 조합 헬퍼를 만든다.
- 더 안전하게는 백엔드 command에 대상 디렉터리와 파일명을 분리 전달하고 Rust `PathBuf::join`으로 조합한다.

---

## High [ 완료 ]

### H-1. 툴바 파일 작업 버튼이 실제 기능과 연결되지 않음

**파일**: `src/components/layout/Toolbar.tsx`

`새 폴더`, `삭제`, `이름 변경` 버튼은 표시되지만 실제 선택 항목과 연결된 작업 흐름이 부족하다.

**수정안**
- 새 S3 폴더: `useS3.createDirectory()` 연결
- 로컬 폴더 생성: 별도 Tauri command 추가
- 삭제: 로컬/S3 선택 상태별 command 분기
- 이름 변경: S3는 CopyObject + DeleteObject, 로컬은 rename command 추가

### H-2. 파일 단위 동시 업로드 제한 없음

**파일**: `src-tauri/src/commands/sync.rs`, `src-tauri/src/commands/s3.rs`

선택한 파일 수만큼 `JoinSet` task를 생성한다. 파일이 많으면 메모리, 네트워크, S3 rate limit에 영향을 줄 수 있다.

**수정안**
- `tokio::sync::Semaphore`로 파일 단위 동시 실행을 4~8개 수준으로 제한한다.
- UI에서 동시 전송 개수를 설정할 수 있게 한다.

### H-3. 프로필 저장 시 연결 검증이 선택 사항으로도 제공되지 않음

**파일**: `src/components/modals/ProfileModal.tsx`, `src/hooks/useProfile.ts`

잘못된 bucket, region, credential도 저장될 수 있다.

**수정안**
- “저장 전 연결 테스트” 버튼을 추가한다.
- 저장 성공 후 자동 연결 옵션을 제공한다.

### H-4. S3 HTTP 오류 응답을 정상 빈 목록처럼 처리할 수 있음

**파일**: `src-tauri/src/adapters/storage/s3.rs`

`list_objects_raw()`는 `signed_get()` 이후 HTTP status를 확인하지 않고 XML 파싱을 진행한다. 권한 오류나 서명 오류 XML을 빈 목록처럼 파싱할 위험이 있다.

**수정안**
- `resp.status().is_success()`를 확인한다.
- 실패 시 status와 응답 body를 포함한 오류를 반환한다.

### H-5. S3 XML 응답 파서가 특수문자와 XML entity를 처리하지 않음

**파일**: `src-tauri/src/adapters/storage/s3.rs`

`xml_extract()`가 문자열 검색 기반이라 `&amp;`, `&lt;` 같은 entity를 디코딩하지 않는다. 파일명에 `&`, `<`, `>` 등이 포함되면 UI 경로와 실제 key가 달라질 수 있다.

**수정안**
- `quick-xml` 같은 XML 파서를 사용한다.
- 최소한 Key, Prefix, ETag, LastModified, Size 필드는 entity decode를 적용한다.

### H-6. CloudFront 외 CDN이 UI에서 선택 가능하지만 미구현

**파일**: `src/components/modals/ProfileModal.tsx`, `src-tauri/src/adapters/cdn/mod.rs`

Akamai, LG U+, Hyosung은 선택 가능하지만 백엔드는 `not implemented` 오류를 반환한다.

**수정안**
- 미구현 공급자는 UI에서 비활성화하고 “준비 중”으로 표시한다.
- 구현 대상 CDN별 인증 방식과 Purge API 명세를 분리 문서화한다.

### H-7. 앱 재시작 시 마지막 프로필 복원 없음

**파일**: `src/hooks/useProfile.ts`, `src/store/appStore.ts`

앱을 다시 열면 항상 연결되지 않은 상태로 시작한다.

**수정안**
- `lastProfileId`를 로컬 설정에 저장한다.
- 앱 시작 시 마지막 프로필을 선택 상태로 복원하고, 옵션에 따라 자동 연결한다.

---

## Medium

### M-1. ProfileModal의 한글 문자열과 UX 정리가 필요함

**파일**: `src/components/modals/ProfileModal.tsx`, `src/components/modals/ProfileModal.module.css`

주요 화면 일부는 정리됐지만 프로필 모달에는 깨진 한글 문자열과 오래된 UI 톤이 남아 있다.

**수정안**
- 모든 라벨, placeholder, 버튼 문구를 정상 한글로 교체한다.
- 최근 개선한 운영 콘솔 스타일과 맞춰 모달 UI를 재설계한다.

### M-2. 삭제 확인에 브라우저 기본 `confirm()` 사용

**파일**: `src/components/panels/RemotePanel.tsx`

Tauri/WebView 환경에서 기본 confirm은 일관된 UX를 제공하지 못한다.

**수정안**
- 공용 ConfirmDialog 컴포넌트를 추가한다.
- 삭제 대상, 개수, 경로를 명확히 보여준다.

### M-3. 전송 중 앱 종료 경고 없음

**파일**: `src/components/layout/TitleBar.tsx`, `src-tauri/src/lib.rs`

전송 중 닫기 버튼을 누르면 작업이 중단될 수 있다.

**수정안**
- `isTransferring` 상태에서 닫기 전 확인 다이얼로그를 표시한다.
- Tauri `on_window_event` 또는 프론트엔드 close handler를 통해 종료를 가로챈다.

### M-4. 대용량 업로드 확인 및 예상 비용 안내 없음

**파일**: `src/components/transfer/TransferButtons.tsx`, `src/hooks/useTransfer.ts`

선택 파일 총량이 큰 경우에도 즉시 업로드가 시작된다.

**수정안**
- 선택 파일 총 크기를 계산한다.
- 예: 100MB 또는 1GB 초과 시 확인 다이얼로그를 표시한다.

### M-5. 재시도 로직 없음

**파일**: `src-tauri/src/adapters/storage/s3.rs`, `src-tauri/src/adapters/cdn/cloudfront.rs`

일시적인 네트워크 오류, S3 `503 SlowDown`, CloudFront rate limit에 대한 재시도가 없다.

**수정안**
- 지수 백오프 기반 최대 3회 재시도 구현
- 재시도 가능한 status code와 즉시 실패해야 하는 인증 오류를 구분

### M-6. ErrorBoundary 없음

**파일**: `src/App.tsx`

렌더링 오류가 발생하면 전체 화면이 깨질 수 있다.

**수정안**
- 최상위 ErrorBoundary 추가
- 로그 저장, 재시도, 앱 재시작 안내 fallback UI 제공

### M-7. 다운로드 대상 경로 선택 다이얼로그 없음

**파일**: `src/hooks/useTransfer.ts`

다운로드는 현재 로컬 패널 경로로만 저장된다.

**수정안**
- `@tauri-apps/plugin-dialog`를 추가한다.
- 다운로드 전 폴더 선택 옵션을 제공한다.

### M-8. 빈 업로드 계획 처리 UX 부족

**파일**: `src/hooks/useTransfer.ts`

Smart Sync 결과가 모두 skip인 경우에도 진행 다이얼로그가 열린 뒤 닫히지 않을 수 있다.

**수정안**
- 업로드 대상이 0개면 다이얼로그를 열지 않거나 즉시 닫는다.
- “모든 파일이 최신 상태입니다.” 로그/토스트를 표시한다.

### M-9. 로컬 디렉터리 순회에서 심볼릭 링크 처리 정책 없음

**파일**: `src-tauri/src/commands/s3.rs`, `src-tauri/src/commands/sync.rs`

심볼릭 링크를 따라갈지, 제외할지 정책이 명확하지 않다. 순환 링크가 있으면 문제가 될 수 있다.

**수정안**
- `symlink_metadata()`로 링크 여부를 확인한다.
- 기본 정책은 symlink 제외, 필요 시 옵션으로 허용한다.

### M-10. 프론트엔드 로그 메시지가 문자열 기반 분류에 의존

**파일**: `src/components/log/LogPanel.tsx`, `src/types/index.ts`

Purge 이력 필터가 메시지 문자열에 의존한다.

**수정안**
- `LogEntry`에 `category?: "cdn" | "transfer" | "profile" | "system"` 필드 추가
- 로그 패널 탭은 category 기준으로 필터링

---

## Low

### L-1. `sync_preview` 기능이 UI와 연결되지 않음

**파일**: `src-tauri/src/commands/sync.rs`, `src/hooks/useTransfer.ts`

백엔드에는 dry-run 비교 기능이 있으나 UI에서 볼 수 없다.

**개선안**
- 업로드 전 “변경 미리보기” 패널 추가
- 신규/수정/삭제/동일 항목을 표로 표시

### L-2. S3Adapter가 command마다 새로 생성됨

**파일**: `src-tauri/src/commands/s3.rs`, `src-tauri/src/commands/sync.rs`

각 command가 HTTP client와 adapter를 새로 만든다.

**개선안**
- 프로필별 adapter cache 또는 managed state 도입
- credential 변경 시 cache invalidation

### L-3. Presigned URL 만료 시간이 고정됨

**파일**: `src/components/panels/RemotePanel.tsx`

현재 1시간으로 고정되어 있다.

**개선안**
- 15분, 1시간, 24시간 옵션 제공
- 클립보드 복사 후 만료 시각을 로그에 표시

### L-4. 진행 속도 값이 항상 0으로 전달됨

**파일**: `src-tauri/src/commands/sync.rs`, `src-tauri/src/commands/s3.rs`

전송 progress payload의 `speed`가 계산되지 않는다.

**개선안**
- 시작 시각과 전송 바이트 차이를 기반으로 bytes/sec 계산
- UI에 `MB/s`, 남은 시간 표시

### L-5. 테스트 자동화 부재

**파일**: `package.json`, `src-tauri/Cargo.toml`

현재 명시적인 프론트엔드 테스트/러스트 테스트 스크립트가 없다.

**개선안**
- Vitest + React Testing Library 추가
- Rust unit test 및 LocalStack 기반 통합 테스트 추가
- CI에서 `npm.cmd run build`, `cargo test`, `cargo check` 실행

---

## 우선순위 요약

| 우선순위 | 이슈 | 권장 처리 |
|---|---|---|
| 1 | C-1, C-2 | 업로드/Purge 정확도와 프로필 목록 표시부터 수정 |
| 2 | C-3, C-4 | 대량 버킷과 Windows 다운로드 안정화 |
| 3 | H-4, H-5 | S3 오류 응답과 XML 파싱 안정화 |
| 4 | H-1, H-6 | UI에 보이는 기능과 실제 구현 상태 일치 |
| 5 | M-1, M-3, M-5 | 상용 배포 전 UX와 장애 복구 보강 |
