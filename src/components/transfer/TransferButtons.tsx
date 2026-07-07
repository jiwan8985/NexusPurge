import { useState } from "react";
import { useAppStore } from "../../store/appStore";
import { useTransfer } from "../../hooks/useTransfer";
import ConfirmDialog from "../common/ConfirmDialog";
import styles from "./TransferButtons.module.css";

import { CDN_LABELS } from "../../utils/cdn";
import { readBatchSettings } from "../../utils/batch-settings";

function fmtSize(bytes: number): string {
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

export default function TransferButtons() {
  const { isConnected, isTransferring, local, remote, activeCdn, autoPurgeEnabled } = useAppStore((s) => ({
    isConnected: s.isConnected,
    isTransferring: s.isTransferring,
    local: s.local,
    remote: s.remote,
    activeCdn: s.activeCdn,
    autoPurgeEnabled: s.autoPurgeEnabled,
  }));
  const { startUpload, startDownload } = useTransfer();
  const [uploadConfirm, setUploadConfirm] = useState<{ totalSize: number; count: number } | null>(null);
  const [fileCountConfirm, setFileCountConfirm] = useState<{ message: string; totalSize: number; count: number } | null>(null);
  // 자동 Purge 승인 대기 (승인 팝업 표시 트리거)
  const [autoPurgeConfirm, setAutoPurgeConfirm] = useState(false);

  const canUpload = isConnected && !isTransferring && local.selectedPaths.size > 0;
  const canDownload = isConnected && !isTransferring && remote.selectedPaths.size > 0;

  // 경고 통과 후 실제 업로드 진입 (자동 Purge ON이면 승인 팝업 먼저)
  const proceedUpload = () => {
    if (autoPurgeEnabled && activeCdn) {
      setAutoPurgeConfirm(true);
    } else {
      startUpload();
    }
  };

  const handleUpload = () => {
    const cfg = readBatchSettings();
    const selectedItems = local.files.filter((f) => local.selectedPaths.has(f.path));
    const hasDirectory = selectedItems.some((f) => f.isDirectory);
    const totalSize = selectedItems.reduce((sum, f) => sum + f.size, 0);
    const count = selectedItems.length;

    if (hasDirectory) {
      // 폴더 포함 시: 실제 파일 수는 sync plan이 확정하므로 개수/크기 경고를 건너뜀
      proceedUpload();
      return;
    }

    if (count >= cfg.fileCountLimit) {
      setFileCountConfirm({
        message: `선택한 파일이 ${count}개입니다 (${cfg.fileCountLimit.toLocaleString()}개 이상). S3/CDN 제한으로 처리 시간이 매우 오래 걸리거나 실패할 수 있습니다.`,
        totalSize, count,
      });
      return;
    } else if (count >= cfg.fileCountWarn) {
      setFileCountConfirm({
        message: `선택한 파일이 ${count}개입니다 (${cfg.fileCountWarn.toLocaleString()}개 이상). 배치 처리 시간이 오래 걸릴 수 있습니다.`,
        totalSize, count,
      });
      return;
    }

    const largeThreshold = cfg.largeSizeMb * 1024 * 1024;
    if (totalSize > largeThreshold) {
      setUploadConfirm({ totalSize, count });
    } else {
      proceedUpload();
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
      </button>

      <button
        className={`${styles.transferBtn} ${styles.download}`}
        onClick={() => startDownload()}
        disabled={!canDownload}
        title={`선택한 ${remote.selectedPaths.size}개 파일을 로컬로 다운로드`}
      >
        <span className={styles.arrow}>←</span>
      </button>

      {fileCountConfirm && (
        <ConfirmDialog
          title="파일 수 경고"
          message={<p>{fileCountConfirm.message}</p>}
          confirmLabel="계속 진행"
          onConfirm={() => {
            const info = fileCountConfirm;
            setFileCountConfirm(null);
            const largeThreshold = readBatchSettings().largeSizeMb * 1024 * 1024;
            if (info.totalSize > largeThreshold) {
              setUploadConfirm({ totalSize: info.totalSize, count: info.count });
            } else {
              proceedUpload();
            }
          }}
          onCancel={() => setFileCountConfirm(null)}
        />
      )}

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
          confirmLabel="업로드"
          onConfirm={() => {
            setUploadConfirm(null);
            proceedUpload();
          }}
          onCancel={() => setUploadConfirm(null)}
        />
      )}

      {autoPurgeConfirm && (
        <ConfirmDialog
          title="자동 Purge 확인"
          message={
            <>
              <p>
                <strong>자동 Purge</strong>가 활성화되어 있습니다.
              </p>
              <p>
                업로드 완료 후 선택된 파일의 CDN 캐시({activeCdn ? CDN_LABELS[activeCdn] : ""})를
                자동으로 Purge합니다.
              </p>
              <p>미변경(스킵) 파일도 포함하여 전체 경로를 Purge합니다.</p>
            </>
          }
          confirmLabel="업로드 + 자동 Purge 실행"
          onConfirm={() => {
            setAutoPurgeConfirm(false);
            startUpload();
          }}
          onCancel={() => setAutoPurgeConfirm(false)}
        />
      )}
    </div>
  );
}
