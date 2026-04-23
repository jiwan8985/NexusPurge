import { useCallback } from "react";
import { useAppStore } from "../../store/appStore";
import { useS3 } from "../../hooks/useS3";
import type { FileItem } from "../../types";
import styles from "./Panel.module.css";

function formatSize(bytes: number): string {
  if (bytes === 0) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleString("ko-KR", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export default function RemotePanel() {
  const { remote, isConnected, toggleRemoteSelection, setRemotePath } = useAppStore(
    (s) => ({
      remote: s.remote,
      isConnected: s.isConnected,
      toggleRemoteSelection: s.toggleRemoteSelection,
      setRemotePath: s.setRemotePath,
    })
  );
  const { listObjects } = useS3();

  const loadPrefix = useCallback(
    async (prefix: string) => {
      if (!isConnected) return;
      await listObjects(prefix);
    },
    [isConnected, listObjects]
  );

  const handleRowClick = (e: React.MouseEvent, file: FileItem) => {
    if (e.ctrlKey || e.metaKey) {
      toggleRemoteSelection(file.path);
    } else if (file.isDirectory) {
      loadPrefix(file.path);
    } else {
      toggleRemoteSelection(file.path);
    }
  };

  const goUp = () => {
    const parts = remote.path.replace(/\/$/, "").split("/");
    parts.pop();
    const parent = parts.length > 0 ? parts.join("/") + "/" : "";
    if (parent !== remote.path) {
      setRemotePath(parent);
      loadPrefix(parent);
    }
  };

  return (
    <div className={styles.panel}>
      {/* 패널 헤더 */}
      <div className={styles.header}>
        <span className={styles.headerTitle}>S3 버킷</span>
        <div className={styles.pathBar}>
          <button className={styles.upBtn} onClick={goUp} disabled={!isConnected} title="상위 경로">
            ↑
          </button>
          <span className={styles.path}>
            {isConnected ? `s3://${remote.path || ""}` : "연결 안됨"}
          </span>
        </div>
      </div>

      {/* 파일 목록 헤더 */}
      <div className={`${styles.row} ${styles.columnHeader}`}>
        <span className={`${styles.col} ${styles.colName}`}>이름</span>
        <span className={`${styles.col} ${styles.colSize}`}>크기</span>
        <span className={`${styles.col} ${styles.colDate}`}>수정일</span>
        <span className={`${styles.col} ${styles.colEtag}`}>ETag</span>
      </div>

      {/* 파일 목록 */}
      <div className={styles.fileList}>
        {!isConnected ? (
          <div className={styles.empty}>S3 버킷에 연결하세요</div>
        ) : remote.isLoading ? (
          <div className={styles.loading}>로딩 중...</div>
        ) : remote.files.length === 0 ? (
          <div className={styles.empty}>버킷이 비어 있습니다</div>
        ) : (
          remote.files.map((file) => (
            <div
              key={file.path}
              className={`${styles.row} ${styles.fileRow} ${
                remote.selectedPaths.has(file.path) ? styles.selected : ""
              }`}
              onClick={(e) => handleRowClick(e, file)}
            >
              <span className={`${styles.col} ${styles.colName}`}>
                <span className={styles.fileIcon}>
                  {file.isDirectory ? "📁" : "📄"}
                </span>
                {file.name}
              </span>
              <span className={`${styles.col} ${styles.colSize}`}>
                {file.isDirectory ? "-" : formatSize(file.size)}
              </span>
              <span className={`${styles.col} ${styles.colDate}`}>
                {formatDate(file.lastModified)}
              </span>
              <span className={`${styles.col} ${styles.colEtag}`}>
                {file.etag ? file.etag.slice(0, 8) + "…" : "-"}
              </span>
            </div>
          ))
        )}
      </div>

      {/* 하단 선택 정보 */}
      <div className={styles.footer}>
        {remote.selectedPaths.size > 0
          ? `${remote.selectedPaths.size}개 선택됨`
          : `${remote.files.length}개 항목`}
      </div>
    </div>
  );
}
