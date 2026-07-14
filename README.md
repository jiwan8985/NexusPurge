# NexusPurge

> S3 파일 배포 · CDN 자동 Purge 데스크톱 도구

FTP 스타일 듀얼 패널 UI로 로컬 파일 시스템과 AWS S3 버킷을 나란히 탐색하고, 파일을 업로드·다운로드합니다.
파일 덮어쓰기가 감지되면 **CDN 캐시를 자동으로 Purge**하여 배포 반영 속도를 단축합니다.

---

## 목차

1. [주요 기능](#주요-기능)
2. [시스템 아키텍처](#시스템-아키텍처)
3. [핵심 데이터 흐름](#핵심-데이터-흐름)
4. [CDN 제공자 지원 현황](#cdn-제공자-지원-현황)
5. [프로필 시스템](#프로필-시스템)
6. [Purge 정책 모델](#purge-정책-모델)
7. [업로드 Header/Metadata 정책](#업로드-headermetadata-정책)
8. [런타임 브릿지](#런타임-브릿지)
9. [인증 어댑터](#인증-어댑터)
10. [로그 적재 구조](#로그-적재-구조)
11. [설치 및 실행](#설치-및-실행)
12. [기술 스택](#기술-스택)
13. [프로젝트 구조](#프로젝트-구조)
14. [CDN 어댑터 추가 방법](#cdn-어댑터-추가-방법)
15. [테스트](#테스트)
16. [알려진 제한 및 TODO](#알려진-제한-및-todo)
17. [라이선스](#라이선스)

---

## 주요 기능

| 기능 | 설명 |
|------|------|
| **듀얼 패널 탐색** | 로컬 파일 시스템 ↔ S3 버킷을 나란히 탐색, 컨텍스트 메뉴 지원 |
| **Smart Sync** | MD5/ETag 비교로 변경된 파일만 전송, 동일 파일 자동 스킵 |
| **동기화 미리보기** | 업로드 전 신규·수정·삭제·변경없음 4탭으로 변경 내역 드라이런 확인 |
| **CDN 자동 Purge** | 덮어쓰기 감지 시 CDN 캐시 무효화 자동 실행, 배치 1,000개 단위 |
| **Purge 승인 정책** | 기본 수동 Purge, 자동 Purge 시 사전 승인 팝업, 대용량 Purge 경고 |
| **Purge 대상 선택** | 전체·개별·일부 선택 Purge, Overwrite/Skip 정책 연계 |
| **Header/Metadata** | Content-Type, Cache-Control, 사용자 정의 헤더·메타데이터 입력, 실패 시 수동 재시도 |
| **실시간 진행률** | 파일별 전송 속도(MB/s)·잔여 시간·진행 바 표시, 취소 지원 |
| **Presigned URL** | S3 객체에 대한 15분·1시간·24시간 임시 공개 URL 생성·복사 |
| **다중 프로필** | 여러 S3 계정/버킷 + 멀티 CDN을 프로필로 저장, 즉시 전환 |
| **프로필 Import/Export** | `.json`/`.nexprofile`(AES-256-GCM 암호화) 파일 가져오기로만 등록하는 write-only 정책 — 목록은 이름만 노출, 연결·테스트·내보내기·삭제만 가능, 수정은 삭제 후 재등록 |
| **보안 자격증명** | SecretAccessKey·CDN 패스워드를 OS 키링(Credential Manager/Keychain)에 암호화 보관 |
| **작업 로그** | 업로드·다운로드·삭제·Purge·Sync 작업 결과 로컬 JSON 로그 적재 |
| **가상 스크롤** | 수만 개 파일도 끊김 없이 렌더링, 전송 큐·로그 항목 수 상한 관리 |
| **드래그 앤 드롭** | 로컬 → 리모트 패널에 파일을 끌어다 놓아 업로드 |
| **멀티파트 업로드** | 10MB 이상 파일 자동 멀티파트 처리, ETag 비교 fallback 지원 |
| **동시 전송 제어** | Semaphore 기반 최대 4파일 동시 전송, CPU·메모리 스파이크 방지 |

---

## 시스템 아키텍처

```
┌──────────────────────────────────────────────────────────────────┐
│  React Frontend  (Vite + TypeScript + Zustand)                   │
│                                                                  │
│  ┌────────────┐  ┌─────────────┐   ┌────────────────────────┐  │
│  │ LocalPanel │  │ RemotePanel │   │  ProfileModal          │  │
│  │  (로컬 FS) │  │   (S3)      │   │  PurgeDialog           │  │
│  └────────────┘  └─────────────┘   │  SyncPreviewDialog     │  │
│        ↕ 선택/탐색                  │  UploadOptionsModal    │  │
│  ┌─────────────────────────────┐   └────────────────────────┘  │
│  │  Zustand Store (appStore)   │                                 │
│  │  - 패널 상태, 선택 항목       │                                 │
│  │  - 전송 큐, 로그              │                                 │
│  │  - 현재 프로필                │                                 │
│  └──────────────┬──────────────┘                                │
│                 │                                                │
│  ┌──────────────▼──────────────────────────────────────────┐   │
│  │  Hooks Layer                                             │   │
│  │  useTransfer  → build_sync_plan / start_uploads         │   │
│  │  useS3        → list / delete / presign / mkdir         │   │
│  │  useProfile   → load / save / connect / import          │   │
│  │  usePurge     → purge_cdn_paths (수동 Purge)            │   │
│  │  useLocalFs   → 로컬 디렉터리 탐색                        │   │
│  │  useVirtualList → 가상 스크롤 (10,000개+ 성능)           │   │
│  └──────────────┬──────────────────────────────────────────┘   │
│                 │ RuntimeBridge (invoke / listen)               │
└─────────────────┼────────────────────────────────────────────────┘
                  │
         ┌────────▼────────┐
         │  RuntimeBridge  │
         │  desktop-runtime│ ← @tauri-apps/api (IPC)
         │  web-runtime    │ ← 백엔드 API HTTP (향후)
         └────────┬────────┘
                  │ Tauri IPC (invoke / emit)
┌─────────────────▼────────────────────────────────────────────────┐
│  Rust Backend  (Tauri 2 + Tokio)                                  │
│                                                                   │
│  commands/sync.rs                                                 │
│    build_sync_plan()   → 로컬 MD5 계산 + S3 HeadObject 병렬 비교 │
│    start_uploads()     → Semaphore 4개 동시, 진행률 emit         │
│    start_downloads()   → Semaphore 4개 동시, 취소 지원          │
│    sync_preview()      → 드라이런 (크기 stat만, MD5 없음)        │
│                                                                   │
│  commands/s3.rs                                                   │
│    list_s3_objects()   → S3 ListObjectsV2 (페이지네이션)         │
│    delete_s3_objects() → 병렬 삭제                               │
│    create_s3_folder()  → 0byte 키 생성                           │
│    generate_presigned_url() → SigV4 서명 URL                    │
│                                                                   │
│  commands/cdn.rs                                                  │
│    purge_cdn_paths()   → 수동 Purge (배치 1,000개)               │
│    get_purge_status()  → Invalidation 상태 조회                  │
│    test_cdn_connection() → CDN 연결 테스트                       │
│    check_cdn_urls()    → 캐시 갱신 여부 HTTP 확인               │
│                                                                   │
│  commands/auth.rs      → 외부 인증 Stub                          │
│  commands/operation_log.rs → 작업 로그 저장·조회                 │
│  commands/log_shipping.rs  → S3 로그 적재 Stub                  │
│                                                                   │
│  ┌───────────────────────────────────────────────────────┐      │
│  │  adapters/storage/s3.rs  (S3Adapter)                  │      │
│  │  - SigV4 자체 서명 (hmac + sha2)                      │      │
│  │  - 멀티파트 업로드 10MB+ (PART_SIZE 8MB)              │      │
│  │  - 멀티파트 다운로드 10MB+ (Range GET 파트 병렬)      │      │
│  │  - upload_with_options() → 진행 콜백 + 취소 지원      │      │
│  │  - head_object_meta() → ETag + size 조회              │      │
│  │  - list_objects() → 페이지네이션 전체 수집             │      │
│  └───────────────────────────────────────────────────────┘      │
│                                                                   │
│  ┌───────────────────────────────────────────────────────┐      │
│  │  adapters/cdn/                                        │      │
│  │  cloudfront.rs → CreateInvalidation API               │      │
│  │  akamai.rs     → EdgeGrid Fast Purge API              │      │
│  │  lguplus.rs    → Solbox CDN v2 JWT + Purge           │      │
│  │  kt.rs         → Solbox CDN v3 JWT + Purge           │      │
│  │  hyosung.rs    → 헤더 인증(X-ITX-*) 배치 Purge         │      │
│  └───────────────────────────────────────────────────────┘      │
│                                                                   │
│  utils/sigv4.rs        → AWS SigV4 서명 자체 구현               │
│  utils/hash.rs         → MD5 / 멀티파트 ETag 계산               │
│  utils/config.rs       → ProfileStore + OS keyring 연동         │
│  utils/adapter_cache.rs → S3Adapter 인스턴스 캐시               │
│  utils/retry.rs        → Exponential Backoff 재시도             │
│  utils/crypto.rs       → 프로필 암호화/복호화                    │
│  utils/transfer_control.rs → 전송 취소 신호 관리               │
└─────────────────────────────────────────────────────────────────┘
```

---

## 핵심 데이터 흐름

### 업로드 (Smart Sync)

```
useTransfer.startUpload(localPaths, remotePrefix)
  │
  ├─ 1. invoke("build_sync_plan", { profileId, localPaths, remotePrefix })
  │       ├─ 폴더 경로 → 재귀 파일 확장
  │       ├─ 각 파일에 대해 JoinSet 병렬 실행:
  │       │    - 로컬 MD5 계산 (10MB+ → 멀티파트 ETag 형식 "hash-N")
  │       │    - S3 HeadObject → ETag + size 조회
  │       └─ 결과 3분류:
  │            toUpload   → 신규 (S3에 없음)
  │            toSkip     → ETag 일치 → 스킵
  │            toOverwrite→ ETag 불일치 → 덮어쓰기 후 CDN Purge 대상
  │
  ├─ 2. SyncPreviewDialog → 4탭 미리보기 (신규/수정/삭제/변경없음)
  │
  ├─ 3. invoke("start_uploads", { profileId, items, cdnProvider, cdnDistributionId })
  │       ├─ CDN 자격증명 사전 조회 (ProfileStore → keyring)
  │       ├─ Semaphore(4) 동시 제한
  │       ├─ 각 파일:
  │       │    upload_with_options() → S3 PutObject / 멀티파트
  │       │    emit("transfer:progress", { id, progress, speed })
  │       │    emit("transfer:complete", { id, status })
  │       └─ 업로드 완료 후 isOverwrite 항목 일괄 CDN Purge:
  │            paths.chunks(1000) → purge_with_credentials()
  │            emit("transfer:complete", { cdnPurged, cdnInvalidationId })
  │
  └─ 4. useTransfer listen("transfer:progress" / "transfer:complete")
         → Zustand 상태 업데이트 → UI 리렌더
```

### 수동 CDN Purge

```
PurgeDialog (경로 선택 / 전체 / 일부)
  │
  ├─ PurgePolicy 확인:
  │    mode=manual     → 즉시 실행
  │    mode=automatic  → 승인 팝업 → 확인 후 실행
  │    paths >= 경고임계값 → 대용량 경고 표시
  │
  ├─ usePurge.executePurge(paths, policy)
  │    → invoke("purge_cdn_paths", { profileId, paths, distributionId })
  │
  ├─ paths.chunks(1000) → 배치 순차 실행
  │    purge_with_credentials(distributionId, batch, cdnCreds)
  │    → CDN별 어댑터 호출 (3회 Exponential Backoff 재시도)
  │
  └─ PurgeResultDialog → 배치별 성공/실패 상세 + invalidationId
```

### 프로필 자격증명 저장 흐름

```
ProfileModal → invoke("save_profile", profile)
  │
  ├─ ProfileStore::save_profile()
  │    ├─ secretAccessKey   → keyring.set("cdn-upload-tool", profileId)
  │    ├─ lguplusPassword   → keyring.set("cdn-upload-tool", "lguplus:{profileId}")
  │    ├─ ktPassword        → keyring.set("cdn-upload-tool", "kt:{profileId}")
  │    ├─ akamaiClientSecret→ keyring.set("cdn-upload-tool", "akamai:{profileId}")
  │    └─ 나머지 메타데이터  → ~/.local/share/cdn-upload-tool/profiles.json
  │
  └─ get_cdn_credentials(profileId, provider) 호출 시 keyring에서 복원
```

---

## CDN 제공자 지원 현황

| 제공자 | Purge 구현 | 인증 방식 | 비고 |
|--------|:---------:|----------|------|
| **AWS CloudFront** | ✅ 완료 | AWS SigV4 (accessKeyId / secretAccessKey) | CreateInvalidation API, 멱등성 caller_reference 보장 |
| **Akamai** | ✅ 완료 | EdgeGrid (client_token / client_secret / access_token / host) | Fast Purge API |
| **LG U+ CDN** | ✅ 완료 | Solbox CDN v2 — username / password JWT | purge_paths(), 3회 재시도 |
| **KT CDN** | ✅ 완료 | Solbox CDN v3 — username / password JWT | purge_paths(), 3회 재시도 |
| **효성 ITX CDN** | ✅ 완료 | 헤더 인증 (X-ITX-Security-Principal / X-ITX-Security-Secret) | POST filelist 배치, 응답 meta+data 2차 파싱, 3회 재시도 |

모든 CDN은 `adapters/cdn/purge_with_credentials()` 단일 진입점으로 디스패치되며, 최대 3회 Exponential Backoff 재시도를 적용합니다.

---

## 프로필 시스템

하나의 프로필이 S3 연결 + CDN 연결을 통합 관리합니다.

### 프로필 항목

| 항목 | 필수 | 설명 |
|------|:----:|------|
| 프로필 이름 | ✓ | 식별용 이름 |
| 스코프 | | `project` (팀 공유) / `user` (개인) |
| 버킷 이름 | ✓ | S3 버킷 이름 |
| 리전 | ✓ | `ap-northeast-2` 등 AWS 리전 코드 |
| Base Prefix | | 버킷 내 루트로 사용할 경로 접두사 |
| Access Key ID | ✓ | IAM 액세스 키 |
| Secret Access Key | ✓ | **OS 키링에 암호화 보관** (JSON 미포함) |
| 커스텀 엔드포인트 | | MinIO 등 S3 호환 서비스 URL |
| CDN 제공자 | | CloudFront / Akamai / LG U+ / KT / 효성 ITX |
| CDN Distribution ID | | CloudFront distribution ID |
| CDN Domain | | Purge URL 구성용 도메인 |
| CDN Base Path | | S3 키에서 제거할 CDN 경로 접두사 |
| Purge On New Upload | | 신규 업로드에도 CDN Purge 수행 여부 |
| Purge Policy | | 수동/자동, 배치 크기, 경고 임계값 |
| Upload Policy | | Overwrite/Skip 정책, 배치 크기 |
| Metadata Policy | | Content-Type, Cache-Control, 커스텀 헤더·메타데이터 기본값 |
| Log Shipping | | 고객 제공 S3 버킷으로 작업 로그 적재 설정 |
| Auth Binding | | 외부 인증 연동 Stub (AI LB / external) |

### 권한 모델 (write-only 정책)

역할 기반 접근 제어(`ProfileScope`/`ProfilePermissions`)는 데이터 모델(`config.rs`)에 필드로 존재하지만, 현재 커맨드/UI 어디에서도 값을 검사해 동작을 제한하지 않는다 — 실제로 적용 중인 정책은 아래와 같이 **모든 프로필에 동일하게** 적용되는 write-only 규칙이다.

| 동작 | 가능 여부 |
|------|:---------:|
| 파일로 가져오기(`.json` / `.nexprofile`) | ✓ |
| 연결 / 테스트 | ✓ |
| 내보내기(암호화 파일 재발급) | ✓ |
| 삭제 | ✓ |
| 저장된 값 열람(버킷·키·CDN 설정 등) | ✗ — 이름만 노출 |
| 직접 입력으로 생성/수정 | ✗ — 삭제 후 파일 재가져오기로만 변경 가능 |

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

## Purge 정책 모델

```
PurgePolicy {
  mode:                         "manual" | "automatic"
  requireApprovalBeforeAutomaticPurge: boolean   // 자동 Purge 시 승인 팝업
  requireLargePurgeWarning:     boolean           // 대용량 Purge 경고 표시
  selectionMode:                "all" | "individual" | "partial"
  overwritePolicy:              "overwrite" | "skip"
  batch: {
    batchSize:                  1000              // 기본값, 1회 API 요청 최대 경로 수
    warningThreshold:           5000              // 안전 기본값 (설정에서 변경 가능)
    notRecommendedThreshold:    10000             // 안전 기본값 (설정에서 변경 가능)
  }
}
```

- `mode=manual`: 사용자가 명시적으로 Purge 버튼을 눌러야 실행
- `mode=automatic`: 업로드 완료 즉시 자동 실행, `requireApproval=true`이면 사전 승인 팝업
- Overwrite 감지 파일만 Purge 대상 포함 (Skip 파일 제외)
- `purgeOnNewUpload=true`이면 신규 업로드 파일도 Purge 대상

---

## 업로드 Header/Metadata 정책

```
UploadMetadataPolicy {
  autoApply:      boolean               // 업로드 시 자동 적용
  contentType:    string                // 비어 있으면 확장자 기반 자동 감지
  cacheControl:   string                // 예: "public, max-age=31536000"
  customHeaders:  Record<string,string> // x-amz-* 등 추가 헤더
  userMetadata:   Record<string,string> // S3 user-defined metadata
  allowManualRetryOnFailure: boolean    // 헤더 적용 실패 시 수동 재시도 허용
}
```

메타데이터 적용 실패 시 `MetadataFailureLog`에 기록하고, `allowManualRetryOnFailure=true`이면 UI에서 수동 재시도 버튼 제공.

---

## 런타임 브릿지

NexusPurge는 **데스크톱**과 **웹** 두 가지 런타임을 추상화 레이어로 분리합니다.

```
RuntimeBridge (interface)
  ├─ DesktopRuntime  → @tauri-apps/api (invoke / listen / 로컬 FS / OS keyring)
  └─ WebRuntime      → fetch / WebSocket (백엔드 API 서버 필요, 향후 구현)
```

| 기능 | 데스크톱 | 웹 |
|------|:--------:|:--:|
| 로컬 파일 시스템 접근 | ✓ | ✗ (API 경유) |
| OS 키링 사용 | ✓ | ✗ |
| Tauri IPC | ✓ | ✗ |
| S3/CDN 직접 호출 | ✓ (Rust) | 백엔드 서버 필요 |

현재 배포 형태는 **데스크톱 설치형**입니다. 웹 런타임은 `web-runtime.ts`에 인터페이스만 준비되어 있습니다.

---

## 인증 어댑터

자체 계정 DB·로그인 화면은 없습니다. 인증은 Adapter/Stub 구조로만 준비되어 있습니다.

```
AuthAdapter (trait / interface)
  login()           → AuthSession
  logout()          → void
  refresh_token()   → AuthSession
  current_session() → AuthSession | null

구현체:
  ExternalAuthAdapter (Stub)
    → AI LB 또는 외부 인증 모듈 연동 시 교체
```

프로필의 `authBinding` 필드로 AI LB 또는 외부 인증 세션 연동을 선언적으로 바인딩할 수 있습니다.

---

## 로그 적재 구조

### 로컬 작업 로그

모든 업로드·다운로드·삭제·Purge·Sync 결과가 `OperationLog` 형태로 적재됩니다.

```
OperationLog {
  id, profileId, operation, status
  files:        FileOperationResult[]   // 파일별 성공/실패
  purgeResults: CdnOperationPurgeResult[] // CDN Purge 결과
  metadataFailures: MetadataFailureLog[]  // 헤더 적용 실패 목록
  logShipping:  LogShippingState          // S3 적재 상태
  startedAt, finishedAt
}
```

### CDN API 감사 로그

CDN 어댑터가 호출하는 모든 요청(인증 제외)은 `adapters/cdn/mod.rs::log_cdn_http()`를 거쳐 `tracing`으로 기록되고, `logs/audit.YYYY-MM-DD.log`에 메서드·URL·HTTP 상태·소요시간·응답 본문(최대 1,000자)이 함께 남는다. 콘솔과 파일에 동시 출력된다(`lib.rs`의 `tracing-appender` daily rolling).

### S3 로그 적재 (Stub)

프로필의 `logShipping` 설정으로 고객 제공 S3 버킷에 JSON 포맷으로 로그를 업로드합니다. 실패 시 Exponential Backoff 재시도 구조가 준비되어 있으나, 실제 적재 로직은 Stub 상태입니다.

```
LogShippingConfig {
  enabled:           boolean
  bucket:            string    // 고객 제공 S3 버킷
  prefix:            string    // 로그 경로 접두사
  includeOperations: OperationType[]
  format:            "json"
  retry: { enabled, maxAttempts, backoffMs }
}
```

---

## 설치 및 실행

### 사전 요구사항

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://rustup.rs/) stable
- [Tauri CLI](https://tauri.app/start/prerequisites/) 2.x
- pnpm

```bash
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

## 기술 스택

| 레이어 | 기술 |
|--------|------|
| 프레임워크 | [Tauri 2](https://tauri.app/) |
| 프론트엔드 | React 18 · TypeScript 5 · Vite 5 |
| 상태 관리 | Zustand 4 |
| 스타일 | CSS Modules (외부 UI 라이브러리 미사용) |
| 백엔드 | Rust + Tokio 비동기 런타임 |
| S3 통신 | SigV4 자체 구현 (`hmac` + `sha2` crate) |
| 파일 해시 | `md5` crate (ETag 비교, 멀티파트 ETag 계산) |
| 자격증명 | `keyring` crate (OS 키링 — Windows Credential Manager / macOS Keychain) |
| HTTP | `reqwest` (native-tls) |
| Percent Encoding | `percent-encoding` crate (CDN URL 경로 인코딩) |
| 테스트 | Vitest (프론트엔드) / `cargo test` (Rust) |

---

## 프로젝트 구조

```
NexusPurge/
│
├── src/                               # React 프론트엔드
│   ├── components/
│   │   ├── layout/
│   │   │   ├── TitleBar.tsx           # 프로필 선택 칩, 창 제어 버튼
│   │   │   ├── Toolbar.tsx            # 연결/새 폴더/삭제 버튼
│   │   │   └── StatusBar.tsx          # 연결 상태, 선택 항목 수
│   │   ├── panels/
│   │   │   ├── LocalPanel.tsx         # 로컬 파일 시스템 패널
│   │   │   ├── RemotePanel.tsx        # S3 버킷 패널
│   │   │   └── Panel.module.css       # 공유 패널 스타일
│   │   ├── transfer/
│   │   │   ├── TransferButtons.tsx    # ▶ 업로드 / ◀ 다운로드 버튼
│   │   │   ├── ProgressDialog.tsx     # 플로팅 전송 진행 패널
│   │   │   ├── SyncPreviewDialog.tsx  # 동기화 미리보기 (4탭)
│   │   │   └── UploadOptionsModal.tsx # 업로드 전 Header/Metadata 설정
│   │   ├── sync/
│   │   │   └── SyncPreviewDialog.tsx  # (transfer/ 와 공존, 리팩터링 예정)
│   │   ├── log/
│   │   │   └── LogPanel.tsx           # 작업로그 / 전송큐 / Purge이력 3탭
│   │   ├── modals/
│   │   │   ├── ProfileModal.tsx       # 프로필 생성·편집·Import·Remove
│   │   │   ├── PurgeDialog.tsx        # 수동 Purge 대상 선택 + 정책 설정
│   │   │   ├── PurgeResultDialog.tsx  # Purge 결과 상세 (배치별 성공/실패)
│   │   │   └── SettingsModal.tsx      # 앱 설정
│   │   └── common/
│   │       ├── ContextMenu.tsx        # 우클릭 컨텍스트 메뉴
│   │       ├── ConfirmDialog.tsx      # 범용 확인 다이얼로그
│   │       └── InputDialog.tsx        # 범용 입력 다이얼로그
│   │
│   ├── hooks/
│   │   ├── useTransfer.ts             # 업로드·다운로드·SyncPlan, 이벤트 구독
│   │   ├── useS3.ts                   # S3 CRUD, Presigned URL
│   │   ├── useProfile.ts             # 프로필 CRUD, 연결
│   │   ├── usePurge.ts               # 수동 CDN Purge, 상태 조회
│   │   ├── useLocalFs.ts             # 로컬 디렉터리 탐색
│   │   └── useVirtualList.ts         # 가상 스크롤 (10,000개+ 성능)
│   │
│   ├── services/
│   │   ├── runtime/
│   │   │   ├── runtime-types.ts      # RuntimeBridge interface
│   │   │   ├── desktop-runtime.ts    # Tauri IPC 구현
│   │   │   ├── web-runtime.ts        # 웹 런타임 Stub
│   │   │   └── index.ts              # 런타임 선택 (환경 감지)
│   │   ├── auth/
│   │   │   ├── auth-types.ts         # AuthAdapter interface
│   │   │   └── external-auth-adapter.ts # 외부 인증 Stub
│   │   └── operation-log/
│   │       ├── operation-log-types.ts
│   │       ├── operation-log-store.ts
│   │       └── operation-log-service.ts
│   │
│   ├── store/
│   │   └── appStore.ts               # Zustand 전역 상태
│   ├── types/
│   │   └── index.ts                  # 공유 타입 (Rust 구조체와 동기화 필수)
│   ├── utils/
│   │   ├── batch-settings.ts         # Purge 배치 기본값
│   │   └── cdn.ts                    # CDN URL 헬퍼
│   └── styles/
│       ├── variables.css             # 디자인 토큰 (색상, 간격, 폰트)
│       └── global.css                # 전역 리셋 및 기반 스타일
│
└── src-tauri/src/                    # Rust 백엔드
    ├── commands/
    │   ├── sync.rs                   # build_sync_plan / start_uploads / start_downloads / sync_preview
    │   ├── s3.rs                     # S3 CRUD + Presigned URL
    │   ├── cdn.rs                    # purge_cdn_paths / get_purge_status / check_cdn_urls
    │   ├── auth.rs                   # 외부 인증 Stub
    │   ├── operation_log.rs          # 작업 로그 저장·조회
    │   ├── log_shipping.rs           # S3 로그 적재 Stub
    │   └── mod.rs
    ├── adapters/
    │   ├── storage/
    │   │   ├── base.rs               # StorageAdapter trait
    │   │   ├── s3.rs                 # S3Adapter (SigV4 자체 구현, 멀티파트)
    │   │   └── mod.rs
    │   └── cdn/
    │       ├── base.rs               # CdnAdapter trait
    │       ├── cloudfront.rs         # CloudFront CreateInvalidation
    │       ├── akamai.rs             # Akamai EdgeGrid Fast Purge
    │       ├── lguplus.rs            # LG U+ Solbox CDN v2
    │       ├── kt.rs                 # KT Solbox CDN v3
    │       ├── hyosung.rs            # 효성 ITX (Stub)
    │       ├── mock.rs               # 테스트용 Mock
    │       └── mod.rs                # purge_with_credentials() 디스패처
    ├── services/
    │   ├── auth.rs                   # AuthAdapter trait + ExternalAuthAdapter Stub
    │   ├── operation_log.rs          # 작업 로그 서비스
    │   ├── log_shipping.rs           # S3 로그 적재 서비스 Stub
    │   └── mod.rs
    └── utils/
        ├── sigv4.rs                  # AWS SigV4 서명 자체 구현
        ├── hash.rs                   # MD5 / 멀티파트 ETag 계산
        ├── config.rs                 # ProfileStore + CdnCredentials + OS keyring 연동
        ├── adapter_cache.rs          # S3Adapter 인스턴스 재사용 캐시
        ├── retry.rs                  # Exponential Backoff 재시도
        ├── crypto.rs                 # 프로필 파일 암호화/복호화
        ├── transfer_control.rs       # 전송 취소 신호 관리
        └── mod.rs
```

---

## CDN 어댑터 추가 방법

새 CDN을 지원하려면 4곳을 수정합니다.

**1. Rust 어댑터 구현**

```rust
// src-tauri/src/adapters/cdn/new_cdn.rs
use anyhow::Result;

pub struct NewCdnAdapter { /* 자격증명 필드 */ }

impl NewCdnAdapter {
    pub fn new(/* 자격증명 */) -> Result<Self> { ... }
    pub async fn purge_paths(&self, paths: &[String]) -> Result<()> { ... }
}
```

**2. 디스패처에 match arm 추가**

```rust
// src-tauri/src/adapters/cdn/mod.rs
// CdnCredentials enum에 variant 추가
// purge_with_credentials() match arm 추가
```

**3. TypeScript 타입 추가**

```typescript
// src/types/index.ts
export type CdnProvider = "cloudfront" | "akamai" | "lguplus" | "hyosung" | "kt" | "new_cdn";
```

**4. 프로필 모달 UI 추가**

```typescript
// src/components/modals/ProfileModal.tsx
const CDN_PROVIDERS = [
  ...
  { value: "new_cdn", label: "New CDN" },
];
```

---

## 테스트

```bash
# 프론트엔드 단위 테스트 (Vitest)
pnpm test

# 감시 모드
pnpm test:watch

# Rust 단위 테스트
cargo test --manifest-path src-tauri/Cargo.toml
```

Rust 테스트에는 `build_cdn_url` 정규화 검증, 멀티파트 ETag 파싱 등이 포함됩니다.

---

## 알려진 제한 및 TODO

| 항목 | 상태 | 비고 |
|------|------|------|
| 효성 ITX CDN Purge 대용량 임계값 | TODO | 대량 경로 Purge 시 서버 분할 처리 한계치 고객 확인 필요 |
| S3 로그 적재 | ⏳ Stub | 고객 버킷 정보 확인 후 구현 |
| 외부 인증 (AI LB) | ⏳ Stub | 인증 모듈 연동 계약 후 구현 |
| 멀티파트 ETag 비교 | 제한 | ≥10MB 파일은 ETag 형식 불일치 가능 → `multipartEtagFallback` 옵션으로 크기 fallback 비교 |
| Windows Server 지원 범위 | TODO | 2019/2022/2025 → 고객 확인 필요 |
| 자동 패치/보안 업데이트 | TODO | 재시작 없는 패치 가능 여부 → 고객 확인 필요 |
| 웹 런타임 | 인터페이스만 | 실제 웹 배포는 별도 백엔드 API 서버 필요 |

---

## 라이선스

MIT
