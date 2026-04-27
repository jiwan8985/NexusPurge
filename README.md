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
| **실시간 진행률** | 파일별 전송 속도(MB/s) · 잔여 시간 · 진행 바 표시 |
| **Presigned URL** | S3 객체에 대한 1시간 임시 공개 URL 생성 · 복사 |
| **다중 프로필** | 여러 S3 계정/버킷을 프로필로 저장, 빠른 전환 |
| **보안 자격증명** | SecretAccessKey를 OS 키링(Credential Manager/Keychain)에 암호화 보관 |
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

`main` 브랜치 push 또는 수동 실행 시 GitHub Actions가 3개 OS에서 동시 빌드합니다.

```
push → main
  ├─ windows-latest  →  .msi  +  .exe
  ├─ macos-latest    →  .dmg  +  .app
  └─ ubuntu-latest   →  .AppImage
```

워크플로우 파일: [`.github/workflows/build.yml`](.github/workflows/build.yml)

**릴리즈 배포**: Actions 탭 → Build → `workflow_dispatch` → `release_tag` 입력 시 GitHub Draft Release 자동 생성.

---

## 프로젝트 구조

```
NexusPurge/
├── src/                        # React 프론트엔드
│   ├── components/
│   │   ├── layout/             # TitleBar · Toolbar · StatusBar
│   │   ├── panels/             # LocalPanel · RemotePanel
│   │   ├── transfer/           # TransferButtons · ProgressDialog
│   │   ├── log/                # LogPanel (3탭)
│   │   ├── modals/             # ProfileModal
│   │   └── common/             # ContextMenu
│   ├── hooks/
│   │   ├── useTransfer.ts      # 업로드/다운로드 + 이벤트 구독
│   │   ├── useS3.ts            # S3 CRUD · Presigned URL
│   │   ├── useProfile.ts       # 프로필 CRUD · 연결
│   │   └── useVirtualList.ts   # 가상 스크롤 (10,000개+ 성능)
│   ├── store/appStore.ts       # Zustand 전역 상태
│   ├── types/index.ts          # 공유 타입 (Rust 구조체와 동기화)
│   └── styles/                 # CSS 변수 · 전역 스타일
│
└── src-tauri/src/              # Rust 백엔드
    ├── commands/               # Tauri invoke 핸들러
    ├── adapters/storage/       # S3Adapter (멀티파트)
    ├── adapters/cdn/           # CloudFrontAdapter
    └── utils/                  # SigV4 · MD5 · ProfileStore
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

## 알려진 제한

- Akamai · LG U+ · 효성 ITX CDN Purge는 스텁 상태 (미구현)
- 대용량 파일(≥10MB) 다운로드는 단일 GET으로 처리 (멀티파트 다운로드 미지원)
- Toolbar의 파일 작업 버튼(새 폴더 · 삭제 · 이름 변경)은 UI만 존재, 연결 예정

---

## 라이선스

MIT
