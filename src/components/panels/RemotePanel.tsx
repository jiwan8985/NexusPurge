import { useCallback, useEffect, useRef, useState } from "react";
import { ContextMenu, type MenuEntry } from "../common/ContextMenu";
import { useS3 } from "../../hooks/useS3";
import { useTransfer } from "../../hooks/useTransfer";
import { useVirtualList, ITEM_H } from "../../hooks/useVirtualList";
import { useAppStore } from "../../store/appStore";
import type { FileItem } from "../../types";
import styles from "./Panel.module.css";

function fmtSize(bytes: number) {
  if (bytes === 0) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

function fmtDate(iso: string) {
  if (!iso) return "-";
  return new Date(iso).toLocaleString("ko-KR", {
    year: "2-digit",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export default function RemotePanel() {
  const { remote, isConnected, activeProfile, toggleRemoteSelection, clearRemoteSelection, addLog } =
    useAppStore((s) => ({
      remote: s.remote,
      isConnected: s.isConnected,
      activeProfile: s.activeProfile,
      toggleRemoteSelection: s.toggleRemoteSelection,
      clearRemoteSelection: s.clearRemoteSelection,
      addLog: s.addLog,
    }));

  const { listObjects, deleteObjects, getPresignedUrl } = useS3();
  const { startUpload } = useTransfer();
  const [pathInput, setPathInput] = useState(remote.path);
  const [isDragOver, setIsDragOver] = useState(false);
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; file: FileItem } | null>(null);
  const pathInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => setPathInput(remote.path), [remote.path]);

  const loadPrefix = useCallback(
    async (prefix: string) => {
      if (!isConnected) return;
      await listObjects(prefix);
      setPathInput(prefix);
      clearRemoteSelection();
    },
    [clearRemoteSelection, isConnected, listObjects]
  );

  const goUp = () => {
    const trimmed = remote.path.replace(/\/$/, "");
    const lastSlash = trimmed.lastIndexOf("/");
    const parent = lastSlash >= 0 ? trimmed.slice(0, lastSlash + 1) : "";
    if (parent !== remote.path) loadPrefix(parent);
  };

  const handlePathSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    loadPrefix(pathInput);
    pathInputRef.current?.blur();
  };

  const buildMenuItems = (file: FileItem): MenuEntry[] => [
    {
      label: file.isDirectory ? "폴더 열기" : "다운로드 선택",
      action: () => (file.isDirectory ? loadPrefix(file.path) : toggleRemoteSelection(file.path)),
      disabled: !isConnected,
    },
    { divider: true },
    {
      label: "Presigned URL 복사",
      action: async () => {
        try {
          const url = await getPresignedUrl(file.path, 3600);
          await navigator.clipboard.writeText(url);
          addLog("success", `Presigned URL 복사 완료: ${file.name}`);
        } catch (err) {
          addLog("error", `Presigned URL 생성 실패: ${err}`);
        }
      },
      disabled: !isConnected || file.isDirectory,
    },
    { divider: true },
    {
      label: "삭제",
      action: async () => {
        if (!confirm(`"${file.name}" 항목을 삭제할까요?`)) return;
        await deleteObjects([file.path]);
        await loadPrefix(remote.path);
      },
      disabled: !isConnected,
      danger: true,
    },
  ];

  const { containerRef, onScroll, visibleItems, startIndex, totalHeight, offsetTop } =
    useVirtualList(remote.files);

  const selectedFiles = remote.files.filter((file) => remote.selectedPaths.has(file.path));
  const selectedSize = selectedFiles.reduce((sum, file) => sum + file.size, 0);
  const footerText =
    remote.selectedPaths.size > 0
      ? `${remote.selectedPaths.size}개 선택 · ${fmtSize(selectedSize)}`
      : `${remote.files.length}개 항목`;
  const bucketLabel = activeProfile ? `s3://${activeProfile.bucket}/${remote.path}` : "S3 프로필을 연결하세요.";

  return (
    <div
      className={`${styles.panel} ${isDragOver ? styles.dragOver : ""}`}
      onDragOver={(event) => {
        if (!isConnected) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "copy";
        setIsDragOver(true);
      }}
      onDragLeave={() => setIsDragOver(false)}
      onDrop={async (event) => {
        event.preventDefault();
        setIsDragOver(false);
        if (!isConnected || event.dataTransfer.getData("text/plain") !== "local-files") return;
        await startUpload();
      }}
    >
      <div className={styles.header}>
        <span className={styles.headerTitle}>
          <span className={`${styles.headerTitleDot} ${isConnected ? styles.active : ""}`} />
          S3 버킷
        </span>
        <div className={styles.pathBar}>
          <button className={styles.upBtn} onClick={goUp} disabled={!isConnected} title="상위 경로">
            ↑
          </button>
          <form className={styles.pathForm} onSubmit={handlePathSubmit}>
            <input
              ref={pathInputRef}
              className={styles.pathInput}
              value={isConnected ? pathInput : "연결되지 않음"}
              onChange={(event) => setPathInput(event.target.value)}
              onFocus={(event) => event.target.select()}
              disabled={!isConnected}
              spellCheck={false}
              aria-label="S3 경로"
            />
          </form>
          {isConnected && (
            <button className={styles.refreshBtn} onClick={() => loadPrefix(remote.path)} title="새로고침">
              ↻
            </button>
          )}
        </div>
      </div>

      <div className={`${styles.row} ${styles.columnHeader}`}>
        <span className={`${styles.col} ${styles.colName}`}>이름</span>
        <span className={`${styles.col} ${styles.colSize}`}>크기</span>
        <span className={`${styles.col} ${styles.colDate}`}>수정일</span>
        <span className={`${styles.col} ${styles.colEtag}`}>ETag</span>
      </div>

      <div ref={containerRef} className={styles.fileList} onScroll={onScroll}>
        {!isConnected ? (
          <div className={styles.placeholder}>
            <span className={styles.placeholderIcon}>S3</span>
            <span className={styles.placeholderText}>상단에서 프로필을 연결하면 버킷 파일을 볼 수 있습니다.</span>
          </div>
        ) : remote.isLoading ? (
          <div className={styles.placeholder}>S3 객체를 불러오는 중입니다.</div>
        ) : remote.files.length === 0 ? (
          <div className={styles.placeholder}>버킷 경로가 비어 있습니다.</div>
        ) : (
          <div style={{ height: totalHeight, position: "relative" }}>
            <div style={{ transform: `translateY(${offsetTop}px)` }}>
              {visibleItems.map((file, index) => {
                const isSelected = remote.selectedPaths.has(file.path);
                return (
                  <div
                    key={file.path}
                    className={`${styles.row} ${styles.fileRow} ${isSelected ? styles.selected : ""}`}
                    style={{ height: ITEM_H }}
                    data-index={startIndex + index}
                    onClick={(event) => {
                      if (event.ctrlKey || event.metaKey) {
                        toggleRemoteSelection(file.path);
                      } else if (file.isDirectory) {
                        loadPrefix(file.path);
                      } else {
                        clearRemoteSelection();
                        toggleRemoteSelection(file.path);
                      }
                    }}
                    onDoubleClick={() => file.isDirectory && loadPrefix(file.path)}
                    onContextMenu={(event) => {
                      event.preventDefault();
                      setCtxMenu({ x: event.clientX, y: event.clientY, file });
                    }}
                  >
                    <span className={`${styles.col} ${styles.colName}`}>
                      <span className={`${styles.fileIcon} ${file.isDirectory ? styles.folderIcon : ""}`}>
                        {file.isDirectory ? "DIR" : "OBJ"}
                      </span>
                      <span className={`${styles.fileName} ${file.isDirectory ? styles.dirName : ""}`}>{file.name}</span>
                    </span>
                    <span className={`${styles.col} ${styles.colSize}`}>{file.isDirectory ? "-" : fmtSize(file.size)}</span>
                    <span className={`${styles.col} ${styles.colDate}`}>{fmtDate(file.lastModified)}</span>
                    <span className={`${styles.col} ${styles.colEtag}`}>
                      {file.etag ? (file.etag.includes("-") ? file.etag : `${file.etag.slice(0, 8)}...`) : "-"}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </div>

      <div className={styles.footer}>
        {isConnected ? footerText : bucketLabel}
        {isDragOver && <span className={styles.dropHint}>여기에 놓으면 업로드됩니다.</span>}
      </div>

      {ctxMenu && (
        <ContextMenu
          x={ctxMenu.x}
          y={ctxMenu.y}
          items={buildMenuItems(ctxMenu.file)}
          onClose={() => setCtxMenu(null)}
        />
      )}
    </div>
  );
}
