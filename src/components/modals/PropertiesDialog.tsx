import { useState } from "react";
import { createPortal } from "react-dom";
import { runtime } from "../../services/runtime";
import type { FileItem, S3ObjectDetail, S3Profile } from "../../types";
import styles from "./PropertiesDialog.module.css";

interface Props {
  file: FileItem;
  profile: S3Profile;
  onClose: () => void;
}

function fmtSize(bytes: number) {
  if (bytes === 0) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

function fmtDate(iso: string) {
  if (!iso) return "-";
  return new Date(iso).toLocaleString("ko-KR");
}

// S3ObjectDetail의 필드를 사람이 읽을 수 있는 라벨의 헤더 테이블 행으로 변환
const S3_DETAIL_LABELS: [keyof S3ObjectDetail, string][] = [
  ["etag", "ETag"],
  ["contentLength", "Content-Length"],
  ["contentType", "Content-Type"],
  ["contentEncoding", "Content-Encoding"],
  ["contentDisposition", "Content-Disposition"],
  ["contentLanguage", "Content-Language"],
  ["cacheControl", "Cache-Control"],
  ["lastModified", "Last-Modified"],
  ["storageClass", "Storage Class"],
  ["serverSideEncryption", "Server-Side-Encryption"],
  ["sseKmsKeyId", "SSE-KMS-Key-Id"],
  ["versionId", "Version ID"],
  ["replicationStatus", "Replication Status"],
  ["acceptRanges", "Accept-Ranges"],
  ["checksumCrc32", "Checksum (CRC32)"],
  ["checksumSha256", "Checksum (SHA256)"],
];

function HeadersTable({ rows }: { rows: [string, string][] }) {
  if (rows.length === 0) return <div className={styles.empty}>표시할 헤더가 없습니다.</div>;
  return (
    <div className={styles.headerTable}>
      {rows.map(([k, v]) => (
        <div key={k} className={styles.headerRow}>
          <span className={styles.headerKey}>{k}</span>
          <span className={styles.headerVal}>{v}</span>
        </div>
      ))}
    </div>
  );
}

/** S3 객체 속성 다이얼로그 (우클릭 → 속성) — 고객사 요청: CDN이 아닌 S3 속성 중심으로 표시 */
export default function PropertiesDialog({ file, profile, onClose }: Props) {
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  const [s3Detail, setS3Detail] = useState<S3ObjectDetail | null>(null);
  const [s3DetailState, setS3DetailState] = useState<"idle" | "loading" | "error">("idle");
  const [s3DetailError, setS3DetailError] = useState<string | null>(null);

  const copy = async (key: string, value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setCopiedKey(key);
      window.setTimeout(() => setCopiedKey((k) => (k === key ? null : k)), 1200);
    } catch {
      /* 클립보드 권한 없음 — 무시 */
    }
  };

  const fetchS3Detail = async () => {
    if (file.isDirectory) return;
    setS3DetailState("loading");
    setS3DetailError(null);
    try {
      const detail = await runtime.invoke<S3ObjectDetail | null>("get_s3_object_detail", {
        profileId: profile.id,
        key: file.path,
      });
      setS3Detail(detail);
      setS3DetailState("idle");
    } catch (err) {
      console.error("[PropertiesDialog] get_s3_object_detail 실패:", err);
      setS3DetailError(String(err));
      setS3DetailState("error");
    }
  };

  return createPortal(
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <span className={styles.title}>객체 속성</span>
          <button className={styles.closeBtn} onClick={onClose}>✕</button>
        </div>

        <div className={styles.body}>
          {/* 기본 정보 */}
          <section className={styles.section}>
            <div className={styles.sectionTitle}>기본 정보</div>
            <div className={styles.grid}>
              <div className={styles.item}>
                <span className={styles.label}>이름</span>
                <span className={styles.value}>{file.name}</span>
              </div>
              <div className={styles.item}>
                <span className={styles.label}>유형</span>
                <span className={styles.value}>{file.isDirectory ? "폴더" : "파일"}</span>
              </div>
              <div className={styles.item} style={{ gridColumn: "1 / -1" }}>
                <span className={styles.label}>S3 Key (전체 경로)</span>
                <span className={styles.value}>
                  <code className={styles.code}>{file.path}</code>
                  <button className={styles.copyBtn} onClick={() => copy("key", file.path)}>
                    {copiedKey === "key" ? "복사됨" : "복사"}
                  </button>
                </span>
              </div>
              <div className={styles.item}>
                <span className={styles.label}>버킷</span>
                <span className={styles.value}>{profile.bucket}</span>
              </div>
              <div className={styles.item}>
                <span className={styles.label}>리전</span>
                <span className={styles.value}>{profile.region}</span>
              </div>
              {!file.isDirectory && (
                <>
                  <div className={styles.item}>
                    <span className={styles.label}>크기</span>
                    <span className={styles.value}>{fmtSize(file.size)}</span>
                  </div>
                  <div className={styles.item}>
                    <span className={styles.label}>수정일</span>
                    <span className={styles.value}>{fmtDate(file.lastModified)}</span>
                  </div>
                  {file.etag && (
                    <div className={styles.item} style={{ gridColumn: "1 / -1" }}>
                      <span className={styles.label}>ETag</span>
                      <span className={styles.value}><code className={styles.code}>{file.etag}</code></span>
                    </div>
                  )}
                  {file.contentType && (
                    <div className={styles.item}>
                      <span className={styles.label}>Content-Type</span>
                      <span className={styles.value}>{file.contentType}</span>
                    </div>
                  )}
                </>
              )}
            </div>
          </section>

          {/* S3 응답 헤더 (실시간 조회 — 크롬 개발자모드 Network 탭 수준의 상세) */}
          {!file.isDirectory && (
            <section className={styles.section}>
              <div className={styles.sectionTitle} style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <span>S3 응답 헤더 (HeadObject, 실시간)</span>
                <button className={styles.copyBtn} onClick={fetchS3Detail} disabled={s3DetailState === "loading"}>
                  {s3DetailState === "loading" ? "조회 중..." : s3Detail ? "새로고침" : "조회"}
                </button>
              </div>
              {s3DetailState === "error" && <div className={styles.errorBox}>조회 실패: {s3DetailError}</div>}
              {s3Detail && (
                <HeadersTable
                  rows={S3_DETAIL_LABELS
                    .filter(([key]) => s3Detail[key] !== undefined && s3Detail[key] !== null && s3Detail[key] !== "")
                    .map(([key, label]) => [label, String(s3Detail[key])])}
                />
              )}
              {s3Detail && Object.keys(s3Detail.metadata ?? {}).length > 0 && (
                <>
                  <div className={styles.label} style={{ marginTop: 10, marginBottom: 4 }}>사용자 정의 메타데이터 (x-amz-meta-*)</div>
                  <HeadersTable rows={Object.entries(s3Detail.metadata)} />
                </>
              )}
            </section>
          )}
        </div>

        <div className={styles.footer}>
          <button className={styles.closeFullBtn} onClick={onClose}>닫기</button>
        </div>
      </div>
    </div>,
    document.body
  );
}
