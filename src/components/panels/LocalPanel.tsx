import { useCallback, useEffect, useRef, useState } from "react";
import { ContextMenu, type MenuEntry } from "../common/ContextMenu";
import ConfirmDialog from "../common/ConfirmDialog";
import InputDialog from "../common/InputDialog";
import { useVirtualList, ITEM_H } from "../../hooks/useVirtualList";
import { runtime } from "../../services/runtime";
import { useAppStore } from "../../store/appStore";
import type { FileItem } from "../../types";
import styles from "./Panel.module.css";

type FileStatus = "new" | "modified" | "skipped" | "purge" | null;

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

function parentDir(path: string) {
  const isWin = path.includes("\\");
  const sep = isWin ? "\\" : "/";
  const parts = path.replace(/[/\\]+$/, "").split(sep).filter(Boolean);
  if (parts.length <= 1) {
    return isWin ? path : "/";
  }
  parts.pop();
  const parent = parts.join(sep);
  if (isWin) {
    return /^[A-Za-z]:$/.test(parent) ? `${parent}\\` : parent;
  }
  return `/${parent}`;
}

function StatusBadge({ status }: { status: FileStatus }) {
  if (!status) return null;
  const labels: Record<NonNullable<FileStatus>, string> = {
    new: "신규",
    modified: "수정",
    skipped: "스킵",
    purge: "Purge",
  };
  return <span className={`${styles.badge} ${styles[`badge_${status}`]}`}>{labels[status]}</span>;
}

function getFileIcon(name: string) {
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  if (["png", "jpg", "jpeg", "gif", "svg", "webp", "ico"].includes(ext)) return "IMG";
  if (["mp4", "mov", "avi", "webm"].includes(ext)) return "VID";
  if (["js", "ts", "jsx", "tsx", "html", "css", "json", "yaml", "yml", "xml"].includes(ext)) return "DEV";
  if (["zip", "tar", "gz", "rar"].includes(ext)) return "ZIP";
  return "FILE";
}

export default function LocalPanel() {
  const {
    local,
    syncPlan,
    activeProfile,
    setLocalPath,
    setLocalFiles,
    setLocalLoading,
    toggleLocalSelection,
    clearLocalSelection,
    addLog,
    setFocusedSide,
    localRefreshKey,
    focusedSide,
  } = useAppStore((s) => ({
    local:               s.local,
    syncPlan:            s.syncPlan,
    activeProfile:       s.activeProfile,
    setLocalPath:        s.setLocalPath,
    setLocalFiles:       s.setLocalFiles,
    setLocalLoading:     s.setLocalLoading,
    toggleLocalSelection: s.toggleLocalSelection,
    clearLocalSelection: s.clearLocalSelection,
    addLog:              s.addLog,
    setFocusedSide:      s.setFocusedSide,
    localRefreshKey:     s.localRefreshKey,
    focusedSide:         s.focusedSide,
  }));

  const [pathInput, setPathInput] = useState(local.path);
  const [isDragOver, setIsDragOver] = useState(false);
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; file: FileItem } | null>(null);
  const [renameDialog, setRenameDialog] = useState<FileItem | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<FileItem | null>(null);
  const pathInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => setPathInput(local.path), [local.path]);

  const loadDirectory = useCallback(
    async (path: string) => {
      setLocalLoading(true);
      try {
        const files = await runtime.invoke<FileItem[]>("list_local_dir", { path });
        setLocalFiles(files);
        setLocalPath(path);
        setPathInput(path);
        clearLocalSelection();
      } catch (err) {
        addLog("error", `로컬 폴더 로드 실패: ${err}`);
      } finally {
        setLocalLoading(false);
      }
    },
    [addLog, clearLocalSelection, setLocalFiles, setLocalLoading, setLocalPath]
  );

  // H-1: 초기 로드 + Toolbar에서 triggerLocalRefresh() 호출 시 재조회
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (local.path) {
      loadDirectory(local.path);
    } else {
      runtime.invoke<string>("get_home_dir").then((home) => loadDirectory(home));
    }
  }, [localRefreshKey]);

  const fileStatusMap = (() => {
    const map = new Map<string, FileStatus>();
    if (!syncPlan) return map;
    for (const file of syncPlan.toSkip) map.set(file.path, "skipped");
    for (const file of syncPlan.toUpload) {
      map.set(file.path, "new");
    }
    for (const file of syncPlan.toOverwrite) {
      map.set(file.path, activeProfile?.cdnProvider ? "purge" : "modified");
    }
    return map;
  })();

  const { containerRef, onScroll, visibleItems, startIndex, totalHeight, offsetTop } =
    useVirtualList(local.files);

  const handleRowClick = (event: React.MouseEvent, file: FileItem) => {
    if (event.ctrlKey || event.metaKey) {
      // Ctrl/Cmd+클릭: 파일/폴더 모두 선택 토글 (폴더는 통째 업로드)
      toggleLocalSelection(file.path);
    } else if (file.isDirectory) {
      loadDirectory(file.path);
    } else {
      clearLocalSelection();
      toggleLocalSelection(file.path);
    }
  };

  const handlePathSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    loadDirectory(pathInput);
    pathInputRef.current?.blur();
  };

  const buildMenuItems = (file: FileItem): MenuEntry[] => [
    {
      label: file.isDirectory ? "폴더 열기" : "선택",
      action: () => (file.isDirectory ? loadDirectory(file.path) : toggleLocalSelection(file.path)),
    },
    ...(file.isDirectory
      ? [{
          label: local.selectedPaths.has(file.path) ? "폴더 선택 해제" : "폴더 선택 (통째 업로드)",
          action: () => toggleLocalSelection(file.path),
        }]
      : []),
    { divider: true },
    { label: "경로 복사", action: () => navigator.clipboard.writeText(file.path) },
  ];

  const deleteLocalFile = (file: FileItem) => {
    setCtxMenu(null);
    setDeleteConfirm(file);
  };

  const doDeleteLocalFile = async (file: FileItem) => {
    try {
      await runtime.invoke("delete_local_files", { paths: [file.path] });
      await loadDirectory(local.path);
    } catch (err) {
      addLog("error", `로컬 삭제 실패: ${err}`);
    }
  };

  const renameLocalFile = (file: FileItem) => {
    setCtxMenu(null);
    setRenameDialog(file);
  };

  const doRenameLocalFile = async (file: FileItem, newName: string) => {
    const oldName = file.path.replace(/[/\\]+$/, "").split(/[/\\]/).pop() ?? file.name;
    if (newName === oldName) return;
    try {
      await runtime.invoke("rename_local_file", { oldPath: file.path, newName });
      await loadDirectory(local.path);
    } catch (err) {
      addLog("error", `로컬 이름 변경 실패: ${err}`);
    }
  };

  const selectedItems = local.files.filter((file) => local.selectedPaths.has(file.path));
  const selectedDirs  = selectedItems.filter((f) => f.isDirectory).length;
  const selectedSize  = selectedItems.reduce((sum, file) => sum + file.size, 0);
  const footerText =
    local.selectedPaths.size > 0
      ? selectedDirs > 0
        ? `${local.selectedPaths.size}개 선택 (폴더 ${selectedDirs}개 포함 — 내부 파일 전체 업로드)`
        : `${local.selectedPaths.size}개 선택 · ${fmtSize(selectedSize)}`
      : `${local.files.length}개 항목`;

  return (
    <div
      className={`${styles.panel} ${isDragOver ? styles.dragOver : ""} ${focusedSide === "local" ? styles.focused : ""}`}
      onClick={() => setFocusedSide("local")}
      onContextMenu={(event) => event.preventDefault()}
      onDragOver={(event) => {
        event.preventDefault();
        setIsDragOver(true);
      }}
      onDragLeave={() => setIsDragOver(false)}
      onDrop={() => setIsDragOver(false)}
    >
      <div className={styles.header}>
        <span className={styles.headerTitle}>
          <span className={styles.headerTitleDot} />
          로컬 파일
        </span>
        <div className={styles.pathBar}>
          <button className={styles.upBtn} onClick={() => loadDirectory(parentDir(local.path))} title="상위 폴더">
            ↑
          </button>
          <form className={styles.pathForm} onSubmit={handlePathSubmit}>
            <input
              ref={pathInputRef}
              className={styles.pathInput}
              value={pathInput}
              onChange={(event) => setPathInput(event.target.value)}
              onFocus={(event) => event.target.select()}
              spellCheck={false}
              aria-label="로컬 경로"
              title={pathInput || undefined}
            />
          </form>
        </div>
      </div>

      <div className={`${styles.row} ${styles.columnHeader}`}>
        <span className={`${styles.col} ${styles.colName}`}>이름</span>
        <span className={`${styles.col} ${styles.colSize}`}>크기</span>
        <span className={`${styles.col} ${styles.colDate}`}>수정일</span>
        <span className={`${styles.col} ${styles.colBadge}`} />
      </div>

      <div ref={containerRef} className={styles.fileList} onScroll={onScroll}>
        {local.isLoading ? (
          <div className={styles.placeholder}>로컬 파일을 불러오는 중입니다.</div>
        ) : local.files.length === 0 ? (
          <div className={styles.placeholder}>표시할 파일이 없습니다.</div>
        ) : (
          <div style={{ height: totalHeight, position: "relative" }}>
            <div style={{ transform: `translateY(${offsetTop}px)` }}>
              {visibleItems.map((file, index) => {
                const isSelected = local.selectedPaths.has(file.path);
                return (
                  <div
                    key={file.path}
                    className={`${styles.row} ${styles.fileRow} ${isSelected ? styles.selected : ""}`}
                    style={{ height: ITEM_H }}
                    data-index={startIndex + index}
                    onClick={(event) => handleRowClick(event, file)}
                    onDoubleClick={() => file.isDirectory && loadDirectory(file.path)}
                    onContextMenu={(event) => {
                      event.preventDefault();
                      setCtxMenu({ x: event.clientX, y: event.clientY, file });
                    }}
                    tabIndex={0}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        file.isDirectory ? void loadDirectory(file.path) : toggleLocalSelection(file.path);
                      } else if (event.key === " ") {
                        event.preventDefault();
                        // Space: 파일과 폴더 모두 선택 토글 (폴더는 통째 업로드)
                        toggleLocalSelection(file.path);
                      } else if (event.key === "Delete" || event.key === "Backspace") {
                        event.preventDefault();
                        deleteLocalFile(file);
                      } else if (event.key === "F2") {
                        event.preventDefault();
                        renameLocalFile(file);
                      }
                    }}
                    draggable
                    onDragStart={(event) => {
                      if (!local.selectedPaths.has(file.path)) {
                        clearLocalSelection();
                        toggleLocalSelection(file.path);
                      }
                      event.dataTransfer.setData("text/plain", "local-files");
                      event.dataTransfer.effectAllowed = "copy";
                    }}
                  >
                    <span className={`${styles.col} ${styles.colName}`}>
                      <span className={`${styles.fileIcon} ${file.isDirectory ? styles.folderIcon : ""}`}>
                        {file.isDirectory ? "DIR" : getFileIcon(file.name)}
                      </span>
                      <span className={`${styles.fileName} ${file.isDirectory ? styles.dirName : ""}`}>{file.name}</span>
                    </span>
                    <span className={`${styles.col} ${styles.colSize}`}>{file.isDirectory ? "-" : fmtSize(file.size)}</span>
                    <span className={`${styles.col} ${styles.colDate}`}>{fmtDate(file.lastModified)}</span>
                    <span className={`${styles.col} ${styles.colBadge}`}>
                      <StatusBadge status={fileStatusMap.get(file.path) ?? null} />
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </div>

      <div className={styles.footer}>{footerText}</div>

      {ctxMenu && (
        <ContextMenu
          x={ctxMenu.x}
          y={ctxMenu.y}
          items={buildMenuItems(ctxMenu.file)}
          onClose={() => setCtxMenu(null)}
        />
      )}

      {deleteConfirm && (
        <ConfirmDialog
          title="항목 삭제"
          message={
            <>
              <p><strong>{deleteConfirm.name}</strong>을(를) 삭제합니다.</p>
              <p>로컬에서 삭제된 파일은 복구할 수 없습니다.</p>
            </>
          }
          confirmLabel="삭제"
          danger
          onConfirm={() => {
            const file = deleteConfirm;
            setDeleteConfirm(null);
            void doDeleteLocalFile(file);
          }}
          onCancel={() => setDeleteConfirm(null)}
        />
      )}

      {renameDialog && (
        <InputDialog
          title="이름 변경"
          label={`"${renameDialog.name}"의 새 이름을 입력하세요.`}
          initialValue={renameDialog.name}
          placeholder="새 이름"
          confirmLabel="변경"
          onConfirm={(newName) => {
            const file = renameDialog;
            setRenameDialog(null);
            void doRenameLocalFile(file, newName);
          }}
          onCancel={() => setRenameDialog(null)}
        />
      )}
    </div>
  );
}
