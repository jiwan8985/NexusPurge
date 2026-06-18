import { useState } from "react";
import type { SyncPreviewResult, SyncPreviewEntry } from "../../types";
import styles from "./SyncPreviewDialog.module.css";

type Tab = "new" | "modified" | "purge" | "unchanged";

function fmtSize(b: number) {
  if (b === 0) return "-";
  if (b < 1024) return `${b} B`;
  if (b < 1048576) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1073741824) return `${(b / 1048576).toFixed(1)} MB`;
  return `${(b / 1073741824).toFixed(2)} GB`;
}

function extIcon(key: string) {
  const ext = key.split(".").pop()?.toLowerCase() ?? "";
  if (["png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "avif"].includes(ext)) return "IMG";
  if (["mp4", "mov", "avi", "webm", "mkv"].includes(ext)) return "VID";
  if (["js", "ts", "jsx", "tsx", "html", "css", "json", "yaml", "yml", "xml", "rs", "go", "py"].includes(ext)) return "DEV";
  if (["zip", "tar", "gz", "rar", "7z"].includes(ext)) return "ZIP";
  return "FILE";
}

function baseName(entry: SyncPreviewEntry) {
  const key = entry.localPath ?? entry.remoteKey;
  return key.replace(/\\/g, "/").split("/").pop() ?? key;
}

function FileRow({ entry, overwrite }: { entry: SyncPreviewEntry; overwrite?: boolean }) {
  return (
    <div className={styles.fileRow}>
      <span className={styles.fileExt}>{extIcon(entry.remoteKey)}</span>
      <span className={styles.fileName}>
        {baseName(entry)}
        {overwrite && <span className={styles.overwriteBadge}>⚠ 기존 파일 교체</span>}
      </span>
      <span className={styles.fileSize}>{fmtSize(entry.size)}</span>
      <span className={styles.fileKey} title={entry.remoteKey}>{entry.remoteKey}</span>
    </div>
  );
}

interface Props {
  result: SyncPreviewResult;
  localPath: string;
  remotePath: string;
  onClose: () => void;
}

export default function SyncPreviewDialog({ result, localPath, remotePath, onClose }: Props) {
  const [tab, setTab] = useState<Tab>("new");

  const { new: newFiles, modified, unchanged, purgeTargets } = result;
  const uploadBytes = [...newFiles, ...modified].reduce((s, e) => s + e.size, 0);

  const TAB_META: { id: Tab; label: string; count: number; activeCls: string }[] = [
    { id: "new",       label: "업로드 신규",  count: newFiles.length,     activeCls: styles.tabActiveNew      },
    { id: "modified",  label: "덮어쓰기",     count: modified.length,     activeCls: styles.tabActiveModified },
    { id: "purge",     label: "CDN Purge",    count: purgeTargets.length, activeCls: styles.tabActivePurge    },
    { id: "unchanged", label: "스킵",         count: unchanged.length,    activeCls: styles.tabActiveSkip     },
  ];

  const isEmpty = newFiles.length === 0 && modified.length === 0 &&
    purgeTargets.length === 0 && unchanged.length === 0;

  return (
    <div className={styles.overlay} onClick={(e) => e.target === e.currentTarget && onClose()}>
      <div className={styles.dialog}>
        {/* 헤더 */}
        <div className={styles.header}>
          <div className={styles.headerTop}>
            <div>
              <div className={styles.title}>업로드 계획</div>
              <div className={styles.subtitle}>
                로컬: {localPath} → S3: {remotePath || "(루트)"}
              </div>
            </div>
            <button className={styles.closeBtn} onClick={onClose}>✕</button>
          </div>
          <div className={styles.summaryRow}>
            {newFiles.length > 0 && (
              <span className={`${styles.pill} ${styles.pillNew}`}>
                신규 {newFiles.length}개 · {fmtSize(uploadBytes)}
              </span>
            )}
            {modified.length > 0 && (
              <span className={`${styles.pill} ${styles.pillModified}`}>
                덮어쓰기 {modified.length}개
              </span>
            )}
            {purgeTargets.length > 0 && (
              <span className={`${styles.pill} ${styles.pillPurge}`}>
                CDN Purge {purgeTargets.length}개
              </span>
            )}
            {unchanged.length > 0 && (
              <span className={`${styles.pill} ${styles.pillSkip}`}>
                스킵 {unchanged.length}개
              </span>
            )}
          </div>
        </div>

        {/* 탭 */}
        <div className={styles.tabs}>
          {TAB_META.map(({ id, label, count, activeCls }) => (
            <button
              key={id}
              className={`${styles.tab} ${tab === id ? activeCls : ""}`}
              onClick={() => setTab(id)}
            >
              {label}
              <span className={styles.tabCount}>{count}</span>
            </button>
          ))}
        </div>

        {/* 목록 */}
        <div className={styles.list}>
          {isEmpty ? (
            <div className={styles.listEmpty}>
              <span>업로드할 파일이 없습니다</span>
              <span style={{ fontSize: 11 }}>모두 최신 상태입니다.</span>
            </div>
          ) : tab === "new" ? (
            newFiles.length === 0
              ? <div className={styles.listEmpty}>신규 파일 없음</div>
              : newFiles.map((e, i) => <FileRow key={i} entry={e} />)
          ) : tab === "modified" ? (
            modified.length === 0
              ? <div className={styles.listEmpty}>덮어쓰기 파일 없음</div>
              : modified.map((e, i) => <FileRow key={i} entry={e} overwrite />)
          ) : tab === "purge" ? (
            purgeTargets.length === 0
              ? <div className={styles.listEmpty}>CDN Purge 대상 없음</div>
              : (
                <>
                  <div className={styles.purgeNote}>
                    이 파일들은 업로드 완료 후 CDN Purge됩니다.
                  </div>
                  {purgeTargets.map((path, i) => (
                    <div key={i} className={styles.purgeRow}>
                      <span className={styles.purgeIdx}>{i + 1}</span>
                      <span className={styles.purgePath} title={path}>{path}</span>
                    </div>
                  ))}
                </>
              )
          ) : (
            unchanged.length === 0
              ? <div className={styles.listEmpty}>스킵 파일 없음</div>
              : unchanged.map((e, i) => <FileRow key={i} entry={e} />)
          )}
        </div>

        {/* 푸터 */}
        <div className={styles.footer}>
          <button className={styles.btn} onClick={onClose}>닫기</button>
        </div>
      </div>
    </div>
  );
}
