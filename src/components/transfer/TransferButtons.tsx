import { useState } from "react";
import { useAppStore } from "../../store/appStore";
import { useTransfer } from "../../hooks/useTransfer";
import ConfirmDialog from "../common/ConfirmDialog";
import SyncPreviewDialog from "./SyncPreviewDialog";
import type { SyncPreviewResult } from "../../types";
import styles from "./TransferButtons.module.css";

const LARGE_UPLOAD_THRESHOLD = 100 * 1024 * 1024; // 100 MB

function fmtSize(bytes: number): string {
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

export default function TransferButtons() {
  const { isConnected, isTransferring, local, remote, addLog } = useAppStore((s) => ({
    isConnected: s.isConnected,
    isTransferring: s.isTransferring,
    local: s.local,
    remote: s.remote,
    addLog: s.addLog,
  }));
  const { startUpload, startDownload, buildPreview } = useTransfer();
  const [uploadConfirm, setUploadConfirm] = useState<{ totalSize: number; count: number } | null>(null);
  const [previewResult, setPreviewResult] = useState<SyncPreviewResult | null>(null);
  const [isPreviewing, setIsPreviewing] = useState(false);

  const canUpload = isConnected && !isTransferring && local.selectedPaths.size > 0;
  const canDownload = isConnected && !isTransferring && remote.selectedPaths.size > 0;
  const canPreview = isConnected && !isTransferring && !isPreviewing && !!local.path;

  const handleUpload = () => {
    // M-4: 100 MB 초과 시 확인 다이얼로그 표시
    const selectedFiles = local.files.filter((f) => local.selectedPaths.has(f.path));
    const totalSize = selectedFiles.reduce((sum, f) => sum + f.size, 0);
    if (totalSize > LARGE_UPLOAD_THRESHOLD) {
      setUploadConfirm({ totalSize, count: selectedFiles.length });
    } else {
      startUpload();
    }
  };

  const handlePreview = async () => {
    setIsPreviewing(true);
    try {
      const result = await buildPreview();
      setPreviewResult(result);
    } catch (err) {
      addLog("error", `동기화 미리보기 실패: ${err}`, "system");
    } finally {
      setIsPreviewing(false);
    }
  };

  return (
    <div className={styles.container}>
      <button
        className={`${styles.transferBtn} ${styles.upload}`}
        onClick={handleUpload}
        disabled={!canUpload}
        title={`선택한 ${local.selectedPaths.size}개 파일을 S3로 업로드`}
      >
        <span className={styles.arrow}>→</span>
        <span className={styles.label}>업로드</span>
      </button>

      <button
        className={`${styles.transferBtn} ${styles.preview}`}
        onClick={handlePreview}
        disabled={!canPreview}
        title="로컬 디렉터리와 S3 버킷의 차이를 미리 확인"
      >
        <span className={styles.arrow}>{isPreviewing ? "…" : "⚖"}</span>
        <span className={styles.label}>미리보기</span>
      </button>

      <button
        className={`${styles.transferBtn} ${styles.download}`}
        onClick={startDownload}
        disabled={!canDownload}
        title={`선택한 ${remote.selectedPaths.size}개 파일을 로컬로 다운로드`}
      >
        <span className={styles.arrow}>←</span>
        <span className={styles.label}>다운로드</span>
      </button>

      {uploadConfirm && (
        <ConfirmDialog
          title="대용량 업로드 확인"
          message={
            <>
              <p>
                {uploadConfirm.count}개 파일, 총{" "}
                <strong>{fmtSize(uploadConfirm.totalSize)}</strong>을 업로드합니다.
              </p>
              <p>S3 업로드 비용이 발생할 수 있습니다. 계속하시겠습니까?</p>
            </>
          }
          confirmLabel="업로드 시작"
          onConfirm={() => {
            setUploadConfirm(null);
            startUpload();
          }}
          onCancel={() => setUploadConfirm(null)}
        />
      )}

      {previewResult && (
        <SyncPreviewDialog
          result={previewResult}
          localPath={local.path}
          remotePath={remote.path}
          onClose={() => setPreviewResult(null)}
        />
      )}
    </div>
  );
}
