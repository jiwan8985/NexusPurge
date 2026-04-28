import { useState } from "react";
import type { SyncResult, FileEntry } from "../../types";
import styles from "./SyncPreviewDialog.module.css";

function fmtSize(b: number) {
  if (b === 0) return "-";
  if (b < 1024) return `${b} B`;
  if (b < 1048576) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1073741824) return `${(b / 1048576).toFixed(1)} MB`;
  return `${(b / 1073741824).toFixed(2)} GB`;
}

function baseName(entry: FileEntry): string {
  const key = entry.localPath ?? entry.remoteKey;
  return key.replace(/\\/g, "/").split("/").pop() ?? key;
}

type Tab = "new" | "modified" | "deleted" | "unchanged";

const TAB_LABEL: Record<Tab, string> = {
  new:       "새 파일",
  modified:  "수정됨",
  deleted:   "삭제 예정",
  unchanged: "변경 없음",
};

interface Props {
  result: SyncResult;
  localPath: string;
  remotePath: string;
  onClose: () => void;
}

export default function SyncPreviewDialog({ result, localPath, remotePath, onClose }: Props) {
  const [tab, setTab] = useState<Tab>("new");

  const counts: Record<Tab, number> = {
    new:       result.new.length,
    modified:  result.modified.length,
    deleted:   result.deleted.length,
    unchanged: result.unchanged.length,
  };

  const entries: FileEntry[] = result[tab];

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <div>
            <div className={styles.title}>동기화 미리보기</div>
            <div className={styles.subtitle}>
              {localPath} → S3: {remotePath || "(루트)"}
            </div>
          </div>
          <button className={styles.closeBtn} onClick={onClose} aria-label="닫기">✕</button>
        </div>

        <div className={styles.tabs}>
          {(["new", "modified", "deleted", "unchanged"] as Tab[]).map((t) => (
            <button
              key={t}
              className={`${styles.tab} ${tab === t ? styles.tabActive : ""} ${t === "new" ? styles.tabNew : t === "modified" ? styles.tabModified : t === "deleted" ? styles.tabDeleted : styles.tabUnchanged}`}
              onClick={() => setTab(t)}
            >
              {TAB_LABEL[t]}
              <span className={styles.tabCount}>{counts[t]}</span>
            </button>
          ))}
        </div>

        <div className={styles.list}>
          {entries.length === 0 ? (
            <div className={styles.empty}>항목 없음</div>
          ) : (
            entries.map((entry, i) => (
              <div key={i} className={styles.row}>
                <span className={styles.fileName}>{baseName(entry)}</span>
                <span className={styles.fileSize}>{fmtSize(entry.size)}</span>
                <span className={styles.filePath}>{entry.remoteKey}</span>
              </div>
            ))
          )}
        </div>

        <div className={styles.footer}>
          <span className={styles.summary}>
            신규 {counts.new} · 수정 {counts.modified} · 삭제 {counts.deleted} · 변경 없음 {counts.unchanged}
          </span>
          <button className={styles.closeButton} onClick={onClose}>닫기</button>
        </div>
      </div>
    </div>
  );
}
