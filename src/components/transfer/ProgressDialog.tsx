import { useMemo, useState } from "react";
import { runtime } from "../../services/runtime";
import { useAppStore } from "../../store/appStore";
import type { TransferItem } from "../../types";
import styles from "./ProgressDialog.module.css";

// ── Helpers ───────────────────────────────────────────────────────────────────

function fmtSize(b: number) {
  if (b < 1024) return `${b} B`;
  if (b < 1048576) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1073741824) return `${(b / 1048576).toFixed(1)} MB`;
  return `${(b / 1073741824).toFixed(2)} GB`;
}

function fmtSpeed(bps: number) {
  if (bps <= 0) return "—";
  return `${fmtSize(bps)}/s`;
}

function fmtEta(remaining: number, speed: number) {
  if (speed <= 0 || remaining <= 0) return null;
  const s = Math.round(remaining / speed);
  if (s < 60) return `${s}초`;
  return `${Math.floor(s / 60)}분 ${s % 60}초`;
}

function extIcon(fileName: string) {
  const ext = fileName.split(".").pop()?.toLowerCase() ?? "";
  if (["png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "avif"].includes(ext)) return "IMG";
  if (["mp4", "mov", "avi", "webm", "mkv"].includes(ext)) return "VID";
  if (["js", "ts", "jsx", "tsx", "html", "css", "json", "yaml", "yml", "xml", "rs", "go", "py"].includes(ext)) return "DEV";
  if (["zip", "tar", "gz", "rar", "7z"].includes(ext)) return "ZIP";
  return "FILE";
}

// ── Status Maps ───────────────────────────────────────────────────────────────

const STATUS_LABEL: Record<TransferItem["status"], string> = {
  pending:     "대기",
  uploading:   "업로드 중",
  downloading: "다운로드 중",
  hashing:     "MD5 계산",
  skipped:     "스킵",
  overwriting: "덮어쓰기",
  complete:    "완료",
  canceled:    "취소됨",
  error:       "오류",
};

const STATUS_CLS: Record<TransferItem["status"], string> = {
  pending:     styles.stPending,
  uploading:   styles.stActive,
  downloading: styles.stActive,
  hashing:     styles.stActive,
  skipped:     styles.stSkipped,
  overwriting: styles.stOverwrite,
  complete:    styles.stComplete,
  canceled:    styles.stSkipped,
  error:       styles.stError,
};

// ── CDN Badge ─────────────────────────────────────────────────────────────────

function CdnBadges({ item }: { item: TransferItem }) {
  const purgeCls = (() => {
    if (item.cdnPurgeStatus === "pending")    return styles.cdnPending;
    if (item.cdnPurgeStatus === "inProgress") return styles.cdnProgress;
    if (item.cdnPurgeStatus === "complete")   return styles.cdnOk;
    if (item.cdnPurgeStatus === "error")      return styles.cdnError;
    return styles.cdnPending;
  })();

  const purgeLabel = (() => {
    if (item.cdnPurgeStatus === "pending")    return "CDN Purge 대기";
    if (item.cdnPurgeStatus === "inProgress") return "CDN Purge 진행중";
    if (item.cdnPurgeStatus === "complete")   return "CDN Purge 완료";
    if (item.cdnPurgeStatus === "error")      return "CDN Purge 실패";
    return null;
  })();

  const hasCdn = purgeLabel || item.cdnVerified !== undefined || item.cdnUrl;
  if (!hasCdn) return null;

  return (
    <div className={styles.tCdnMeta}>
      {purgeLabel && (
        <span className={`${styles.cdnBadge} ${purgeCls}`} title={item.cdnPurgeError}>
          {purgeLabel}
          {item.cdnInvalidationId && (
            <span style={{ marginLeft: 4, opacity: 0.7, fontSize: 9 }}>
              {item.cdnInvalidationId.slice(0, 12)}…
            </span>
          )}
        </span>
      )}
      {item.cdnVerified !== undefined && (
        <span className={`${styles.cdnBadge} ${item.cdnVerified ? styles.cdnOk : styles.cdnWarn}`}>
          CDN {item.cdnVerified ? "반영 확인" : "반영 미확인"}
          {item.cdnStatusCode ? ` (${item.cdnStatusCode})` : ""}
        </span>
      )}
      {item.cdnUrl && (
        <>
          <button
            className={styles.urlBtn}
            type="button"
            onClick={() => navigator.clipboard.writeText(item.cdnUrl ?? "")}
            title={item.cdnUrl}
          >
            CDN URL 복사
          </button>
          <button
            className={styles.urlBtn}
            type="button"
            onClick={() => window.open(item.cdnUrl, "_blank", "noopener,noreferrer")}
          >
            열기
          </button>
        </>
      )}
    </div>
  );
}

// ── TransferRow ───────────────────────────────────────────────────────────────

function TransferRow({ item }: { item: TransferItem }) {
  const isActive = item.status === "uploading" || item.status === "downloading" || item.status === "hashing";
  const eta = isActive && item.speed
    ? fmtEta(item.size - item.transferredBytes, item.speed)
    : null;

  const canCancel =
    item.status === "pending" || item.status === "uploading" ||
    item.status === "downloading" || item.status === "hashing" || item.status === "overwriting";

  return (
    <div className={styles.tRow}>
      <div className={styles.tTop}>
        <span className={styles.tIcon}>{extIcon(item.fileName)}</span>
        <span className={styles.tName}>{item.fileName}</span>
        <span className={`${styles.tStatus} ${STATUS_CLS[item.status]}`}>
          {STATUS_LABEL[item.status]}
        </span>
        {canCancel && (
          <button
            className={styles.cancelBtn}
            type="button"
            onClick={() => void runtime.invoke("cancel_transfer", { id: item.id })}
          >
            취소
          </button>
        )}
      </div>

      {isActive && (
        <div className={styles.tMeta}>
          <div className={styles.tProgress}>
            <div className={styles.tProgressFill} style={{ width: `${item.progress}%` }} />
          </div>
          <div className={styles.tStats}>
            <span>{fmtSize(item.transferredBytes)} / {fmtSize(item.size)}</span>
            {item.speed && item.speed > 0 && <span>{fmtSpeed(item.speed)}</span>}
            {eta && <span>남은 {eta}</span>}
          </div>
        </div>
      )}

      <CdnBadges item={item} />

      {item.error && <div className={styles.tError}>{item.error}</div>}
    </div>
  );
}

// ── ProgressDialog ────────────────────────────────────────────────────────────

export default function ProgressDialog() {
  const { transfers, isTransferring, setShowProgressDialog, clearCompletedTransfers } =
    useAppStore((s) => ({
      transfers:               s.transfers,
      isTransferring:          s.isTransferring,
      setShowProgressDialog:   s.setShowProgressDialog,
      clearCompletedTransfers: s.clearCompletedTransfers,
    }));

  const [minimized, setMinimized] = useState(false);

  const summary = useMemo(() => {
    const total      = transfers.length;
    const done       = transfers.filter((t) => t.status === "complete" || t.status === "skipped").length;
    const errored    = transfers.filter((t) => t.status === "error").length;
    const active     = transfers.filter((t) => t.status === "uploading" || t.status === "downloading");
    const totalBytes = transfers.reduce((s, t) => s + t.size, 0);
    const txBytes    = transfers.reduce((s, t) => s + t.transferredBytes, 0);
    const avgSpeed   = active.reduce((s, t) => s + (t.speed ?? 0), 0);
    return { total, done, errored, active, totalBytes, txBytes, avgSpeed };
  }, [transfers]);

  const overallPct = summary.total > 0 ? Math.round((summary.done / summary.total) * 100) : 0;
  const eta        = fmtEta(summary.totalBytes - summary.txBytes, summary.avgSpeed);

  const cdnUrls = transfers
    .filter((t) => t.direction === "upload" && t.cdnUrl && t.status === "complete")
    .map((t) => t.cdnUrl as string);

  const subtitleParts: string[] = [
    `${summary.done}/${summary.total}개`,
    ...(summary.errored > 0 ? [`오류 ${summary.errored}개`] : []),
    ...(isTransferring && summary.avgSpeed > 0 ? [fmtSpeed(summary.avgSpeed)] : []),
    ...(isTransferring && eta ? [`남은 ${eta}`] : []),
  ];

  if (minimized) {
    return (
      <div className={styles.minimized} onClick={() => setMinimized(false)}>
        <span className={styles.minimizedIcon}>↑</span>
        <span className={styles.minimizedText}>
          {overallPct}% · {subtitleParts.join(" · ")}
        </span>
      </div>
    );
  }

  return (
    <div className={styles.container}>
      <div className={styles.dialog}>
        {/* 헤더 */}
        <div className={styles.header}>
          <div className={styles.headerTop}>
            <div className={styles.titleGroup}>
              <span className={styles.title}>
                {isTransferring ? "파일 전송 중…" : "전송 완료"}
              </span>
              <span className={styles.subtitle}>{subtitleParts.join(" · ")}</span>
            </div>
            <div className={styles.headerActions}>
              <span className={styles.pct}>{overallPct}%</span>
              <button className={styles.iconBtn} onClick={() => setMinimized(true)} title="최소화">
                ━
              </button>
              <button
                className={styles.iconBtn}
                onClick={() => setShowProgressDialog(false)}
                title={isTransferring ? "창 숨기기 (전송은 계속)" : "닫기"}
              >
                ✕
              </button>
            </div>
          </div>
          <div className={styles.overallBar}>
            <div className={styles.overallFill} style={{ width: `${overallPct}%` }} />
          </div>
        </div>

        {/* 전송 목록 */}
        <div className={styles.list}>
          {transfers.length === 0 ? (
            <div className={styles.empty}>전송 항목이 없습니다</div>
          ) : (
            transfers.map((t) => <TransferRow key={t.id} item={t} />)
          )}
        </div>

        {cdnUrls.length > 0 && (
          <div className={styles.cdnUrlList}>
            <div className={styles.cdnUrlHeader}>
              <span>업로드 CDN URL</span>
              <button
                type="button"
                className={styles.urlBtn}
                onClick={() => navigator.clipboard.writeText(cdnUrls.join("\n"))}
              >
                전체 복사
              </button>
            </div>
            {cdnUrls.slice(0, 5).map((url) => (
              <div className={styles.cdnUrlRow} key={url} title={url}>{url}</div>
            ))}
            {cdnUrls.length > 5 && (
              <div className={styles.cdnUrlMore}>외 {cdnUrls.length - 5}개</div>
            )}
          </div>
        )}

        {/* 액션 버튼 */}
        <div className={styles.actions}>
          <button className={styles.btnSecondary} onClick={clearCompletedTransfers}>
            완료 항목 지우기
          </button>
          <div className={styles.actionRight}>
            {isTransferring && (
              <button
                className={styles.btnDanger}
                onClick={() => setShowProgressDialog(false)}
                title="창을 닫아도 전송은 백그라운드에서 계속됩니다"
              >
                창 숨기기
              </button>
            )}
            <button
              className={styles.btnPrimary}
              disabled={isTransferring}
              onClick={() => setShowProgressDialog(false)}
            >
              {isTransferring ? "전송 중…" : "닫기"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
