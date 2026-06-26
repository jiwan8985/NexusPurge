# NexusPurge: API 및 연동 가이드 (상세 기술 명세)

본 가이드는 **NexusPurge**에서 사용하는 IPC 커맨드 엔드포인트, 이벤트 페이로드, 디렉토리 구조 및 TypeScript 인터페이스 매핑에 대한 종합적인 기술 참조를 제공합니다.

---

## 1. 애플리케이션 디렉토리 및 저장 경로

NexusPurge는 운영체제 표준에 따라 지정된 로컬 데이터 폴더 아래에 평문 메타데이터 설정을 저장합니다.

- **Windows**: `C:\Users\<사용자명>\AppData\Local\cdn-upload-tool\`
- **macOS**: `/Users/<사용자명>/Library/Application Support/cdn-upload-tool/`
- **Linux**: `/home/<사용자명>/.local/share/cdn-upload-tool/`

### 파일 레이아웃
```
cdn-upload-tool/
├── profiles.json       # 평문 프로필 설정 목록 (자격증명 비밀키 제외)
├── settings.json       # 전역 애플리케이션 설정 (예: lastProfileId)
└── operation_logs/     # (OperationLogService에서 생성)
    ├── log_tx123.json  # 작업 트랜잭션 JSON 로그 파일
    └── index.json      # 작업 인덱스 목록
```

---

## 2. Tauri IPC 커맨드 상세 규격

모든 백엔드 커맨드 핸들러는 Rust에서 비동기 함수로 매핑되며, 프론트엔드 TypeScript에서 `runtime.invoke<T>(cmdName, payload)`를 통해 호출됩니다.

### 2.1 프로필 및 연결 관련 커맨드

#### `load_profiles`
S3 및 CDN 메타데이터 구성 프로필 목록을 불러옵니다.
- **Tauri 호출명**: `load_profiles`
- **Rust 시그니처**: `pub async fn load_profiles(store: State<'_, ProfileStore>) -> Result<Vec<ProfileConfig>, String>`
- **TypeScript 응답**: `S3Profile[]` ([src/types/index.ts](file:///D:/Projects/NexusPurge/src/types/index.ts)에 정의됨)

#### `save_profile`
프로필 설정을 저장하거나 덮어씁니다. 메타데이터는 `profiles.json`에 저장하고, 비밀키는 시스템 Keyring에 안전하게 저장합니다.
- **Tauri 호출명**: `save_profile`
- **Rust 시그니처**: `pub async fn save_profile(profile: ProfileConfig, store: State<'_, ProfileStore>) -> Result<(), String>`
- **페이로드 매개변수**:
  - `profile: ProfileConfig` (Rust serde 명명 규칙을 준수해야 함)

---

### 2.2 S3 파일 브라우저 커맨드

#### `list_s3_objects`
버킷 접두사(Prefix) 내의 오브젝트 목록을 조회합니다.
- **Tauri 호출명**: `list_s3_objects`
- **Rust 시그니처**:
  ```rust
  pub async fn list_s3_objects(
      profile_id: String,
      prefix: String,
      continuation_token: Option<String>,
      store: State<'_, ProfileStore>,
      cache: State<'_, AdapterCache>,
  ) -> Result<S3ListResponse, String>
  ```
- **TypeScript 요청 페이로드**:
  ```typescript
  {
    profileId: string;
    prefix: string;
    continuationToken?: string;
  }
  ```
- **TypeScript 응답**:
  ```typescript
  interface S3ListResponse {
    files: FileItem[];
    nextContinuationToken?: string;
    isTruncated: boolean;
  }
  ```

---

### 2.3 CDN 캐시 무효화(Purge) 커맨드

#### `purge_cdn`
여러 S3 키에 대한 캐시 무효화 요청을 트리거하며, 요청 전 `cdn_base_path` 필터를 적용하여 경로를 보정합니다.
- **Tauri 호출명**: `purge_cdn`
- **Rust 시그니처**:
  ```rust
  pub async fn purge_cdn(
      profile_id: String,
      provider: String,
      distribution_id: String,
      paths: Vec<String>,
      store: State<'_, ProfileStore>,
  ) -> Result<CdnPurgeResult, String>
  ```
- **TypeScript 요청 페이로드**:
  ```typescript
  {
    profileId: string;
    provider: CdnProvider;
    distributionId: string;
    paths: string[];
  }
  ```
- **TypeScript 응답**:
  ```typescript
  interface CdnPurgeResult {
    success: boolean;
    provider: CdnProvider;
    invalidationId?: string; // CDN 응답에서 받은 transactionId
    paths: string[];
    purgedAt?: string; // ISO 8601 형식 문자열
    error?: string;
  }
  ```

#### `get_purge_status`
CDN 무효화 트랜잭션의 처리 진행 상태를 조회합니다.
- **Tauri 호출명**: `get_purge_status`
- **Rust 시그니처**:
  ```rust
  pub async fn get_purge_status(
      profile_id: String,
      provider: String,
      distribution_id: String,
      invalidation_id: String,
      store: State<'_, ProfileStore>,
  ) -> Result<CdnPurgeStatusResult, String>
  ```
- **TypeScript 요청 페이로드**:
  ```typescript
  {
    profileId: string;
    provider: CdnProvider;
    distributionId: string;
    invalidationId: string; // 저장된 transactionId
  }
  ```
- **TypeScript 응답**:
  ```typescript
  interface CdnPurgeStatusResult {
    success: boolean;
    provider: CdnProvider;
    status?: string; // "running" | "completed" | "failed"
    message?: string;
    error?: string;
  }
  ```

---

## 3. 비동기 이벤트 및 페이로드

Tauri는 진행 상황 및 태스크 상태 알림을 프론트엔드로 실시간 전송하기 위해 이벤트 스트림을 사용합니다. 프론트엔드에서는 `@tauri-apps/api/event` 모듈의 `listen()` 함수를 통해 이벤트를 수신합니다.

### 3.1 이벤트명: `transfer:progress`
- **전송 방향**: Rust 백엔드 -> React 프론트엔드
- **발생 빈도**: 파일 업로드/다운로드 과정에서 매 256KB의 데이터 청크가 전송될 때마다 트리거됩니다.
- **페이로드 스키마**:
  ```typescript
  interface TransferProgressPayload {
    id: string;                // 업로드 항목 고유 UUID
    progress: number;          // 0에서 100 사이의 정수 백분율
    transferredBytes: number;  // 전송 완료된 누적 바이트 수
    speed: number;             // 초당 전송 바이트 수 (전송 속도)
    status: "uploading" | "downloading";
  }
  ```
- **사용 예시**:
  ```typescript
  import { listen } from "@tauri-apps/api/event";

  const unlisten = await listen<TransferProgressPayload>("transfer:progress", (event) => {
    const { id, progress, speed } = event.payload;
    updateTransferState(id, { progress, speed });
  });
  ```

### 3.2 이벤트명: `transfer:complete`
- **전송 방향**: Rust 백엔드 -> React 프론트엔드
- **발생 시점**: S3 쓰기 및 CDN Purge를 포함한 파일 전송 수명 주기가 완료되었을 때 트리거됩니다.
- **페이로드 스키마**:
  ```typescript
  interface TransferCompletePayload {
    id: string;
    status: "complete" | "canceled" | "error";
    cdnPurged: boolean;
    cdnPurgeError?: string;
    cdnInvalidationId?: string; // KT, LG U+, 효성 등에서 반환된 transactionId
    error?: string;
  }
  ```
- **사용 예시**:
  ```typescript
  const unlisten = await listen<TransferCompletePayload>("transfer:complete", (event) => {
    const { id, status, cdnPurged, cdnInvalidationId } = event.payload;
    markTransferFinished(id, { status, cdnPurged, invalidationId: cdnInvalidationId });
  });
  ```

