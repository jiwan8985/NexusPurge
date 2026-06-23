import { useCallback, useEffect, useRef, useState } from "react";
import { ContextMenu, type MenuEntry } from "../common/ContextMenu";
import ConfirmDialog from "../common/ConfirmDialog";
import InputDialog from "../common/InputDialog";
import { useS3 } from "../../hooks/useS3";
import { useTransfer } from "../../hooks/useTransfer";
import { useVirtualList, ITEM_H } from "../../hooks/useVirtualList";
import { runtime } from "../../services/runtime";
import { useAppStore } from "../../store/appStore";
import { buildCdnUrl } from "../../utils/cdn";
import type { CdnUrlCheck, FileItem } from "../../types";
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
  const {
    remote,
    isConnected,
    activeProfile,
    toggleRemoteSelection,
    clearRemoteSelection,
    addLog,
    setFocusedSide,
    remoteRefreshKey,
    focusedSide,
  } = useAppStore((s) => ({
    remote:               s.remote,
    isConnected:          s.isConnected,
    activeProfile:        s.activeProfile,
    toggleRemoteSelection: s.toggleRemoteSelection,
    clearRemoteSelection: s.clearRemoteSelection,
    addLog:               s.addLog,
    setFocusedSide:       s.setFocusedSide,
    remoteRefreshKey:     s.remoteRefreshKey,
    focusedSide:          s.focusedSide,
  }));

  const { listObjects, deleteObjects, getPresignedUrl, renameObject } = useS3();
  const { startUpload } = useTransfer();
  const [pathInput, setPathInput] = useState(remote.path);
  const [isDragOver, setIsDragOver] = useState(false);
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; file: FileItem } | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<FileItem | null>(null);
  const [renameDialog, setRenameDialog] = useState<FileItem | null>(null);
  const pathInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => setPathInput(remote.path), [remote.path]);

  // H-1: Toolbar에서 triggerRemoteRefresh() 호출 시 재조회
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { if (isConnected) loadPrefix(remote.path); }, [remoteRefreshKey]);

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

  const copyCdnUrl = async (file: FileItem) => {
    const url = buildCdnUrl(activeProfile?.cdnDomain, file.path);
    if (!url) {
      addLog("warn", "CDN 도메인이 설정되지 않았습니다.", "cdn");
      return;
    }
    await navigator.clipboard.writeText(url);
    addLog("success", `CDN URL 복사 완료: ${file.name}`, "cdn");
  };

  const openCdnUrl = (file: FileItem) => {
    const url = buildCdnUrl(activeProfile?.cdnDomain, file.path);
    if (!url) {
      addLog("warn", "CDN 도메인이 설정되지 않았습니다.", "cdn");
      return;
    }
    window.open(url, "_blank", "noopener,noreferrer");
  };

  const verifyCdnUrl = async (file: FileItem) => {
    if (!activeProfile) return;
    try {
      const [check] = await runtime.invoke<CdnUrlCheck[]>("verify_cdn_urls", {
        profileId: activeProfile.id,
        paths: [file.path],
      });
      if (check?.ok) {
        addLog("success", `CDN 확인 성공: ${file.name} (${check.statusCode})`, "cdn");
      } else {
        addLog("warn", `CDN 확인 실패: ${file.name} (${check?.error ?? check?.statusCode ?? "응답 없음"})`, "cdn");
      }
    } catch (err) {
      addLog("error", `CDN 확인 오류: ${err}`, "cdn");
    }
  };

  const renameRemoteFile = (file: FileItem) => {
    setCtxMenu(null);
    setRenameDialog(file);
  };

  const doRenameRemoteFile = async (file: FileItem, newName: string) => {
    const oldName = file.path.replace(/\/$/, "").split("/").pop() ?? file.name;
    if (newName === oldName) return;
    const newKey = file.path.replace(/[^/]*\/?$/, newName + (file.path.endsWith("/") ? "/" : ""));
    await renameObject(file.path, newKey);
    await loadPrefix(remote.path);
  };

  const buildMenuItems = (file: FileItem): MenuEntry[] => {
    const copyPresigned = async (seconds: number, label: string) => {
      try {
        const url = await getPresignedUrl(file.path, seconds);
        await navigator.clipboard.writeText(url);
        addLog("success", `Presigned URL 복사 완료 (${label}): ${file.name}`, "system");
      } catch (err) {
        addLog("error", `Presigned URL 생성 실패: ${err}`, "system");
      }
    };
    const urlDisabled = !isConnected || file.isDirectory;
    return [
      {
        label: file.isDirectory ? "폴더 열기" : "다운로드 선택",
        action: () => (file.isDirectory ? loadPrefix(file.path) : toggleRemoteSelection(file.path)),
        disabled: !isConnected,
      },
      { divider: true },
      { label: "URL 복사 (15분)", action: () => copyPresigned(900, "15분"), disabled: urlDisabled },
      { label: "URL 복사 (1시간)", action: () => copyPresigned(3600, "1시간"), disabled: urlDisabled },
      { label: "URL 복사 (24시간)", action: () => copyPresigned(86400, "24시간"), disabled: urlDisabled },
      {
        label: "CDN URL 복사",
        action: () => copyCdnUrl(file),
        disabled: !isConnected || file.isDirectory || !activeProfile?.cdnDomain,
      },
      {
        label: "CDN URL 열기",
        action: () => openCdnUrl(file),
        disabled: !isConnected || file.isDirectory || !activeProfile?.cdnDomain,
      },
      {
        label: "CDN 반영 확인",
        action: () => verifyCdnUrl(file),
        disabled: !isConnected || file.isDirectory || !activeProfile?.cdnDomain,
      },
      { divider: true },
      {
        label: "이름 변경",
        action: () => {
          setCtxMenu(null);
          renameRemoteFile(file);
        },
        disabled: !isConnected,
      },
      {
        label: "삭제",
        action: () => setDeleteConfirm(file),
        disabled: !isConnected,
        danger: true,
      },
    ];
  };

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
      className={`${styles.panel} ${isDragOver ? styles.dragOver : ""} ${focusedSide === "remote" ? styles.focused : ""}`}
      onClick={() => setFocusedSide("remote")}
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
              title={isConnected ? pathInput : undefined}
            />
          </form>
          {isConnected && (
            <button
              className={styles.refreshBtn}
              onClick={() => loadPrefix(remote.path)}
              disabled={remote.isLoading}
              title="새로고침"
            >
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
          <div className={styles.placeholder}>
            {activeProfile?.cdnProvider
              ? "현재 S3 경로가 비어 있습니다. CDN은 업로드 대상이 아니며, 업로드된 S3 객체가 CDN origin으로 제공됩니다."
              : "현재 S3 경로가 비어 있습니다. CDN을 설정하면 업로드/삭제 후 Purge와 CDN URL 확인을 사용할 수 있습니다."}
          </div>
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
                    tabIndex={0}
                    onKeyDown={async (event) => {
                      if (event.key === "Enter") {
                        file.isDirectory ? await loadPrefix(file.path) : toggleRemoteSelection(file.path);
                      } else if (event.key === " ") {
                        event.preventDefault();
                        toggleRemoteSelection(file.path);
                      } else if (event.key === "Delete" || event.key === "Backspace") {
                        event.preventDefault();
                        setDeleteConfirm(file);
                      } else if (event.key === "F2") {
                        event.preventDefault();
                        renameRemoteFile(file);
                      }
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

      {deleteConfirm && (
        <ConfirmDialog
          title="항목 삭제"
          message={
            <>
              <p>
                <strong>{deleteConfirm.name}</strong>을(를) 삭제합니다.
              </p>
              <p>S3에서 삭제된 파일은 복구할 수 없습니다.</p>
              {activeProfile?.cdnProvider && (
                <p>삭제에 성공한 S3 객체만 CDN Purge 대상으로 전송됩니다.</p>
              )}
            </>
          }
          confirmLabel="삭제"
          danger
          onConfirm={async () => {
            const target = deleteConfirm;
            setDeleteConfirm(null);
            try {
              await deleteObjects([target.path]);
            } catch (err) {
              // 에러 로그는 이미 useS3 내부에서 기록함
            } finally {
              await loadPrefix(remote.path);
            }
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
            void doRenameRemoteFile(file, newName);
          }}
          onCancel={() => setRenameDialog(null)}
        />
      )}
    </div>
  );
}
