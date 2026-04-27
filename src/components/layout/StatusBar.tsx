import { useAppStore } from "../../store/appStore";
import styles from "./StatusBar.module.css";

export default function StatusBar() {
  const { isConnected, activeProfile, transfers, isTransferring, toggleLogPanel } =
    useAppStore((s) => ({
      isConnected: s.isConnected,
      activeProfile: s.activeProfile,
      transfers: s.transfers,
      isTransferring: s.isTransferring,
      toggleLogPanel: s.toggleLogPanel,
    }));

  const activeTransfers = transfers.filter(
    (transfer) =>
      transfer.status === "uploading" ||
      transfer.status === "downloading" ||
      transfer.status === "hashing"
  );
  const completedCount = transfers.filter((transfer) => transfer.status === "complete").length;

  return (
    <div className={styles.statusbar}>
      <div className={styles.section}>
        <span className={`${styles.indicator} ${isConnected ? styles.connected : styles.disconnected}`} />
        <span className={styles.text}>
          {isConnected ? `${activeProfile?.bucket} · ${activeProfile?.region}` : "S3 연결 대기"}
        </span>
      </div>

      <div className={styles.divider} />

      <div className={styles.section}>
        {isTransferring ? (
          <span className={styles.text}>전송 중 · {activeTransfers.length}개 파일</span>
        ) : completedCount > 0 ? (
          <span className={styles.text}>완료 · {completedCount}개</span>
        ) : (
          <span className={styles.text}>준비됨</span>
        )}
      </div>

      <div className={styles.spacer} />

      <button className={styles.logToggle} onClick={toggleLogPanel} title="작업 로그 표시 전환">
        로그
      </button>
    </div>
  );
}
