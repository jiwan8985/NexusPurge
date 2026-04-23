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
- CloudFront: `InvalidationBatch` API, caller_reference로 멱등성 보장
- 미구현 CDN (Akamai, LG U+, 효성): `Err` 반환, 로그에 기록

### 프로파일 저장
- 메타데이터: `~/.local/share/cdn-upload-tool/profiles.json`
- `secretAccessKey`: OS keyring (Windows Credential Manager / macOS Keychain)
- keyring 키 형식: service=`cdn-upload-tool`, username=`{profile_id}`

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
- `ProfileStore`는 `lib.rs`에서 `.manage(ProfileStore::new().unwrap())`로 등록 필요 (현재 누락, 추가 예정)
- S3 ETag와 MD5는 Multipart 업로드(>8MB 기본값) 시 일치하지 않음 → `hash.rs::parse_multipart_etag` 참고
