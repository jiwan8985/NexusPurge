import { useAppStore } from "../../store/appStore";
import { CDN_LABELS } from "../../utils/cdn";
import { readBatchSettings } from "../../utils/batch-settings";
import styles from "./StatusBar.module.css";

function fmtSpeed(bytesPerSec: number) {
  if (bytesPerSec < 1024) return `${bytesPerSec} B/s`;
  if (bytesPerSec < 1048576) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
  return `${(bytesPerSec / 1048576).toFixed(1)} MB/s`;
}

function fmtSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

export default function StatusBar() {
  const {
    isConnected, activeProfile, activeCdns, transfers, isTransferring, toggleLogPanel, local, remote, networkStats,
  } = useAppStore((s) => ({
      isConnected:     s.isConnected,
      activeProfile:   s.activeProfile,
      activeCdns:      s.activeCdns,
      transfers:       s.transfers,
      isTransferring:  s.isTransferring,
      toggleLogPanel:  s.toggleLogPanel,
      local:           s.local,
      remote:          s.remote,
      networkStats:    s.networkStats,
    }));

  const activeTransfers = transfers.filter(
    (t) => t.status === "uploading" || t.status === "downloading" || t.status === "hashing"
  );
  const completedCount = transfers.filter((t) => t.status === "complete").length;
  const errorCount     = transfers.filter((t) => t.status === "error").length;
  const uploadSpeed    = activeTransfers
    .filter((t) => t.direction === "upload")
    .reduce((sum, t) => sum + (t.speed ?? 0), 0);
  const downloadSpeed  = activeTransfers
    .filter((t) => t.direction === "download")
    .reduce((sum, t) => sum + (t.speed ?? 0), 0);
  const maxConcurrentTransfers = readBatchSettings().maxConcurrentTransfers;

  // 포커스된 패널의 선택 항목 정보
  const localSelected  = local.selectedPaths.size;
  const remoteSelected = remote.selectedPaths.size;
  const localSelSize   = local.files
    .filter((f) => local.selectedPaths.has(f.path))
    .reduce((sum, f) => sum + f.size, 0);
  const remoteSelSize  = remote.files
    .filter((f) => remote.selectedPaths.has(f.path))
    .reduce((sum, f) => sum + f.size, 0);

  return (
    <div className={styles.statusbar}>
      {/* 연결 상태 */}
      <div className={styles.section}>
        <span className={`${styles.indicator} ${isConnected ? styles.connected : styles.disconnected}`} />
        <span className={styles.text}>
          {isConnected
            ? `${activeProfile?.bucket} · ${activeProfile?.region}`
            : "S3 연결 대기"}
        </span>
        {isConnected && activeCdns.map((c) => (
          <span key={c} className={styles.cdnBadge} title="현재 Purge 대상 CDN">
            {CDN_LABELS[c]}
          </span>
        ))}
      </div>

      <div className={styles.divider} />

      {/* 전송 상태 */}
      <div className={styles.section}>
        {isTransferring ? (
          <>
            <span className={`${styles.text} ${styles.active}`}>
              전송 중 {activeTransfers.length}개
            </span>
            {uploadSpeed > 0 && (
              <span className={styles.speed} title="업로드 속도">⬆ {fmtSpeed(uploadSpeed)}</span>
            )}
            {downloadSpeed > 0 && (
              <span className={styles.speed} title="다운로드 속도">⬇ {fmtSpeed(downloadSpeed)}</span>
            )}
          </>
        ) : completedCount > 0 || errorCount > 0 ? (
          <span className={styles.text}>
            완료 {completedCount}개
            {errorCount > 0 && <span className={styles.errBadge}> 오류 {errorCount}</span>}
          </span>
        ) : (
          <span className={styles.text}>준비됨</span>
        )}
      </div>

      <div className={styles.divider} />

      {/* 네트워크 상태 */}
      <div className={styles.section}>
        <span className={styles.textMuted} title="활성 S3 요청 수 / 최대 동시 전송 수">
          연결 {networkStats.activeS3Calls}/{maxConcurrentTransfers}
        </span>
        {networkStats.avgRttMs != null && (
          <span className={styles.textMuted} title="최근 S3/CDN 호출 평균 응답시간">
            · 평균RTT {networkStats.avgRttMs}ms
          </span>
        )}
      </div>

      <div className={styles.divider} />

      {/* 선택 항목 정보 */}
      <div className={styles.section}>
        {localSelected > 0 && (
          <span className={styles.text}>
            로컬 {localSelected}개 선택
            {localSelSize > 0 && ` · ${fmtSize(localSelSize)}`}
          </span>
        )}
        {remoteSelected > 0 && (
          <span className={styles.text}>
            S3 {remoteSelected}개 선택
            {remoteSelSize > 0 && ` · ${fmtSize(remoteSelSize)}`}
          </span>
        )}
        {localSelected === 0 && remoteSelected === 0 && (
          <span className={styles.textMuted}>
            {isConnected
              ? `로컬 ${local.files.length}개 · S3 ${remote.files.length}개`
              : "항목 없음"}
          </span>
        )}
      </div>

      <div className={styles.spacer} />

      <button className={styles.logToggle} onClick={toggleLogPanel} title="작업 로그 표시 전환 (L)">
        로그
      </button>
    </div>
  );
}
