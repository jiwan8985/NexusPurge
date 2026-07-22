// ─── S3 Profile ──────────────────────────────────────────────────────────────

export interface S3Profile {
  id: string;
  name: string;
  scope?: ProfileScope;
  permissions?: ProfilePermissions;
  region: string;
  bucket: string;
  basePrefix?: string;
  accessKeyId?: string;           // keyring에 저장, 로드 시 빈 값
  secretAccessKey: string;
  endpoint?: string;              // S3-compatible 서비스용 커스텀 엔드포인트
  cdnProvider?: CdnProvider;
  cdnProviders?: CdnProviderConfig[];
  cdnDistributionId?: string;     // CloudFront distribution ID
  cdnDomain?: string;             // CDN 도메인 (Purge URL 구성용)
  cdnBasePath?: string;           // S3 키에서 제거할 CDN 경로 접두사 (예: "contents/" → CDN에서 스트립)
  purgeOnNewUpload?: boolean;     // 신규 업로드에도 CDN Purge 수행
  purgePolicy?: PurgePolicy;
  uploadPolicy?: UploadPolicy;
  metadataPolicy?: UploadMetadataPolicy;
  logShipping?: LogShippingConfig;
  authBinding?: ExternalAuthBinding;
  defaultCacheControl?: string;   // 업로드 기본 Cache-Control
  contentTypeOverride?: string;   // 비어 있으면 확장자 기반 자동 감지
  multipartEtagFallback?: boolean; // multipart ETag 불일치 시 크기 fallback 비교
  // H-6: Akamai EdgeGrid 자격증명
  akamaiClientToken?: string;     // Akamai EdgeGrid client token
  akamaiClientSecret?: string;    // 저장 시 keyring에 보관, 로드 시 빈 값
  akamaiAccessToken?: string;     // Akamai EdgeGrid access token
  akamaiHost?: string;            // EdgeGrid API 호스트 (e.g. akab-xxxx.luna.akamaiapis.net)
  akamaiCpCode?: string;          // Purge 대상 CP Code — 폴더/전체(와일드카드) Purge에 사용
  // LG U+ CDN (Solbox CDN v2)
  lguplusUsername?: string;
  lguplusPassword?: string;    // keyring에 저장, 로드 시 빈 값
  lguplusServiceName?: string;
  lguplusVolumeName?: string;
  lguplusEndpoint?: string;
  lguplusServiceType?: "cloudcdn" | "volume"; // cloudcdn이면 전체 Purge 시 서비스 전체 즉시 플러시 사용
  // KT CDN (Solbox CDN v3)
  ktUsername?: string;
  ktPassword?: string;         // keyring에 저장, 로드 시 빈 값
  ktServiceName?: string;
  ktVolumeName?: string;
  ktEndpoint?: string;
  ktServiceType?: "cloudcdn" | "volume"; // cloudcdn이면 전체 Purge 시 서비스 전체 즉시 플러시 사용
  // Hyosung CDN (미지원, 하위 호환)
  hyosungApiKey?: string;
  hyosungApiSecret?: string;
  hyosungEndpoint?: string;
  createdAt: string;
  updatedAt: string;
}

export type ProfileScope = "project" | "user";

export type ProfilePermissionRole = "admin" | "operator" | "viewer";

export interface ProfilePermissions {
  role: ProfilePermissionRole;
  canImport: boolean;
  canRemove: boolean;
  canCreate: boolean;
  canEdit: boolean;
  canPurge: boolean;
  canManageSecrets: boolean;
}

// ─── File System ─────────────────────────────────────────────────────────────

export interface FileItem {
  name: string;
  path: string;              // 로컬: 절대경로, 리모트: S3 key
  size: number;
  lastModified: string;      // ISO 8601
  isDirectory: boolean;
  etag?: string;             // S3 ETag (덮어쓰기 감지용)
  contentType?: string;
}

export interface DirectoryEntry {
  path: string;
  files: FileItem[];
  totalSize: number;
}

// ─── Transfer ────────────────────────────────────────────────────────────────

export type TransferDirection = "upload" | "download";

export type TransferStatus =
  | "pending"
  | "uploading"
  | "downloading"
  | "hashing"      // MD5 계산 중
  | "skipped"      // ETag 동일 → 스킵
  | "overwriting"  // ETag 다름 → 덮어쓰기
  | "complete"
  | "canceled"
  | "error";

export interface TransferItem {
  id: string;
  direction: TransferDirection;
  localPath: string;
  remotePath: string;         // S3 key
  fileName: string;
  size: number;
  status: TransferStatus;
  progress: number;           // 0-100
  transferredBytes: number;
  speed?: number;             // bytes/sec
  error?: string;
  cdnPurged?: boolean;
  cdnPurgeError?: string;
  cdnPurgeStatus?: "notRequested" | "pending" | "inProgress" | "complete" | "error";
  cdnInvalidationId?: string;
  cdnUrl?: string;
  cdnVerified?: boolean;
  cdnStatusCode?: number;
  cdnCheckError?: string;
  startedAt?: string;
  completedAt?: string;
}

// ─── Network Stats ──────────────────────────────────────────────────────────

/** Rust `network:stats` 이벤트 payload — S3/CDN 호출 평균 응답시간과 활성 S3 요청 수 */
export interface NetworkStatsEvent {
  avgRttMs: number | null;
  activeS3Calls: number;
}

export interface TransferSummary {
  total: number;
  completed: number;
  failed: number;
  skipped: number;
  totalBytes: number;
  transferredBytes: number;
  cdnPurgeCount: number;
}

// ─── CDN ─────────────────────────────────────────────────────────────────────

export type CdnProvider = "cloudfront" | "akamai" | "lguplus" | "hyosung" | "kt";

export interface CdnProviderConfig {
  provider: CdnProvider;
  displayName?: string;
  enabled: boolean;
  distributionId?: string;
  domain?: string;
}

export type PurgeMode = "manual" | "automatic";
export type PurgeSelectionMode = "all" | "individual" | "partial";
export type OverwritePolicy = "overwrite" | "skip";

export interface PurgeBatchPolicy {
  batchSize: number;
  warningThreshold: number;
  notRecommendedThreshold: number;
}

export interface PurgePolicy {
  mode: PurgeMode;
  requireApprovalBeforeAutomaticPurge: boolean;
  requireLargePurgeWarning: boolean;
  selectionMode: PurgeSelectionMode;
  overwritePolicy: OverwritePolicy;
  batch: PurgeBatchPolicy;
}

export interface UploadPolicy {
  overwritePolicy: OverwritePolicy;
  batchSize: number;
}

export interface UploadMetadataPolicy {
  autoApply: boolean;
  contentType?: string;
  cacheControl?: string;
  customHeaders: Record<string, string>;
  userMetadata: Record<string, string>;
  allowManualRetryOnFailure: boolean;
}

export interface RetryPolicy {
  enabled: boolean;
  maxAttempts: number;
  backoffMs: number;
}

export interface LogShippingConfig {
  enabled: boolean;
  bucket?: string;
  prefix?: string;
  includeOperations: OperationType[];
  format: "json";
  retry: RetryPolicy;
}

export interface ExternalAuthBinding {
  provider: "ai-lb" | "external";
  keyRef?: string;
  sessionRef?: string;
  requiredRoles?: string[];
}

export type RuntimeTarget = "desktop" | "web";

export type DeliveryMode = "desktop-executable" | "web-hosted";

export interface RuntimeCapabilities {
  target: RuntimeTarget;
  deliveryMode: DeliveryMode;
  canAccessLocalFileSystem: boolean;
  canUseOsKeyring: boolean;
  canUseTauriIpc: boolean;
}

export interface PerformanceLimits {
  maxConcurrentTransfers: number;
  maxCdnPurgeUrlsPerRequest: number;
  maxVisibleTransferRows: number;
  maxLogEntries: number;
}

export interface CdnPurgeRequest {
  provider: CdnProvider;
  distributionId: string;
  paths: string[];
  policy?: PurgePolicy;
}

/** cdn-*.log에 provider 블록 하위로 기록되는 개별 HTTP 호출(인증 → purge 등) 1건 */
export interface CdnRequestStep {
  method: string;
  url: string;
  status: number;
  statusText: string;
  elapsedMs: number;
  summary: string;
}

export interface CdnPurgeResult {
  success: boolean;
  provider: CdnProvider;
  invalidationId?: string;
  paths: string[];
  purgedAt?: string;
  error?: string;
  /** 실제 호출된 CDN Purge API 엔드포인트 (감사/디버깅용) */
  requestEndpoint?: string;
  /** 요청 소요 시간 (ms) */
  durationMs?: number;
  /** 이 Purge 요청 과정에서 실제 발생한 HTTP 호출 단계 (상태코드·소요시간 포함) */
  requestSteps?: CdnRequestStep[];
}

export interface PurgeBatchResult {
  paths: string[];
  success: boolean;
  invalidationId?: string;
  error?: string;
  startedAt: string;
  finishedAt: string;
  requestEndpoint?: string;
  durationMs?: number;
  requestSteps?: CdnRequestStep[];
}

export interface PurgeExecutionResult {
  provider: CdnProvider;
  domain?: string;
  totalPaths: number;
  batches: PurgeBatchResult[];
  successCount: number;
  failedCount: number;
  startedAt: string;
  finishedAt: string;
}

export interface CdnConnectionTestResult {
  success: boolean;
  provider: CdnProvider;
  domain?: string;
  error?: string;
}

/** 속성(우클릭) 다이얼로그 — S3 HeadObject 응답 전체 (크롬 개발자모드 수준의 상세 헤더) */
export interface S3ObjectDetail {
  key: string;
  etag?: string;
  contentLength?: number;
  contentType?: string;
  contentEncoding?: string;
  contentDisposition?: string;
  contentLanguage?: string;
  cacheControl?: string;
  lastModified?: string;
  storageClass?: string;
  serverSideEncryption?: string;
  sseKmsKeyId?: string;
  versionId?: string;
  replicationStatus?: string;
  acceptRanges?: string;
  checksumCrc32?: string;
  checksumSha256?: string;
  metadata: Record<string, string>;
}

/** 속성 다이얼로그의 "실시간 확인" — 임의 URL에 대한 실제 HTTP 응답 상세 */
export interface UrlInspection {
  url: string;
  statusCode?: number;
  headers: [string, string][];
  durationMs: number;
  error?: string;
  errorKind?: "dns" | "timeout" | "connect" | "tls" | "other";
}

export interface CdnPurgeStatusResult {
  success: boolean;
  provider: CdnProvider;
  status?: string;
  message?: string;
  error?: string;
}

export interface AuthUser {
  id: string;
  name: string;
  email: string;
  organization?: string;
  roles: string[];
}

export interface AuthSession {
  user: AuthUser;
  accessToken: string;
  refreshToken?: string;
  expiresAt?: string;
  provider?: "ai-lb" | "external";
}

export interface AuthAdapter {
  login(): Promise<AuthSession>;
  logout(): Promise<void>;
  refreshToken(session: AuthSession): Promise<AuthSession>;
  getCurrentSession(): Promise<AuthSession | null>;
}

export type OperationType = "upload" | "download" | "delete" | "mkdir" | "rename" | "purge" | "sync";

export type OperationStatus = "pending" | "running" | "success" | "failed" | "partial";

export interface FileOperationResult {
  path: string;
  operation: OperationType;
  status: OperationStatus;
  message?: string;
  error?: string;
  startedAt: string;
  finishedAt?: string;
}

export interface CdnOperationPurgeResult {
  provider: CdnProvider;
  urls: string[];
  status: OperationStatus;
  requestId?: string;
  taskId?: string;
  error?: string;
  startedAt: string;
  finishedAt?: string;
  /** 실제 호출된 CDN Purge API 엔드포인트 — 보안팀 감사 로그 전달 시 참고 */
  requestEndpoint?: string;
  /** 요청 소요 시간 (ms) */
  durationMs?: number;
  /** 이 Purge 요청 과정에서 실제 발생한 HTTP 호출 단계 (상태코드·소요시간 포함) */
  requestSteps?: CdnRequestStep[];
}

export interface OperationLog {
  id: string;
  profileId: string;
  operation: OperationType;
  status: OperationStatus;
  bucket?: string;
  prefix?: string;
  files: FileOperationResult[];
  purgeResults: CdnOperationPurgeResult[];
  metadataFailures?: MetadataFailureLog[];
  logShipping?: LogShippingState;
  startedAt: string;
  finishedAt?: string;
}

export interface MetadataFailureLog {
  path: string;
  headers: Record<string, string>;
  metadata: Record<string, string>;
  error: string;
  retryable: boolean;
}

export interface LogShippingState {
  targetBucket?: string;
  targetPrefix?: string;
  status: "pending" | "success" | "failed";
  attempts: number;
  nextRetryAt?: string;
  error?: string;
}

// ─── Log ─────────────────────────────────────────────────────────────────────

export type LogLevel = "info" | "warn" | "error" | "success" | "debug";

export type LogCategory = "transfer" | "cdn" | "profile" | "system";

export interface LogEntry {
  id: string;
  level: LogLevel;
  message: string;
  timestamp: string;         // ISO 8601
  category?: LogCategory;
  metadata?: Record<string, unknown>;
}

// ─── S3 Operations (Tauri IPC) ───────────────────────────────────────────────

export interface S3ListResponse {
  files: FileItem[];
  nextContinuationToken?: string;
  isTruncated: boolean;
}

export interface S3UploadRequest {
  localPath: string;
  remotePath: string;
  contentType?: string;
  metadata?: Record<string, string>;
  headers?: Record<string, string>;
  retryMetadataFailure?: boolean;
}

export interface S3DownloadRequest {
  remotePath: string;
  localPath: string;
}

export interface SyncPlan {
  toUpload: FileItem[];
  toSkip: FileItem[];        // ETag 일치 → 스킵
  toOverwrite: FileItem[];   // ETag 불일치 → 덮어쓰기 후 CDN Purge
  purgeTargets?: string[];
  compareMode?: "etag" | "etagWithSizeFallback";
  purgePolicy?: PurgePolicy;
}

export interface SyncPreviewEntry {
  localPath?: string;
  remoteKey: string;
  size: number;
  localMd5?: string;
  remoteEtag?: string;
  remoteSize?: number;
}

export interface SyncPreviewResult {
  new: SyncPreviewEntry[];
  modified: SyncPreviewEntry[];
  deleted: SyncPreviewEntry[];
  unchanged: SyncPreviewEntry[];
  purgeTargets: string[];
}

export interface FileEntry {
  localPath: string | null;
  remoteKey: string;
  size: number;
  localMd5: string | null;
  remoteEtag: string | null;
}

export interface SyncResult {
  new: FileEntry[];
  modified: FileEntry[];
  deleted: FileEntry[];
  unchanged: FileEntry[];
}

// ─── App State ───────────────────────────────────────────────────────────────

export type PanelSide = "local" | "remote";

export interface PanelState {
  path: string;
  files: FileItem[];
  selectedPaths: Set<string>;
  isLoading: boolean;
  sortKey: keyof FileItem;
  sortAsc: boolean;
}
