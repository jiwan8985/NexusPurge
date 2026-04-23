import { useAppStore } from "../../store/appStore";
import { useTransfer } from "../../hooks/useTransfer";
import styles from "./TransferButtons.module.css";

export default function TransferButtons() {
  const { isConnected, isTransferring, local, remote } = useAppStore((s) => ({
    isConnected: s.isConnected,
    isTransferring: s.isTransferring,
    local: s.local,
    remote: s.remote,
  }));
  const { startUpload, startDownload } = useTransfer();

  const canUpload = isConnected && !isTransferring && local.selectedPaths.size > 0;
  const canDownload = isConnected && !isTransferring && remote.selectedPaths.size > 0;

  return (
    <div className={styles.container}>
      {/* 업로드: 로컬 → S3 */}
      <button
        className={`${styles.transferBtn} ${styles.upload}`}
        onClick={startUpload}
        disabled={!canUpload}
        title={`선택한 ${local.selectedPaths.size}개 파일을 S3로 업로드`}
      >
        <span className={styles.arrow}>→</span>
        <span className={styles.label}>업로드</span>
      </button>

      {/* 다운로드: S3 → 로컬 */}
      <button
        className={`${styles.transferBtn} ${styles.download}`}
        onClick={startDownload}
        disabled={!canDownload}
        title={`선택한 ${remote.selectedPaths.size}개 파일을 로컬로 다운로드`}
      >
        <span className={styles.arrow}>←</span>
        <span className={styles.label}>다운로드</span>
      </button>
    </div>
  );
}
