import { useCallback, useState } from "react";
import { useAppStore } from "../../store/appStore";
import { useTransfer } from "../../hooks/useTransfer";
import ConfirmDialog from "../common/ConfirmDialog";
import SyncPreviewDialog from "./SyncPreviewDialog";
import UploadOptionsModal, { DEFAULT_UPLOAD_OPTIONS } from "./UploadOptionsModal";
import type { UploadOptions } from "./UploadOptionsModal";
import type { SyncPreviewResult } from "../../types";
import styles from "./TransferButtons.module.css";

import { readBatchSettings } from "../../utils/batch-settings";

function fmtSize(bytes: number): string {
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

export default function TransferButtons() {
  const { isConnected, isTransferring, local, remote, addLog, activeProfile, autoPurgeEnabled } = useAppStore((s) => ({
    isConnected: s.isConnected,
    isTransferring: s.isTransferring,
    local: s.local,
    remote: s.remote,
    addLog: s.addLog,
    activeProfile: s.activeProfile,
    autoPurgeEnabled: s.autoPurgeEnabled,
  }));
  const { startUpload, startDownload, buildPreview } = useTransfer();
  const [uploadConfirm, setUploadConfirm] = useState<{ totalSize: number; count: number } | null>(null);
  const [fileCountConfirm, setFileCountConfirm] = useState<{ message: string; totalSize: number; count: number } | null>(null);
  const [previewResult, setPreviewResult] = useState<SyncPreviewResult | null>(null);
  const [isPreviewing, setIsPreviewing] = useState(false);
  const [showOptions, setShowOptions] = useState(false);
  const [uploadOptions, setUploadOptions] = useState<UploadOptions>(DEFAULT_UPLOAD_OPTIONS);
  // 자동 Purge 승인 대기 중인 옵션 (승인 팝업 표시 트리거)
  const [autoPurgeConfirmOpts, setAutoPurgeConfirmOpts] = useState<UploadOptions | null>(null);
  // 옵션 적용 후 즉시 업로드할 파일 수 (크기 경고 통과 후 옵션 모달로 넘어온 경우)
  const [pendingUploadSize, setPendingUploadSize] = useState<{ totalSize: number; count: number } | null>(null);

  // 프로필 기본값 기반 업로드 옵션 초기화 (자동 Metadata 적용)
  const buildInitialOptions = useCallback((): UploadOptions => {
    const policy = activeProfile?.metadataPolicy;
    const autoApply = policy?.autoApply ?? false;
    return {
      contentTypeOverride: autoApply && policy?.contentType
        ? policy.contentType
        : (activeProfile?.contentTypeOverride ?? ""),
      cacheControl: autoApply && policy?.cacheControl
        ? policy.cacheControl
        : (activeProfile?.defaultCacheControl ?? ""),
      headers: autoApply && policy?.customHeaders
        ? Object.entries(policy.customHeaders).map(([key, value]) => ({ key, value }))
        : [],
      metadata: autoApply && policy?.userMetadata
        ? Object.entries(policy.userMetadata).map(([key, value]) => ({ key, value }))
        : [],
    };
  }, [activeProfile]);

  const canUpload = isConnected && !isTransferring && local.selectedPaths.size > 0;
  const canDownload = isConnected && !isTransferring && remote.selectedPaths.size > 0;
  const canPreview = isConnected && !isTransferring && !isPreviewing && !!local.path;

  const proceedToOptions = (totalSize: number, count: number) => {
    setPendingUploadSize({ totalSize, count });
    setUploadOptions(buildInitialOptions());
    setShowOptions(true);
  };

  const handleUpload = () => {
    const cfg = readBatchSettings();
    const selectedItems = local.files.filter((f) => local.selectedPaths.has(f.path));
    const hasDirectory = selectedItems.some((f) => f.isDirectory);
    const totalSize = selectedItems.reduce((sum, f) => sum + f.size, 0);
    const count = selectedItems.length;

    if (hasDirectory) {
      // 폴더 포함 시: 실제 파일 수는 sync plan이 확정하므로 개수 경고를 건너뜀
      // 대용량 경고도 폴더 크기가 0으로 표시되어 의미 없음 → 바로 옵션으로 진행
      proceedToOptions(totalSize, count);
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
      proceedToOptions(totalSize, count);
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
      <div className={styles.uploadGroup}>
        <button
          className={`${styles.transferBtn} ${styles.upload}`}
          onClick={handleUpload}
          disabled={!canUpload}
          title={`선택한 ${local.selectedPaths.size}개 파일을 S3로 업로드`}
        >
          <span className={styles.arrow}>→</span>
        </button>
        <button
          className={styles.optionsBtn}
          onClick={() => { setPendingUploadSize(null); setUploadOptions(buildInitialOptions()); setShowOptions(true); }}
          disabled={!canUpload}
          title="업로드 옵션 설정 (Content-Type, Cache-Control, 헤더, 메타데이터)"
        >
          ⚙
        </button>
      </div>

      <button
        className={`${styles.transferBtn} ${styles.preview}`}
        onClick={handlePreview}
        disabled={!canPreview}
        title="로컬 디렉터리와 S3 버킷의 차이를 미리 확인"
      >
        <span className={styles.arrow}>{isPreviewing ? "…" : "⚖"}</span>
      </button>

      <button
        className={`${styles.transferBtn} ${styles.download}`}
        onClick={startDownload}
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
              proceedToOptions(info.totalSize, info.count);
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
          confirmLabel="옵션 설정 후 업로드"
          onConfirm={() => {
            const info = uploadConfirm;
            setUploadConfirm(null);
            proceedToOptions(info.totalSize, info.count);
          }}
          onCancel={() => setUploadConfirm(null)}
        />
      )}

      {showOptions && (
        <UploadOptionsModal
          initial={uploadOptions}
          fileCount={pendingUploadSize?.count ?? local.selectedPaths.size}
          onConfirm={(opts) => {
            setUploadOptions(opts);
            setShowOptions(false);
            setPendingUploadSize(null);
            // 자동 Purge 활성화 + CDN 설정 시 → 업로드 전 Purge 승인 팝업
            if (autoPurgeEnabled && activeProfile?.cdnProvider) {
              setAutoPurgeConfirmOpts(opts);
            } else {
              startUpload(opts);
            }
          }}
          onCancel={() => { setShowOptions(false); setPendingUploadSize(null); }}
        />
      )}

      {autoPurgeConfirmOpts && (
        <ConfirmDialog
          title="자동 Purge 확인"
          message={
            <>
              <p>
                <strong>자동 Purge</strong>가 활성화되어 있습니다.
              </p>
              <p>
                업로드 완료 후 선택된 파일의 CDN 캐시({activeProfile?.cdnProvider?.toUpperCase()})를
                자동으로 Purge합니다.
              </p>
              <p>미변경(스킵) 파일도 포함하여 전체 경로를 Purge합니다.</p>
            </>
          }
          confirmLabel="업로드 + 자동 Purge 실행"
          onConfirm={() => {
            const opts = autoPurgeConfirmOpts;
            setAutoPurgeConfirmOpts(null);
            startUpload(opts);
          }}
          onCancel={() => setAutoPurgeConfirmOpts(null)}
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
