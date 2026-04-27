// ─── S3 Profile ──────────────────────────────────────────────────────────────

export interface S3Profile {
  id: string;
  name: string;
  region: string;
  bucket: string;
  accessKeyId: string;
  secretAccessKey: string;
  endpoint?: string;              // S3-compatible 서비스용 커스텀 엔드포인트
  cdnProvider?: CdnProvider;
  cdnDistributionId?: string;     // CloudFront distribution ID
  cdnDomain?: string;             // CDN 도메인 (Purge URL 구성용)
  // H-6: Akamai EdgeGrid 자격증명
  akamaiClientToken?: string;     // Akamai EdgeGrid client token
  akamaiClientSecret?: string;    // 저장 시 keyring에 보관, 로드 시 빈 값
  akamaiAccessToken?: string;     // Akamai EdgeGrid access token
  akamaiHost?: string;            // EdgeGrid API 호스트 (e.g. akab-xxxx.luna.akamaiapis.net)
  createdAt: string;
  updatedAt: string;
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
  startedAt?: string;
  completedAt?: string;
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

export type CdnProvider = "cloudfront" | "akamai";

export interface CdnPurgeRequest {
  provider: CdnProvider;
  distributionId: string;
  paths: string[];
}

export interface CdnPurgeResult {
  success: boolean;
  provider: CdnProvider;
  invalidationId?: string;
  paths: string[];
  purgedAt?: string;
  error?: string;
}

// ─── Log ─────────────────────────────────────────────────────────────────────

export type LogLevel = "info" | "warn" | "error" | "success" | "debug";

export interface LogEntry {
  id: string;
  level: LogLevel;
  message: string;
  timestamp: string;         // ISO 8601
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
}

export interface S3DownloadRequest {
  remotePath: string;
  localPath: string;
}

export interface SyncPlan {
  toUpload: FileItem[];
  toSkip: FileItem[];        // ETag 일치 → 스킵
  toOverwrite: FileItem[];   // ETag 불일치 → 덮어쓰기 후 CDN Purge
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
