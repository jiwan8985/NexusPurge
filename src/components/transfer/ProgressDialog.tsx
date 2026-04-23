import { useAppStore } from "../../store/appStore";
import type { TransferItem } from "../../types";
import styles from "./ProgressDialog.module.css";

function TransferRow({ item }: { item: TransferItem }) {
  const statusLabel: Record<TransferItem["status"], string> = {
    pending: "대기",
    uploading: "업로드 중",
    downloading: "다운로드 중",
    hashing: "MD5 계산",
    skipped: "스킵 (동일)",
    overwriting: "덮어쓰기",
    complete: "완료",
    error: "오류",
  };

  const statusClass: Record<TransferItem["status"], string> = {
    pending: styles.statusPending,
    uploading: styles.statusActive,
    downloading: styles.statusActive,
    hashing: styles.statusActive,
    skipped: styles.statusSkipped,
    overwriting: styles.statusOverwrite,
    complete: styles.statusComplete,
    error: styles.statusError,
  };

  return (
    <div className={styles.transferRow}>
      <div className={styles.transferInfo}>
        <span className={styles.fileName}>{item.fileName}</span>
        <span className={`${styles.status} ${statusClass[item.status]}`}>
          {statusLabel[item.status]}
          {item.cdnPurged && " + CDN Purge ✓"}
          {item.cdnPurgeError && " + CDN Purge ✗"}
        </span>
      </div>

      {(item.status === "uploading" || item.status === "downloading") && (
        <div className={styles.progressBar}>
          <div
            className={styles.progressFill}
            style={{ width: `${item.progress}%` }}
          />
        </div>
      )}

      {item.error && (
        <div className={styles.errorMsg}>{item.error}</div>
      )}
    </div>
  );
}

export default function ProgressDialog() {
  const { transfers, isTransferring, setShowProgressDialog, clearCompletedTransfers } =
    useAppStore((s) => ({
      transfers: s.transfers,
      isTransferring: s.isTransferring,
      setShowProgressDialog: s.setShowProgressDialog,
      clearCompletedTransfers: s.clearCompletedTransfers,
    }));

  const completed = transfers.filter((t) => t.status === "complete" || t.status === "skipped").length;
  const total = transfers.length;
  const overallProgress = total > 0 ? (completed / total) * 100 : 0;

  return (
    <div className={styles.overlay}>
      <div className={styles.dialog}>
        <div className={styles.header}>
          <span className={styles.title}>
            {isTransferring ? "파일 전송 중..." : "전송 완료"}
          </span>
          <span className={styles.counter}>{completed} / {total}</span>
        </div>

        {/* 전체 진행률 */}
        <div className={styles.overallProgress}>
          <div
            className={styles.overallFill}
            style={{ width: `${overallProgress}%` }}
          />
        </div>

        {/* 개별 전송 목록 */}
        <div className={styles.transferList}>
          {transfers.map((item) => (
            <TransferRow key={item.id} item={item} />
          ))}
        </div>

        {/* 액션 버튼 */}
        <div className={styles.actions}>
          <button
            className={styles.clearBtn}
            onClick={clearCompletedTransfers}
          >
            완료 항목 지우기
          </button>
          <button
            className={styles.closeBtn}
            disabled={isTransferring}
            onClick={() => setShowProgressDialog(false)}
          >
            닫기
          </button>
        </div>
      </div>
    </div>
  );
}
