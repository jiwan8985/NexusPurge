# NexusPurge

> S3 파일 배포 · CDN 자동 Purge 데스크톱 도구

FTP 스타일 듀얼 패널 UI로 로컬과 S3 버킷을 나란히 탐색하고, 파일을 업로드·다운로드합니다.  
파일 덮어쓰기가 감지되면 **CDN 캐시를 자동으로 Purge**하여 배포 반영 속도를 단축합니다.

---

## 주요 기능

| 기능 | 설명 |
|------|------|
| **듀얼 패널 탐색** | 로컬 파일 시스템 ↔ S3 버킷을 나란히 탐색 |
| **Smart Sync** | MD5(ETag) 비교로 변경된 파일만 전송, 동일 파일 자동 스킵 |
| **CDN 자동 Purge** | 덮어쓰기 감지 시 CloudFront Invalidation 자동 실행 |
| **Purge 승인/배치 정책** | 기본 수동 Purge, 자동 Purge 전 승인, 대용량 Purge 경고와 1,000개 단위 배치 구조 |
| **Header/Metadata 정책** | Content-Type, Cache-Control, 사용자 정의 Header/Metadata 입력과 실패 시 수동 재시도 구조 |
| **실시간 진행률** | 파일별 전송 속도(MB/s) · 잔여 시간 · 진행 바 표시 |
| **동기화 미리보기** | 업로드 전 신규/수정/삭제/변경없음 4탭으로 변경 내역 미리보기 |
| **Presigned URL** | S3 객체에 대한 15분·1시간·24시간 임시 공개 URL 생성 · 복사 |
| **다중 프로필** | 여러 S3 계정/버킷을 프로필로 저장, 빠른 전환 |
| **프로필 Import/Remove 중심 운영** | 일반 사용자는 암호화 프로필 Import/Remove 중심으로 사용하고 생성/수정은 권한 모델로 제한 |
| **보안 자격증명** | SecretAccessKey를 OS 키링(Credential Manager/Keychain)에 암호화 보관 |
| **작업 로그 적재 구조** | 로컬 JSON 로그와 고객 제공 S3 로그 Bucket/Prefix 적재를 위한 Stub 구조 |
| **가상 스크롤** | 수만 개 파일도 끊김 없이 렌더링 |
| **드래그 앤 드롭** | 로컬 → 리모트 패널에 파일을 끌어다 놓아 업로드 |

---

## 스크린샷

```
┌─────────────────────────────────────────────────────────────────┐
│  NexusPurge   [● my-profile  my-bucket]              — □ ✕     │
├──────────────────────────────────────────────────────────────────┤
│  [연결] [새 폴더] [삭제]                                          │
├──────────────────┬──────────────┬──────────────────────────────┤
│  LOCAL            │   [▶] [◀]  │  S3 BUCKET                    │
│  C:\deploy\web\  │             │  s3://my-bucket/static/       │
│  ───────────────  │             │  ─────────────────────────── │
│  📁 assets       │             │  📁 assets/                   │
│  📝 index.html   │     →       │  📝 index.html      [스킵]    │
│  📝 app.js  [NEW]│             │  📝 chunk.js        [Purge예정]│
│                  │             │                               │
│  3개 항목         │             │  2개 항목                     │
├──────────────────┴──────────────┴──────────────────────────────┤
│  작업로그 │ 전송큐 │ Purge이력                                   │
│  [OK] 전송 완료 + CDN Purge: app.js                             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 설치 및 실행

### 사전 요구사항

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://rustup.rs/) stable
- [Tauri CLI](https://tauri.app/start/prerequisites/) 2.x
- pnpm (아래 명령으로 활성화)

```bash
# corepack으로 pnpm 활성화 (Node.js 16.9+ 포함)
corepack enable pnpm
```

**Linux 추가 패키지**

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev
```

### 개발 서버

```bash
pnpm install
pnpm tauri dev
```

### 릴리즈 빌드

```bash
pnpm tauri build
```

#### 산출물 경로

| OS | 포맷 | 경로 |
|----|------|------|
| Windows | `.msi` | `src-tauri/target/release/bundle/msi/` |
| Windows | `.exe` (NSIS) | `src-tauri/target/release/bundle/nsis/` |
| macOS | `.dmg` | `src-tauri/target/release/bundle/dmg/` |
| macOS | `.app` | `src-tauri/target/release/bundle/macos/` |
| Linux | `.AppImage` | `src-tauri/target/release/bundle/appimage/` |

---

## 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│  React Frontend (Vite + TypeScript + Zustand)               │
│                                                             │
│  hooks/useTransfer.ts  ─→  invoke("start_uploads")         │
│  hooks/useS3.ts        ─→  invoke("list_s3_objects")       │
│  hooks/useProfile.ts   ─→  invoke("load_profiles")         │
└──────────────────────────────┬──────────────────────────────┘
                               │ @tauri-apps/api (IPC)
┌──────────────────────────────▼──────────────────────────────┐
│  Rust Backend (Tokio 비동기)                                  │
│                                                             │
│  commands/sync.rs   ──→  MD5 ETag 비교 → 병렬 업로드        │
│  commands/s3.rs     ──→  S3 CRUD + Presigned URL            │
│  commands/cdn.rs    ──→  CloudFront Invalidation            │
│                                                             │
│  adapters/storage/s3.rs   SigV4 자체 구현, 멀티파트 10MB   │
│  adapters/cdn/cloudfront  CreateInvalidation API            │
│  utils/config.rs          ProfileStore + OS 키링            │
└─────────────────────────────────────────────────────────────┘
```

### Smart Sync 흐름

```
1. MD5 계산  ─→  2. ETag 비교  ─→  3. 분류
   로컬 파일        S3 HeadObject       toUpload   (신규)
   <10MB: MD5       병렬 처리           toSkip     (동일 → 스킵)
   ≥10MB: 멀티파트                      toOverwrite(변경 → 덮어쓰기 + CDN Purge)
```

---

## 프로필 설정

앱 실행 후 타이틀바의 프로필 버튼 → **프로필 관리**에서 추가합니다.

| 항목 | 필수 | 설명 |
|------|:----:|------|
| 프로필 이름 | ✓ | 식별용 이름 |
| 버킷 이름 | ✓ | S3 버킷 이름 |
| 리전 | ✓ | `ap-northeast-2` 등 |
| Access Key ID | ✓ | IAM 액세스 키 |
| Secret Access Key | ✓ | OS 키링에 암호화 보관 |
| 커스텀 엔드포인트 | | MinIO 등 S3 호환 서비스 |
| CDN 제공자 | | CloudFront / Akamai / LG U+ / 효성 ITX |
| Distribution ID | | CDN 자동 Purge용 |

### 최소 IAM 권한

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "s3:GetObject",
        "s3:PutObject",
        "s3:DeleteObject",
        "s3:ListBucket",
        "s3:HeadObject"
      ],
      "Resource": [
        "arn:aws:s3:::your-bucket-name",
        "arn:aws:s3:::your-bucket-name/*"
      ]
    },
    {
      "Effect": "Allow",
      "Action": ["cloudfront:CreateInvalidation"],
      "Resource": "arn:aws:cloudfront::ACCOUNT_ID:distribution/DISTRIBUTION_ID"
    }
  ]
}
```

---

## 기술 스택

| 레이어 | 기술 |
|--------|------|
| 프레임워크 | [Tauri 2](https://tauri.app/) |
| 프론트엔드 | React 18 · TypeScript 5 · Vite 5 |
| 상태 관리 | Zustand 4 |
| 스타일 | CSS Modules (외부 UI 라이브러리 미사용) |
| 백엔드 | Rust + Tokio |
| S3 통신 | SigV4 자체 구현 (`hmac` + `sha2`) |
| 파일 해시 | `md5` crate (ETag 비교) |
| 자격증명 | `keyring` crate (OS 키링 연동) |
| HTTP | `reqwest` (native-tls) |

---

## CI / CD

`main` 브랜치 push 및 PR 시 GitHub Actions가 자동으로 검증합니다.

**CI 워크플로우** (`.github/workflows/ci.yml`):

```
push / PR → main
  ├─ frontend job (ubuntu-latest)
  │   ├─ pnpm typecheck
  │   ├─ pnpm test      ← Vitest 단위 테스트
  │   └─ pnpm build
  └─ backend job (ubuntu-latest)
      ├─ cargo check --release
      └─ cargo test
```

**릴리즈 빌드** (`.github/workflows/build.yml`):

```
workflow_dispatch → release_tag 입력
  ├─ windows-latest  →  .msi  +  .exe
  ├─ macos-latest    →  .dmg  +  .app
  └─ ubuntu-latest   →  .AppImage
```

---

## 프로젝트 구조

```
NexusPurge/
├── src/                        # React 프론트엔드
│   ├── components/
│   │   ├── layout/             # TitleBar · Toolbar · StatusBar
│   │   ├── panels/             # LocalPanel · RemotePanel
│   │   ├── transfer/           # TransferButtons · ProgressDialog · SyncPreviewDialog
│   │   ├── log/                # LogPanel (3탭)
│   │   ├── modals/             # ProfileModal
│   │   └── common/             # ContextMenu · ConfirmDialog · ErrorBoundary
│   ├── hooks/
│   │   ├── useTransfer.ts      # 업로드/다운로드 + 이벤트 구독 + buildPreview
│   │   ├── useS3.ts            # S3 CRUD · Presigned URL
│   │   ├── useProfile.ts       # 프로필 CRUD · 연결
│   │   └── useVirtualList.ts   # 가상 스크롤 (10,000개+ 성능)
│   ├── store/appStore.ts       # Zustand 전역 상태
│   ├── types/index.ts          # 공유 타입 (Rust 구조체와 동기화)
│   ├── styles/                 # CSS 변수 · 전역 스타일
│   └── test/                   # Vitest 단위 테스트
│
└── src-tauri/src/              # Rust 백엔드
    ├── commands/               # Tauri invoke 핸들러
    ├── adapters/storage/       # S3Adapter (멀티파트)
    ├── adapters/cdn/           # CloudFrontAdapter
    └── utils/                  # SigV4 · MD5 · ProfileStore · AdapterCache · Retry
```

---

## CDN 어댑터 추가

새 CDN을 지원하려면 4곳만 수정하면 됩니다.

```
1. src-tauri/src/adapters/cdn/akamai.rs  →  CdnAdapter trait 구현
2. src-tauri/src/adapters/cdn/mod.rs     →  match arm 추가
3. src/types/index.ts                    →  CdnProvider union type 추가
4. src/components/modals/ProfileModal.tsx →  CDN_PROVIDERS 배열 추가
```

---

## 테스트

```bash
# 프론트엔드 단위 테스트 (Vitest)
pnpm test

# 프론트엔드 감시 모드
pnpm test:watch

# Rust 단위 테스트
cargo test --manifest-path src-tauri/Cargo.toml
```

CI는 `.github/workflows/ci.yml`에 정의된 GitHub Actions로 `main` 브랜치 push 및 PR 시 자동 실행됩니다.

---

## 알려진 제한

- LG U+ · 효성 ITX · KT CDN Purge는 API 문서 수령 전까지 NotImplemented Stub 상태를 유지합니다.
- 대용량 파일(≥10MB) 다운로드는 단일 GET으로 처리 (멀티파트 다운로드 미지원)

---

## 2026-06-16 Requirements Update

추가 요구사항은 기존 CloudFront/Akamai 구현을 유지하는 전제로 반영합니다.

- Purge 기본값은 수동입니다. 자동 Purge는 미리보기 영역 근처의 옵션으로 제공하고, 실행 전 승인 팝업을 요구합니다.
- Purge는 전체/개별/일부 대상을 지원하는 정책 모델을 둡니다. Overwrite/Skip 정책과 연계하며, 5,000개 이상 경고 및 10,000개 이상 미권고 경고 기준은 고객 확인 TODO로 관리합니다.
- 기본 Purge Batch Size는 1,000개입니다.
- 업로드 Header/Metadata는 Content-Type, Cache-Control, 사용자 정의 Header, 사용자 Metadata를 수동 입력할 수 있는 모델을 둡니다. 자동 Metadata 적용과 실패 시 수동 재시도 구조를 준비합니다.
- 프로필은 프로젝트 단위/사용자 단위 분리, 프로필 내 복수 CDN Provider, 권한 정보, AI LB 또는 외부 인증 세션 연동 Stub 구조를 포함할 수 있습니다.
- 자체 로그인/계정 DB는 만들지 않습니다. 인증은 Adapter/Stub 구조로만 준비합니다.
- 고객 제공 S3 Bucket 로그 적재는 JSON 포맷, Bucket/Prefix 설정, 실패 재시도 구조를 모델과 Stub 서비스로만 준비합니다.
- 테스트 버전은 설치형 기준입니다. Windows Server 2025/2022/2019, Windows/macOS/Linux 최근 3개년 지원 범위, Unix 서버 지원 가능 여부, 자동 패치/보안 업데이트, 재시작 없는 패치 가능 여부는 TODO/고객 확인사항으로 관리합니다.

---

## 라이선스

MIT

---

## 2026-05-21 Development Scope Update

NexusPurge is scoped as a replacement implementation of the existing customer CDN upload and purge tool, not as a broad new feature expansion. The primary storage target is AWS S3. CDN provider support is modeled for AWS CloudFront, Akamai, LG U+ CDN, and Hyosung CDN.

- CloudFront and Akamai remain the implemented purge paths.
- LG U+ CDN and Hyosung CDN are available in profile configuration and backend credential dispatch, but purge calls intentionally return NotImplemented errors until the customer provides API specifications.
- External authentication is represented by adapter interfaces only. NexusPurge does not implement its own account database, password login, or standalone login screen.
- Operation log/result data structures were added for upload/download/delete/mkdir/rename/purge/sync outcomes, retry context, and future JSON/CSV reporting.

---

## 2026-05-22 Performance and Runtime Delivery Update

Large CDN deployments must be handled as bounded batch work, not as one network/API operation per visible UI row.

- Upload/download concurrency remains capped at 4 files to avoid CPU, disk, memory, and network spikes on customer PCs.
- CDN purge after overwrite is batched after successful uploads, with up to 1000 paths per purge request, instead of creating one purge request per file.
- UI lists should stay virtualized and logs/transfers should keep bounded retained history for large folders.
- Hashing and multipart ETag comparison should remain streaming/file-based on the Rust side; do not load large files fully into frontend memory.

Runtime direction:

- Final delivery remains a desktop executable unless the contract changes.
- PC and Web are now treated as two runtime targets behind a runtime bridge concept.
- Desktop runtime uses Tauri IPC, local filesystem access, and OS keyring.
- Web runtime requires a backend API service for S3/CDN operations and cannot use OS keyring or unrestricted local filesystem access directly.
