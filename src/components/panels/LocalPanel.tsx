import { useEffect, useCallback } from "react";
import { useAppStore } from "../../store/appStore";
import { useTransfer } from "../../hooks/useTransfer";
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

export default function LocalPanel() {
  const { local, setLocalPath, setLocalFiles, setLocalLoading, toggleLocalSelection } =
    useAppStore((s) => ({
      local: s.local,
      setLocalPath: s.setLocalPath,
      setLocalFiles: s.setLocalFiles,
      setLocalLoading: s.setLocalLoading,
      toggleLocalSelection: s.toggleLocalSelection,
    }));

  const loadDirectory = useCallback(
    async (path: string) => {
      setLocalLoading(true);
      try {
        // TODO: Tauri invoke("list_local_dir", { path })
        // const files: FileItem[] = await invoke("list_local_dir", { path });
        // setLocalFiles(files);
        setLocalFiles([]);
        setLocalPath(path);
      } catch (err) {
        console.error("Failed to load local directory:", err);
      } finally {
        setLocalLoading(false);
      }
    },
    [setLocalFiles, setLocalLoading, setLocalPath]
  );

  useEffect(() => {
    loadDirectory(local.path);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const handleRowClick = (e: React.MouseEvent, file: FileItem) => {
    if (e.ctrlKey || e.metaKey) {
      toggleLocalSelection(file.path);
    } else if (file.isDirectory) {
      loadDirectory(file.path);
    } else {
      toggleLocalSelection(file.path);
    }
  };

  const handleRowDoubleClick = (file: FileItem) => {
    if (file.isDirectory) loadDirectory(file.path);
  };

  const goUp = () => {
    const parent = local.path.replace(/[/\\][^/\\]+[/\\]?$/, "") || local.path;
    if (parent !== local.path) loadDirectory(parent);
  };

  return (
    <div className={styles.panel}>
      {/* 패널 헤더 */}
      <div className={styles.header}>
        <span className={styles.headerTitle}>로컬</span>
        <div className={styles.pathBar}>
          <button className={styles.upBtn} onClick={goUp} title="상위 폴더">
            ↑
          </button>
          <span className={styles.path}>{local.path}</span>
        </div>
      </div>

      {/* 파일 목록 헤더 */}
      <div className={`${styles.row} ${styles.columnHeader}`}>
        <span className={`${styles.col} ${styles.colName}`}>이름</span>
        <span className={`${styles.col} ${styles.colSize}`}>크기</span>
        <span className={`${styles.col} ${styles.colDate}`}>수정일</span>
      </div>

      {/* 파일 목록 */}
      <div className={styles.fileList}>
        {local.isLoading ? (
          <div className={styles.loading}>로딩 중...</div>
        ) : local.files.length === 0 ? (
          <div className={styles.empty}>폴더가 비어 있습니다</div>
        ) : (
          local.files.map((file) => (
            <div
              key={file.path}
              className={`${styles.row} ${styles.fileRow} ${
                local.selectedPaths.has(file.path) ? styles.selected : ""
              }`}
              onClick={(e) => handleRowClick(e, file)}
              onDoubleClick={() => handleRowDoubleClick(file)}
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
            </div>
          ))
        )}
      </div>

      {/* 하단 선택 정보 */}
      <div className={styles.footer}>
        {local.selectedPaths.size > 0
          ? `${local.selectedPaths.size}개 선택됨`
          : `${local.files.length}개 항목`}
      </div>
    </div>
  );
}
