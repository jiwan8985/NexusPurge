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
    (t) => t.status === "uploading" || t.status === "downloading" || t.status === "hashing"
  );

  const completedCount = transfers.filter((t) => t.status === "complete").length;

  return (
    <div className={styles.statusbar}>
      {/* 연결 상태 */}
      <div className={styles.section}>
        <span
          className={`${styles.indicator} ${isConnected ? styles.connected : styles.disconnected}`}
        />
        <span className={styles.text}>
          {isConnected ? `${activeProfile?.bucket} (${activeProfile?.region})` : "연결 안됨"}
        </span>
      </div>

      <div className={styles.divider} />

      {/* 전송 상태 */}
      <div className={styles.section}>
        {isTransferring ? (
          <span className={styles.text}>
            전송 중: {activeTransfers.length}개 파일
          </span>
        ) : completedCount > 0 ? (
          <span className={styles.text}>완료: {completedCount}개</span>
        ) : (
          <span className={styles.text}>대기 중</span>
        )}
      </div>

      <div className={styles.spacer} />

      {/* 로그 패널 토글 */}
      <button className={styles.logToggle} onClick={toggleLogPanel} title="로그 패널 토글">
        로그
      </button>
    </div>
  );
}
