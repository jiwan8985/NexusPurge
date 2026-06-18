import { useState } from "react";
import { useAppStore } from "../../store/appStore";
import type { SyncPreviewEntry } from "../../types";
import styles from "./SyncPreviewDialog.module.css";

type Tab = "new" | "modified" | "purge" | "unchanged";

function fmtSize(b: number) {
  if (b === 0) return "-";
  if (b < 1024) return `${b} B`;
  if (b < 1048576) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1073741824) return `${(b / 1048576).toFixed(1)} MB`;
  return `${(b / 1073741824).toFixed(2)} GB`;
}

function totalSize(entries: SyncPreviewEntry[]) {
  return entries.reduce((s, e) => s + e.size, 0);
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

function hashShort(hash: string | undefined) {
  return hash ? hash.replace(/"/g, "").slice(0, 8) : null;
}

function FileRow({ entry, showMd5 }: { entry: SyncPreviewEntry; showMd5?: boolean }) {
  return (
    <div className={styles.fileRow}>
      <span className={styles.fileExt}>{extIcon(entry.remoteKey)}</span>
      <span className={styles.fileName}>{baseName(entry)}</span>
      <span className={styles.fileSize}>{fmtSize(entry.size)}</span>
      <span className={styles.fileKey} title={entry.remoteKey}>{entry.remoteKey}</span>
      {showMd5 && (entry.remoteEtag || entry.localMd5) && (
        <div className={styles.md5Row}>
          <span className={styles.md5Label}>ETag</span>
          <span className={styles.md5Old}>{hashShort(entry.remoteEtag) ?? "—"}</span>
          <span className={styles.md5Arrow}>→</span>
          <span className={styles.md5New}>{hashShort(entry.localMd5) ?? "—"}</span>
        </div>
      )}
    </div>
  );
}

export default function SyncPreviewDialog() {
  const { syncPreview, setShowSyncPreview } = useAppStore((s) => ({
    syncPreview:       s.syncPreview,
    setShowSyncPreview: s.setShowSyncPreview,
  }));

  const [tab, setTab] = useState<Tab>("new");

  if (!syncPreview) return null;

  const { new: newFiles, modified, unchanged, purgeTargets } = syncPreview;
  const uploadTotal = newFiles.length + modified.length;
  const uploadBytes = totalSize(newFiles) + totalSize(modified);

  const TAB_META: { id: Tab; label: string; count: number; activeCls: string }[] = [
    { id: "new",       label: "업로드 신규",   count: newFiles.length,     activeCls: styles.tabActiveNew      },
    { id: "modified",  label: "덮어쓰기",      count: modified.length,     activeCls: styles.tabActiveModified },
    { id: "purge",     label: "CDN Purge",     count: purgeTargets.length, activeCls: styles.tabActivePurge    },
    { id: "unchanged", label: "스킵",          count: unchanged.length,    activeCls: styles.tabActiveSkip     },
  ];

  const close = () => setShowSyncPreview(false);

  return (
    <div className={styles.overlay} onClick={(e) => e.target === e.currentTarget && close()}>
      <div className={styles.dialog}>
        {/* 헤더 */}
        <div className={styles.header}>
          <div className={styles.headerTop}>
            <div className={styles.title}>Dry-run 미리보기</div>
            <button className={styles.closeBtn} onClick={close}>✕</button>
          </div>
          <div className={styles.summaryRow}>
            <span className={styles.pill + " " + styles.pillTotal}>
              전체 {newFiles.length + modified.length + unchanged.length + purgeTargets.length}
            </span>
            {uploadTotal > 0 && (
              <span className={styles.pill + " " + styles.pillNew}>
                업로드 {uploadTotal}개 · {fmtSize(uploadBytes)}
              </span>
            )}
            {modified.length > 0 && (
              <span className={styles.pill + " " + styles.pillModified}>
                덮어쓰기 {modified.length}개
              </span>
            )}
            {purgeTargets.length > 0 && (
              <span className={styles.pill + " " + styles.pillPurge}>
                CDN Purge {purgeTargets.length}개
              </span>
            )}
            {unchanged.length > 0 && (
              <span className={styles.pill + " " + styles.pillSkip}>
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
          {tab === "new" && (
            newFiles.length === 0
              ? <div className={styles.listEmpty}>신규 파일 없음</div>
              : newFiles.map((e, i) => <FileRow key={i} entry={e} />)
          )}
          {tab === "modified" && (
            modified.length === 0
              ? <div className={styles.listEmpty}>덮어쓰기 파일 없음</div>
              : modified.map((e, i) => <FileRow key={i} entry={e} showMd5 />)
          )}
          {tab === "purge" && (
            purgeTargets.length === 0
              ? <div className={styles.listEmpty}>CDN Purge 대상 없음</div>
              : (
                <>
                  <div className={styles.purgeNote}>
                    ℹ 이 경로들이 CDN에서 무효화됩니다. 실제 변경은 업로드 시 실행됩니다.
                  </div>
                  {purgeTargets.map((path, i) => (
                    <div key={i} className={styles.purgeRow}>
                      <span className={styles.purgeIdx}>{i + 1}</span>
                      <span className={styles.purgePath} title={path}>{path}</span>
                    </div>
                  ))}
                </>
              )
          )}
          {tab === "unchanged" && (
            unchanged.length === 0
              ? <div className={styles.listEmpty}>스킵 파일 없음</div>
              : unchanged.map((e, i) => <FileRow key={i} entry={e} />)
          )}
        </div>

        {/* 푸터 */}
        <div className={styles.footer}>
          <span className={styles.footerNote}>
            실제 업로드/삭제는 실행되지 않았습니다 — Dry-run 결과입니다.
          </span>
          <div className={styles.footerActions}>
            <button
              className={styles.btn}
              disabled={purgeTargets.length === 0}
              onClick={() => navigator.clipboard.writeText(purgeTargets.join("\n"))}
            >
              Purge 목록 복사
            </button>
            <button className={styles.btn} onClick={close}>닫기</button>
          </div>
        </div>
      </div>
    </div>
  );
}
