# CDN Upload Tool — CLAUDE.md

FTP 스타일 듀얼 패널 S3 업로드 도구. 파일 덮어쓰기 감지 시 CDN을 자동으로 Purge한다.

## 아키텍처 개요

```
┌─────────────────────────────────────────┐
│  React Frontend (Vite + TypeScript)      │
│  ┌──────────┐  ┌──────────┐             │
│  │LocalPanel│  │RemotePanel│  ← 듀얼 패널│
│  └──────────┘  └──────────┘             │
│       ↕ Zustand          ↕              │
│  useTransfer → invoke() → Tauri IPC     │
└──────────────────┬──────────────────────┘
                   │ @tauri-apps/api
┌──────────────────▼──────────────────────┐
│  Rust Backend (Tauri 2)                  │
│  commands/s3.rs   → S3 CRUD             │
│  commands/sync.rs → MD5 Smart Sync      │
│  commands/cdn.rs  → CDN Purge           │
│       ↓                   ↓             │
│  adapters/storage/s3.rs   adapters/cdn/ │
│  (aws-sdk-s3)             (cloudfront)  │
└─────────────────────────────────────────┘
```

## 폴더 구조

```
src/                          # React Frontend
├── components/
│   ├── layout/               # TitleBar, Toolbar, StatusBar
│   ├── panels/               # LocalPanel, RemotePanel (공유 Panel.module.css)
│   ├── transfer/             # TransferButtons, ProgressDialog
│   ├── log/                  # LogPanel
│   └── modals/               # ProfileModal
├── hooks/
│   ├── useS3.ts              # S3 탐색/삭제/presign
│   ├── useTransfer.ts        # 업로드/다운로드 큐 관리
│   └── useProfile.ts         # 프로파일 CRUD + 연결
├── store/appStore.ts         # Zustand 전역 상태
├── types/index.ts            # 공유 타입 (TS ↔ Rust 동기화 필수)
└── styles/                   # variables.css (디자인 토큰), global.css

src-tauri/src/
├── commands/                 # Tauri invoke 핸들러
│   ├── s3.rs                 # 프로파일, 연결, S3 CRUD
│   ├── sync.rs               # Smart Sync 플랜, 업로드/다운로드 실행
│   └── cdn.rs                # CDN Purge 커맨드
├── adapters/
│   ├── storage/base.rs       # StorageAdapter trait
│   ├── storage/s3.rs         # S3Adapter (aws-sdk-s3)
│   ├── cdn/base.rs           # CdnAdapter trait
│   └── cdn/cloudfront.rs     # CloudFrontAdapter
└── utils/
    ├── hash.rs               # MD5 파일 해시 (ETag 비교)
    └── config.rs             # ProfileStore (keyring 연동)
```

## 핵심 데이터 흐름

### 업로드 (Smart Sync)
1. `useTransfer.startUpload()` → `invoke("build_sync_plan")`
2. `sync.rs::build_sync_plan()`: 로컬 MD5 계산 + S3 HeadObject ETag 비교 (병렬)
3. 결과 3분류: `toUpload` / `toSkip` / `toOverwrite`
4. `invoke("start_uploads")` → Rust tokio JoinSet으로 병렬 업로드
5. 각 파일 완료 후 `toOverwrite` 항목은 자동 CDN Purge
6. 진행률: `tauri::Emitter::emit("transfer:progress")` → React `listen()`

### CDN Purge
- 덮어쓰기 감지 → `adapters/cdn/mod.rs::purge_with_credentials()`
- CloudFront: `InvalidationBatch` API, caller_reference로 멱등성 보장. Akamai(Fast Purge)/LG U+·KT(Solbox v3)/효성(ITX) 모두 구현됨 — 각 `adapters/cdn/{provider}.rs` 참고
- 멀티 CDN: `appStore.activeCdns`(배열)에 선택된 CDN 전체에 병렬로 `purge_cdn` 호출 (`usePurge.ts`/`useTransfer.ts`/`useS3.ts` 공통 패턴)
- `commands/cdn.rs::CdnPurgeResult`에 `requestEndpoint`(실제 호출된 API 엔드포인트 설명)·`durationMs`(소요시간) 포함 — 로그/속성 다이얼로그에서 사용
- `commands/cdn.rs::inspect_url` / `commands/s3.rs::get_s3_object_detail`: 우클릭 "속성" 다이얼로그의 온디맨드 실시간 조회용 커맨드(자동 실행 아님) — 각각 임의 URL의 HTTP 응답 헤더, S3 HeadObject 전체 응답을 반환

### 프로파일 저장
- 메타데이터: `~/.local/share/cdn-upload-tool/profiles.json`
- `secretAccessKey`: OS keyring (Windows Credential Manager / macOS Keychain)
- keyring 키 형식: service=`cdn-upload-tool`, username=`{profile_id}` (CDN별 시크릿은 `{profile_id}_akamai` 등 접미사)

### 멀티 CDN 프로필
- 한 프로필에 여러 CDN 가능: `cdnProviders[]` (provider/domain/distributionId), CDN별 도메인은 `provider_domain()` (config.rs)로 해석
- 고객사 전달용 JSON 프로필 파일: `profile-sample.json` 참고 — 프로필 관리 "가져오기"로 임포트 (`import_profile_file` 커맨드)
- 런타임 Purge 대상 CDN: `appStore.activeCdn` — 툴바 드롭다운에서 전환, 모든 Purge(업로드/삭제/수동)가 이 값을 따름

## 코딩 컨벤션

### TypeScript / React
- 파일당 컴포넌트 1개, default export
- CSS Modules만 사용 (`*.module.css`), 인라인 스타일 금지
- Zustand selector: `useAppStore((s) => s.field)` 단위로 구독 최소화
- `invoke()` 호출은 hooks에서만, 컴포넌트에서 직접 호출 금지
- 타입은 `src/types/index.ts`에서 중앙 관리, Rust 구조체와 동기화 유지

### Rust
- `#[tauri::command]` 반환 타입: `Result<T, String>` (에러는 `.map_err(|e| e.to_string())`)
- 비동기 작업: `tokio::task::JoinSet`으로 병렬 처리
- 에러: `anyhow::Result` + `.context("한글 설명")`
- 이벤트 emit: `AppHandle::emit("transfer:progress", payload)`
- `ProfileStore`는 Tauri managed state로 등록 (`tauri::Builder::manage()`)

### 일반
- 함수 분기 없이 단순히 위임하는 wrapper 금지 (adapter trait 구현 제외)
- 외부 UI 라이브러리 도입 금지 (CSS Modules 커스텀만)
- TODO 주석은 `// TODO:` 형식, 이슈 번호 포함 권장

## CDN 어댑터 추가 방법

1. `src-tauri/src/adapters/cdn/` 에 새 파일 생성 (예: `akamai.rs`)
2. `CdnAdapter` trait 구현 (`base.rs` 참고)
3. `cdn/mod.rs::purge_with_credentials()` match arm 추가
4. `src/types/index.ts::CdnProvider` union type에 추가
5. `ProfileModal.tsx::CDN_PROVIDERS` 배열에 추가

## 빌드 / 실행

```bash
# 개발 서버
npm run tauri dev

# 릴리즈 빌드
npm run tauri build

# 빌드 결과물
# Windows: src-tauri/target/release/bundle/msi/*.msi
# macOS:   src-tauri/target/release/bundle/dmg/*.dmg
# Linux:   src-tauri/target/release/bundle/appimage/*.AppImage
```

## 주의 사항

- `src/types/index.ts` 타입과 `src-tauri/src/commands/` 구조체의 필드명(serde rename) 일치 필수
- Tauri 이벤트명 (`transfer:progress`, `transfer:complete`) 변경 시 `useTransfer.ts`와 동기화
- S3 ETag와 MD5는 Multipart 업로드(>8MB 기본값) 시 일치하지 않음 → `hash.rs::parse_multipart_etag` 참고
- 모달/다이얼로그는 반드시 `createPortal(…, document.body)`로 렌더링할 것 — Toolbar/Panel에 `backdrop-filter`(글래스 효과)가 걸려 있어 내부에서 `position: fixed`를 쓰면 해당 요소가 containing block이 되어 레이아웃이 겹침
- `tauri.conf.json`의 `dragDropEnabled: false`는 패널 간 HTML5 드래그앤드랍(업로드/다운로드)에 필수 — true(기본값)면 Windows WebView2가 드랍 이벤트를 가로채 DnD가 조용히 동작하지 않음
- 운영 로그(업로드/다운로드/삭제/Purge)는 JSON(`operation_logs.json`, 전체 이력) + 날짜·타입별 텍스트 파일로 `%LOCALAPPDATA%/cdn-upload-tool/logs/`에 저장됨 — LogPanel "로그 폴더" 버튼으로 열기
  - `system-YYYY-MM-DD.log`: mkdir/rename/delete 등 파일 관리 작업
  - `transfer-YYYY-MM-DD.log`: upload/download 파일 전송 결과
  - `cdn-YYYY-MM-DD.log`: CDN Purge 상세(provider별 상태, 요청 엔드포인트, 소요시간, 대상 경로 최대 50개 미리보기, 전체 오류 메시지) — upload/delete에 딸린 Purge 결과도 여기 기록. 전체 경로 목록은 `operation_logs.json`에 무제한 보관
  - 타입 분리는 `OperationLogService::append_log_file` (operation_log.rs) 참고
