import { useMemo } from "react";
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

function fmtSpeed(bytesPerSec: number) {
  if (bytesPerSec <= 0) return "—";
  return `${fmtSize(bytesPerSec)}/s`;
}

function fmtEta(remainingBytes: number, speed: number) {
  if (speed <= 0 || remainingBytes <= 0) return null;
  const secs = Math.round(remainingBytes / speed);
  if (secs < 60) return `${secs}초`;
  const min = Math.floor(secs / 60);
  const sec = secs % 60;
  return `${min}분 ${sec}초`;
}

// ── TransferRow ───────────────────────────────────────────────────────────────

const STATUS_LABEL: Record<TransferItem["status"], string> = {
  pending:     "대기",
  uploading:   "업로드 중",
  downloading: "다운로드 중",
  hashing:     "MD5 계산",
  skipped:     "스킵",
  overwriting: "덮어쓰기",
  complete:    "완료",
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
  error:       styles.stError,
};

function TransferRow({ item }: { item: TransferItem }) {
  const isActive = item.status === "uploading" || item.status === "downloading";
  const eta = isActive && item.speed
    ? fmtEta(item.size - item.transferredBytes, item.speed)
    : null;

  return (
    <div className={styles.tRow}>
      <div className={styles.tTop}>
        <span className={styles.tName}>{item.fileName}</span>
        <span className={`${styles.tStatus} ${STATUS_CLS[item.status]}`}>
          {STATUS_LABEL[item.status]}
          {item.cdnPurged && " + CDN ✓"}
          {item.cdnPurgeError && " + CDN ✗"}
        </span>
      </div>

      {isActive && (
        <div className={styles.tMeta}>
          <div className={styles.tProgress}>
            <div className={styles.tProgressFill} style={{ width: `${item.progress}%` }} />
          </div>
          <div className={styles.tStats}>
            <span>{fmtSize(item.transferredBytes)} / {fmtSize(item.size)}</span>
            {item.speed !== undefined && item.speed > 0 && (
              <span>{fmtSpeed(item.speed)}</span>
            )}
            {eta && <span>남은 {eta}</span>}
          </div>
        </div>
      )}

      {item.error && (
        <div className={styles.tError}>{item.error}</div>
      )}
    </div>
  );
}

// ── ProgressDialog ────────────────────────────────────────────────────────────

export default function ProgressDialog() {
  const { transfers, isTransferring, setShowProgressDialog, clearCompletedTransfers } =
    useAppStore((s) => ({
      transfers: s.transfers,
      isTransferring: s.isTransferring,
      setShowProgressDialog: s.setShowProgressDialog,
      clearCompletedTransfers: s.clearCompletedTransfers,
    }));

  const summary = useMemo(() => {
    const total     = transfers.length;
    const done      = transfers.filter((t) => t.status === "complete" || t.status === "skipped").length;
    const errored   = transfers.filter((t) => t.status === "error").length;
    const active    = transfers.filter((t) => t.status === "uploading" || t.status === "downloading");
    const totalBytes= transfers.reduce((s, t) => s + t.size, 0);
    const txBytes   = transfers.reduce((s, t) => s + t.transferredBytes, 0);
    const avgSpeed  = active.reduce((s, t) => s + (t.speed ?? 0), 0);
    return { total, done, errored, active, totalBytes, txBytes, avgSpeed };
  }, [transfers]);

  const overallPct = summary.total > 0
    ? Math.round((summary.done / summary.total) * 100)
    : 0;

  const eta = fmtEta(summary.totalBytes - summary.txBytes, summary.avgSpeed);

  return (
    <div className={styles.overlay}>
      <div className={styles.dialog}>
        {/* 헤더 */}
        <div className={styles.header}>
          <div>
            <div className={styles.title}>
              {isTransferring ? "파일 전송 중…" : "전송 완료"}
            </div>
            <div className={styles.subtitle}>
              {summary.done} / {summary.total}개
              {summary.errored > 0 && ` · 오류 ${summary.errored}개`}
              {isTransferring && summary.avgSpeed > 0 && ` · ${fmtSpeed(summary.avgSpeed)}`}
              {isTransferring && eta && ` · 남은 ${eta}`}
            </div>
          </div>
          <div className={styles.pct}>{overallPct}%</div>
        </div>

        {/* 전체 진행 바 */}
        <div className={styles.overallBar}>
          <div className={styles.overallFill} style={{ width: `${overallPct}%` }} />
        </div>

        {/* 전송 목록 */}
        <div className={styles.list}>
          {transfers.length === 0 ? (
            <div className={styles.empty}>전송 항목이 없습니다</div>
          ) : (
            transfers.map((t) => <TransferRow key={t.id} item={t} />)
          )}
        </div>

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
                title="창을 닫아도 Rust 전송은 백그라운드에서 계속됩니다"
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
