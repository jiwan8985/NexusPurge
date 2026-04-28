# NexusPurge — 프로젝트 분석 문서

> 최종 업데이트: 2026-04-28

---

## 1. 개요

NexusPurge는 **Tauri 2 기반 데스크톱 앱**으로, AWS S3 버킷을 FTP 스타일 듀얼 패널로 탐색하고 파일을 업로드/다운로드하며 덮어쓰기 시 CDN 캐시를 자동으로 Purge하는 도구입니다.

| 항목 | 내용 |
|------|------|
| 플랫폼 | Windows / macOS / Linux (Tauri 2) |
| 프론트엔드 | React 18.3 + TypeScript 5.5 + Vite 5.3 |
| 상태 관리 | Zustand 4.5 (`subscribeWithSelector`) |
| 백엔드 | Rust (Tokio 비동기) |
| S3 통신 | 순수 Rust + SigV4 자체 구현 (aws-sdk-s3 미사용) |
| CDN | CloudFront 구현 완료, Akamai/LG U+/효성 스텁 |

---

## 2. 아키텍처

```
┌──────────────────────────────────────────────────────────────┐
│  React Frontend (Vite + TypeScript)                           │
│                                                              │
│  TitleBar  Toolbar             StatusBar                     │
│  ┌─────────────────────────────────────────┐                │
│  │  LocalPanel   [▶ ◀]   RemotePanel       │  ← 듀얼 패널  │
│  │  (로컬 FS)    전송     (S3 버킷)         │               │
│  └─────────────────────────────────────────┘                │
│  LogPanel (작업로그 / 전송큐 / Purge이력)                    │
│                                                              │
│  Zustand Store ─────── hooks ──────────────────────────────  │
│    appStore.ts          useTransfer.ts (업로드/다운로드)      │
│                         useS3.ts (S3 CRUD)                  │
│                         useProfile.ts (프로파일 관리)         │
│                         useVirtualList.ts (가상 스크롤)       │
└────────────────────────────┬─────────────────────────────────┘
                             │ @tauri-apps/api invoke / listen
┌────────────────────────────▼─────────────────────────────────┐
│  Rust Backend (Tauri 2)                                       │
│                                                              │
│  commands/s3.rs    ──→  adapters/storage/s3.rs               │
│  commands/sync.rs  ──→  utils/hash.rs (MD5 ETag 비교)        │
│  commands/cdn.rs   ──→  adapters/cdn/cloudfront.rs           │
│                                                              │
│  utils/config.rs   ──→  OS Keyring (Credential Manager)     │
│  utils/sigv4.rs    ──→  AWS SigV4 서명                       │
└──────────────────────────────────────────────────────────────┘
```

---

## 3. 폴더 구조

```
NexusPurge/
├── src/                              # React 프론트엔드
│   ├── App.tsx                       # 루트 레이아웃 조합
│   ├── types/index.ts                # 공유 타입 (TS ↔ Rust 동기화 필수)
│   ├── store/appStore.ts             # Zustand 전역 상태
│   ├── styles/
│   │   ├── variables.css             # 디자인 토큰 (다크/라이트 모드)
│   │   └── global.css                # 리셋 + 레이아웃 기반
│   ├── hooks/
│   │   ├── useTransfer.ts            # 업로드/다운로드 큐 + Tauri 이벤트 구독
│   │   ├── useS3.ts                  # S3 탐색/삭제/Presigned URL
│   │   ├── useProfile.ts             # 프로파일 CRUD + 연결
│   │   └── useVirtualList.ts         # 가상 스크롤 (10,000개+ 파일 성능)
│   └── components/
│       ├── layout/
│       │   ├── TitleBar.tsx          # 드래그 리전, 프로파일 드롭다운, 윈도우 컨트롤
│       │   ├── Toolbar.tsx           # 연결/해제, 파일 작업 버튼
│       │   └── StatusBar.tsx         # 연결 상태, 전송 카운트, 로그 토글
│       ├── panels/
│       │   ├── LocalPanel.tsx        # 로컬 FS 탐색 (드래그 소스)
│       │   ├── RemotePanel.tsx       # S3 탐색 (드롭 타겟)
│       │   └── Panel.module.css      # 패널 공유 스타일
│       ├── transfer/
│       │   ├── TransferButtons.tsx   # ▶ 업로드 / ◀ 다운로드 / ⚖ 미리보기 버튼
│       │   ├── ProgressDialog.tsx    # 전송 진행률 모달
│       │   └── SyncPreviewDialog.tsx # 동기화 미리보기 (new/modified/deleted/unchanged 4탭)
│       ├── log/
│       │   └── LogPanel.tsx          # 3탭 로그 패널 (category 필터링)
│       ├── modals/
│       │   └── ProfileModal.tsx      # S3 프로파일 + CDN 설정 CRUD
│       └── common/
│           ├── ContextMenu.tsx       # 우클릭 컨텍스트 메뉴 (Portal)
│           └── ConfirmDialog.tsx     # 공용 확인 다이얼로그 (danger 지원)
│
└── src-tauri/src/                    # Rust 백엔드
    ├── main.rs                       # Tauri 진입점
    ├── lib.rs                        # 커맨드 등록, ProfileStore + AdapterCache 관리 상태 등록
    ├── commands/
    │   ├── s3.rs                     # 프로파일 CRUD, S3 리스팅, 객체 조작
    │   ├── sync.rs                   # Smart Sync 플랜, 병렬 업로드/다운로드, sync_preview, 이벤트 emit
    │   └── cdn.rs                    # CloudFront Invalidation 커맨드
    ├── adapters/
    │   ├── storage/
    │   │   ├── base.rs               # StorageAdapter trait
    │   │   └── s3.rs                 # S3Adapter (멀티파트 10MB, 동시 4개)
    │   └── cdn/
    │       ├── base.rs               # CdnAdapter trait
    │       ├── cloudfront.rs         # CloudFront CreateInvalidation 구현
    │       └── mod.rs                # CDN 프로바이더 디스패치
    └── utils/
        ├── config.rs                 # ProfileStore (JSON + OS 키링)
        ├── sigv4.rs                  # AWS SigV4 자체 구현
        ├── hash.rs                   # MD5 파일 해시 (ETag 비교용)
        ├── adapter_cache.rs          # AdapterCache (profile_id 키 RwLock HashMap)
        └── retry.rs                  # is_retryable_status() + 지수 백오프 헬퍼
```

---

## 4. 핵심 데이터 타입

### S3 프로파일
```typescript
interface S3Profile {
  id: string;
  name: string;
  region: string;
  bucket: string;
  accessKeyId: string;
  secretAccessKey: string;      // OS 키링에 저장, UI에서 마스킹
  endpoint?: string;             // S3 호환 커스텀 엔드포인트
  cdnProvider?: CdnProvider;     // "cloudfront" | "akamai" | "lgu" | "hyosung"
  cdnDistributionId?: string;
  cdnDomain?: string;
}
```

### 파일 항목
```typescript
interface FileItem {
  name: string;
  path: string;             // 로컬: 절대경로 / 리모트: S3 키
  size: number;
  lastModified: string;     // ISO 8601
  isDirectory: boolean;
  etag?: string;            // S3 ETag (덮어쓰기 감지)
  contentType?: string;
}
```

### 전송 항목
```typescript
interface TransferItem {
  id: string;
  direction: "upload" | "download";
  fileName: string;
  size: number;
  status: "pending" | "uploading" | "downloading" | "hashing"
        | "skipped" | "overwriting" | "complete" | "error";
  progress: number;         // 0~100
  transferredBytes: number;
  speed?: number;           // bytes/sec (실시간 업데이트)
  cdnPurged?: boolean;
  cdnPurgeError?: string;
  error?: string;
}
```

### 동기화 플랜
```typescript
interface SyncPlan {
  toUpload: FileItem[];     // 신규 파일 (S3에 없음)
  toSkip: FileItem[];       // ETag 일치 → 스킵
  toOverwrite: FileItem[];  // ETag 불일치 → 덮어쓰기 + CDN Purge
}
```

### 파일 엔트리 (sync_preview용)
```typescript
interface FileEntry {
  localPath: string | null;
  remoteKey: string;
  size: number;
  localMd5: string | null;
  remoteEtag: string | null;
}

interface SyncResult {
  new: FileEntry[];
  modified: FileEntry[];
  deleted: FileEntry[];
  unchanged: FileEntry[];
}
```

---

## 5. 핵심 데이터 흐름

### 5.1 Smart Sync 업로드

```
useTransfer.startUpload()
  │
  ├─ invoke("build_sync_plan", { profileId, localPaths, remotePrefix })
  │    │
  │    └─ sync.rs::build_sync_plan()
  │         ├─ 로컬 MD5 계산 (hash.rs)
  │         │   └─ 파일 ≥10MB: calculate_multipart_etag() [ETag 포맷: "hash-N"]
  │         │      파일 <10MB: compute_file_md5()
  │         └─ S3 HeadObject ETag 병렬 비교 (tokio JoinSet)
  │              → toUpload / toSkip / toOverwrite 3분류
  │
  ├─ setSyncPlan(plan)     ← LocalPanel 상태 배지 (신규/Purge예정) 표시
  │
  └─ invoke("start_uploads", { profileId, items, cdnProvider, ... })
       │
       └─ sync.rs::start_uploads()
            ├─ tokio JoinSet으로 병렬 업로드
            ├─ 각 청크마다 emit("transfer:progress", { id, progress, speed })
            └─ 완료 시 emit("transfer:complete", { id, status, cdnPurged })
                 └─ toOverwrite 항목: CDN Purge 자동 실행
```

### 5.2 CDN Purge 흐름

```
cdn.rs::purge_cdn()
  └─ adapters/cdn/mod.rs::purge_with_credentials()
       ├─ "cloudfront" → cloudfront.rs::CloudFrontAdapter::purge()
       │    └─ CreateInvalidation API (CallerReference로 멱등성 보장)
       ├─ "akamai"     → NotImplemented (로그 기록)
       ├─ "lgu"        → NotImplemented
       └─ "hyosung"    → NotImplemented
```

### 5.3 Tauri 이벤트 흐름

```
Rust Backend                    React Frontend
     │                               │
     ├─ emit("transfer:progress")    │
     │   { id, progress, speed }  ──▶│ useTransfer.ts listen()
     │                               │   └─ updateTransfer(id, {...})
     │                               │        └─ ProgressDialog 리렌더
     │                               │
     └─ emit("transfer:complete")    │
         { id, status, cdnPurged } ──▶│ useTransfer.ts listen()
                                     │   └─ addLog("success/error", ...)
```

---

## 6. Zustand 상태 구조

```typescript
interface AppState {
  // 연결 상태
  activeProfile: S3Profile | null;
  isConnected: boolean;
  isConnecting: boolean;

  // 듀얼 패널
  local: PanelState;   // { path, files, selectedPaths, isLoading, sortKey, sortAsc }
  remote: PanelState;

  // 전송
  transfers: TransferItem[];       // 전송 큐 (최대 누적)
  isTransferring: boolean;
  showProgressDialog: boolean;
  syncPlan: SyncPlan | null;       // build_sync_plan 결과 → LocalPanel 배지 반영

  // 로그
  logs: LogEntry[];                // 최대 1000개
  isLogPanelVisible: boolean;

  // 모달
  isProfileModalOpen: boolean;
}
```

---

## 7. Tauri 커맨드 목록

| 커맨드 | 위치 | 설명 |
|--------|------|------|
| `load_profiles` | s3.rs | 저장된 S3 프로파일 전체 로드 |
| `save_profile` | s3.rs | 프로파일 저장 (신규/수정) |
| `delete_profile` | s3.rs | 프로파일 삭제 |
| `connect_s3` | s3.rs | 연결 테스트 (ListBuckets 호출) |
| `list_local_dir` | s3.rs | 로컬 디렉터리 목록 조회 |
| `list_s3_objects` | s3.rs | S3 버킷 객체 목록 조회 |
| `delete_s3_objects` | s3.rs | S3 객체 삭제 |
| `put_s3_object` | s3.rs | S3 객체 업로드 (폴더 생성용) |
| `get_presigned_url` | s3.rs | Presigned URL 생성 (15분·1시간·24시간 선택) |
| `rename_s3_object` | s3.rs | S3 객체 이름 변경 (Copy + Delete) |
| `build_sync_plan` | sync.rs | MD5 기반 동기화 플랜 생성 |
| `start_uploads` | sync.rs | 병렬 업로드 실행 + 이벤트 emit + 실시간 속도 계산 |
| `start_downloads` | sync.rs | 병렬 다운로드 실행 + 이벤트 emit + 실시간 속도 계산 |
| `sync_preview` | sync.rs | 업로드 실행 없이 new/modified/deleted/unchanged 4분류 미리보기 |
| `purge_cloudfront` | cdn.rs | CloudFront Invalidation 실행 |
| `purge_cdn` | cdn.rs | CDN 프로바이더 자동 감지 후 Purge |

---

## 8. 구현 완료 항목

### 백엔드 (Rust)
- [x] SigV4 서명 자체 구현 (`utils/sigv4.rs`)
- [x] MD5 해시 계산 — 일반 파일 + 멀티파트 ETag 포맷 (`utils/hash.rs`)
- [x] S3Adapter — 멀티파트 업로드 (≥10MB, 동시 4개, 10MB 청크)
- [x] S3Adapter — 다운로드, HeadObject, ListObjects
- [x] ProfileStore — JSON 파일 + OS 키링 보관
- [x] Smart Sync — toUpload/toSkip/toOverwrite 분류 (병렬 ETag 비교)
- [x] 전송 이벤트 emit (progress + 실시간 속도, complete)
- [x] CloudFront Invalidation 어댑터
- [x] 로컬 디렉터리 조회 커맨드
- [x] AdapterCache — profile_id 키 RwLock HashMap, save/delete 시 invalidate
- [x] sync_preview — 업로드 실행 없이 4분류(new/modified/deleted/unchanged) SyncResult 반환
- [x] Retry 유틸 — 429/500/502/503/504 재시도, 최대 3회 지수 백오프
- [x] 심볼릭 링크 감지 및 기본 제외 (순환 링크 방지)
- [x] 다운로드 폴더 선택 다이얼로그 (`@tauri-apps/plugin-dialog`)
- [x] S3 객체 이름 변경 커맨드 (`rename_s3_object`)

### 프론트엔드 (React)
- [x] Zustand 전역 상태 (듀얼 패널, 전송 큐, 로그, 모달)
- [x] 가상 스크롤 리스트 (`useVirtualList.ts`) — 10,000개+ 파일 지원
- [x] LocalPanel — 로컬 FS 탐색, 드래그 소스, 상태 배지
- [x] RemotePanel — S3 탐색, 드롭 타겟, 컨텍스트 메뉴
- [x] TitleBar — 커스텀 드래그 리전, 프로파일 드롭다운, 윈도우 컨트롤
- [x] Toolbar — 연결/해제, 파일 작업 버튼 (새 폴더/삭제/이름 변경 연결 완료)
- [x] TransferButtons — 업로드/다운로드/미리보기 트리거
- [x] ProgressDialog — 파일별 진행률, 실시간 속도(MB/s), 예상 시간
- [x] SyncPreviewDialog — new/modified/deleted/unchanged 4탭 미리보기
- [x] LogPanel — 3탭 (작업로그/전송큐/Purge이력), category 필터링
- [x] ProfileModal — S3 + CDN 프로파일 CRUD
- [x] ContextMenu — Portal 기반 우클릭 메뉴
- [x] ConfirmDialog — 공용 확인 다이얼로그 (danger 스타일 지원)
- [x] ErrorBoundary — 렌더링 오류 시 재시도 fallback UI
- [x] 다크/라이트 모드 CSS 토큰 (`prefers-color-scheme`)
- [x] useTransfer.ts — SyncPlan 배지 연동 + buildPreview() 추가
- [x] Presigned URL — 15분·1시간·24시간 3가지 만료 옵션
- [x] 빈 업로드 플랜 조기 종료 ("모든 파일이 최신 상태" 로그)

### 테스트 / CI
- [x] Vitest + JSDOM 단위 테스트 (`src/test/appStore.test.ts`)
- [x] Rust 단위 테스트 (`utils/retry.rs` `#[cfg(test)]` 모듈)
- [x] GitHub Actions CI (`.github/workflows/ci.yml`) — frontend + backend 자동 검증

---

## 9. 알려진 제한 및 미구현

| 항목 | 상태 | 비고 |
|------|------|------|
| Akamai CDN Purge | 미구현 (스텁) | `Err("NotImplemented")` 반환 |
| LG U+ CDN Purge | 미구현 (스텁) | |
| 효성 ITX CDN Purge | 미구현 (스텁) | |
| 대용량 파일 멀티파트 다운로드 | 단순 GET으로 처리 | |
| S3 Transfer Acceleration | 미지원 | |
| 버킷 간 복사 (서버사이드) | 미지원 | |

---

## 10. 개발 환경 설정

### 전제 조건
- Node.js 18+ / npm 9+
- Rust stable + Cargo
- Tauri CLI 2.x (`cargo install tauri-cli`)
- Windows: Microsoft C++ Build Tools

### 실행
```bash
# 의존성 설치
pnpm install

# 개발 서버 (핫 리로드)
pnpm tauri dev

# 릴리즈 빌드
pnpm tauri build

# 프론트엔드 단위 테스트
pnpm test

# Rust 단위 테스트
cargo test --manifest-path src-tauri/Cargo.toml
```

### 빌드 산출물
| 플랫폼 | 경로 |
|--------|------|
| Windows | `src-tauri/target/release/bundle/msi/*.msi` |
| macOS | `src-tauri/target/release/bundle/dmg/*.dmg` |
| Linux | `src-tauri/target/release/bundle/appimage/*.AppImage` |

---

## 11. 중요 주의사항

1. **ETag 포맷**: S3 멀티파트 업로드(≥10MB) 시 ETag는 `"md5hash-N"` 형식. `hash.rs::calculate_multipart_etag()`로 로컬 파일도 동일 포맷으로 계산해야 정확한 비교 가능.

2. **타입 동기화**: `src/types/index.ts`의 필드명과 `src-tauri/src/commands/`의 serde rename이 반드시 일치해야 함.

3. **이벤트명**: `transfer:progress`, `transfer:complete` 변경 시 `useTransfer.ts`와 `sync.rs` 양쪽 동시 수정 필요.

4. **OS 키링**: `secretAccessKey`는 Windows Credential Manager / macOS Keychain에 저장. 키링 키: `service="cdn-upload-tool"`, `username="{profile_id}"`.

5. **ProfileStore 등록**: `lib.rs`에서 `.manage(ProfileStore::new().unwrap())`으로 등록 필수.

---

## 12. CDN 어댑터 추가 방법

1. `src-tauri/src/adapters/cdn/` 에 새 파일 생성 (예: `akamai.rs`)
2. `CdnAdapter` trait 구현 (`base.rs` 참고)
3. `cdn/mod.rs::purge_with_credentials()` match arm 추가
4. `src/types/index.ts::CdnProvider` union type에 추가
5. `ProfileModal.tsx::CDN_PROVIDERS` 배열에 추가
