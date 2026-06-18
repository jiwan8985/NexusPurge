import { useEffect, useState } from "react";
import { useAppStore } from "../../store/appStore";
import { useProfile } from "../../hooks/useProfile";
import { useS3 } from "../../hooks/useS3";
import { useLocalFs } from "../../hooks/useLocalFs";
import { usePurge } from "../../hooks/usePurge";
import PurgeDialog from "../modals/PurgeDialog";
import { runtime } from "../../services/runtime";
import type { SyncPreviewResult } from "../../types";
import styles from "./Toolbar.module.css";

export default function Toolbar() {
  const {
    activeProfile,
    isConnected,
    isConnecting,
    openProfileModal,
    openSettingsModal,
    focusedSide,
    local,
    remote,
    triggerLocalRefresh,
    triggerRemoteRefresh,
    setSyncPreview,
    setShowSyncPreview,
    addLog,
    autoPurgeEnabled,
    toggleAutoPurge,
  } = useAppStore((s) => ({
    activeProfile:        s.activeProfile,
    isConnected:          s.isConnected,
    isConnecting:         s.isConnecting,
    openProfileModal:     s.openProfileModal,
    openSettingsModal:    s.openSettingsModal,
    focusedSide:          s.focusedSide,
    local:                s.local,
    remote:               s.remote,
    triggerLocalRefresh:  s.triggerLocalRefresh,
    triggerRemoteRefresh: s.triggerRemoteRefresh,
    setSyncPreview:       s.setSyncPreview,
    setShowSyncPreview:   s.setShowSyncPreview,
    addLog:               s.addLog,
    autoPurgeEnabled:     s.autoPurgeEnabled,
    toggleAutoPurge:      s.toggleAutoPurge,
  }));

  // 프로필 권한 헬퍼
  const perms = activeProfile?.permissions;
  const canPurge   = !perms || perms.canPurge;
  const canCreate  = !perms || perms.canCreate;

  const { disconnect, connectWithProfile } = useProfile();
  const { deleteObjects, createDirectory, renameObject } = useS3();
  const { createDir, deleteFiles, renameFile } = useLocalFs();
  const { executePurge, selectedPaths: remotePurgePaths, allPrefix } = usePurge();

  const [purgeDialog, setPurgeDialog] = useState<{ paths: string[]; mode: "selected" | "all" } | null>(null);

  // H-1: 새 폴더
  const handleNewFolder = async () => {
    const name = window.prompt("새 폴더 이름을 입력하세요:");
    if (!name || !name.trim()) return;

    if (focusedSide === "remote" && isConnected) {
      const prefix = remote.path.endsWith("/") ? remote.path : remote.path + "/";
      await createDirectory(prefix + name.trim() + "/");
      triggerRemoteRefresh();
    } else {
      const sep = local.path.includes("\\") ? "\\" : "/";
      const base = local.path.replace(/[/\\]+$/, "");
      await createDir(base + sep + name.trim());
      triggerLocalRefresh();
    }
  };

  const handleDryRun = async () => {
    if (!activeProfile || !isConnected) return;
    try {
      const preview = await runtime.invoke<SyncPreviewResult>("sync_preview", {
        profileId: activeProfile.id,
        localDir: local.path,
        remotePrefix: remote.path,
      });
      setSyncPreview(preview);
      setShowSyncPreview(true);
      addLog(
        "info",
        `Dry-run: 신규 ${preview.new.length}, 수정 ${preview.modified.length}, Purge ${preview.purgeTargets.length}`
      );
    } catch (err) {
      addLog("error", `Dry-run 실패: ${err}`);
    }
  };

  // H-1: 삭제
  const handleDelete = async () => {
    if (focusedSide === "remote" && isConnected) {
      const keys = Array.from(remote.selectedPaths);
      if (keys.length === 0) return;
      const purgeNotice = activeProfile?.cdnProvider
        ? "\n삭제 성공한 항목은 CDN 캐시도 Purge됩니다."
        : "";
      if (!window.confirm(`S3에서 ${keys.length}개 항목을 삭제할까요?${purgeNotice}`)) return;
      await deleteObjects(keys);
      triggerRemoteRefresh();
    } else {
      const paths = Array.from(local.selectedPaths);
      if (paths.length === 0) return;
      if (!window.confirm(`로컬에서 ${paths.length}개 항목을 삭제할까요?`)) return;
      await deleteFiles(paths);
      triggerLocalRefresh();
    }
  };

  // H-1: 이름 변경
  const handleRename = async () => {
    if (focusedSide === "remote" && isConnected) {
      const keys = Array.from(remote.selectedPaths);
      if (keys.length !== 1) {
        window.alert("이름 변경은 항목 1개만 선택하세요.");
        return;
      }
      const oldKey = keys[0];
      const oldName = oldKey.replace(/\/$/, "").split("/").pop() ?? oldKey;
      const newName = window.prompt("새 이름을 입력하세요:", oldName);
      if (!newName || !newName.trim() || newName.trim() === oldName) return;
      const newKey = oldKey.replace(/[^/]*\/?$/, newName.trim() + (oldKey.endsWith("/") ? "/" : ""));
      await renameObject(oldKey, newKey);
      triggerRemoteRefresh();
    } else {
      const paths = Array.from(local.selectedPaths);
      if (paths.length !== 1) {
        window.alert("이름 변경은 항목 1개만 선택하세요.");
        return;
      }
      const oldPath = paths[0];
      const oldName = oldPath.replace(/[/\\]+$/, "").split(/[/\\]/).pop() ?? oldPath;
      const newName = window.prompt("새 이름을 입력하세요:", oldName);
      if (!newName || !newName.trim() || newName.trim() === oldName) return;
      await renameFile(oldPath, newName.trim());
      triggerLocalRefresh();
    }
  };

  const hasRemoteSelection = remote.selectedPaths.size > 0;
  const hasLocalSelection  = local.selectedPaths.size > 0;
  const hasSelection       = focusedSide === "remote" ? hasRemoteSelection : hasLocalSelection;

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "p") {
        event.preventDefault();
        openProfileModal();
      } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "r") {
        event.preventDefault();
        focusedSide === "remote" ? triggerRemoteRefresh() : triggerLocalRefresh();
      } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "d") {
        event.preventDefault();
        void handleDryRun();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  return (
    <div className={styles.toolbar}>
      <div className={styles.connectionArea}>
        <button className={styles.toolBtn} onClick={openProfileModal} title="프로필 관리 (Ctrl/Cmd+P)">
          <span className={styles.toolBtnIcon}>●</span>
          프로필
        </button>

        {activeProfile && !isConnected && (
          <button
            className={`${styles.toolBtn} ${styles.primary}`}
            disabled={isConnecting}
            onClick={() => connectWithProfile(activeProfile)}
            title="선택한 프로필로 S3 연결"
          >
            {isConnecting ? "연결 중..." : `${activeProfile.name} 연결`}
          </button>
        )}

        {isConnected && (
          <button className={styles.toolBtn} onClick={disconnect} title="현재 연결 해제">
            {activeProfile?.name} 연결됨
          </button>
        )}
      </div>

      <div className={styles.separator} />

      {/* 자동 Purge 체크 버튼 — 눈에 잘 띄는 위치 (요구사항 2.3) */}
      {isConnected && activeProfile?.cdnProvider && canPurge && (
        <button
          className={`${styles.toolBtn} ${autoPurgeEnabled ? styles.purgeActive : styles.purgeInactive}`}
          onClick={toggleAutoPurge}
          title={autoPurgeEnabled
            ? "자동 Purge 켜짐 — 업로드 후 자동으로 CDN 캐시를 무효화합니다. 클릭하면 끄기"
            : "자동 Purge 꺼짐 — 클릭하면 업로드 후 CDN 자동 Purge를 활성화합니다"}
        >
          <span className={styles.toolBtnIcon}>{autoPurgeEnabled ? "⚡" : "○"}</span>
          자동 Purge {autoPurgeEnabled ? "ON" : "OFF"}
        </button>
      )}

      <div className={styles.separator} />

      <div className={styles.actionArea}>
        {canCreate && (
          <button
            className={styles.toolBtn}
            disabled={focusedSide === "remote" && !isConnected}
            onClick={handleNewFolder}
            title="새 폴더 만들기"
          >
            새 폴더
          </button>
        )}
        <button
          className={styles.toolBtn}
          disabled={(focusedSide === "remote" && !isConnected) || !hasSelection}
          onClick={handleDelete}
          title="선택 항목 삭제"
        >
          삭제
        </button>
        <button
          className={styles.toolBtn}
          disabled={(focusedSide === "remote" && !isConnected) || !hasSelection}
          onClick={handleRename}
          title="선택 항목 이름 변경"
        >
          이름 변경
        </button>
        <button
          className={styles.toolBtn}
          disabled={!isConnected || !local.path}
          onClick={handleDryRun}
          title="업로드 전 변경 사항과 Purge 대상을 미리 봅니다 (Ctrl/Cmd+D)"
        >
          미리보기
        </button>

        {/* Phase 3: 수동 Purge 버튼 — CDN 설정된 경우에만 표시 */}
        {isConnected && activeProfile?.cdnProvider && canPurge && (
          <>
            <div className={styles.separator} />
            <button
              className={styles.toolBtn}
              disabled={remotePurgePaths.length === 0}
              onClick={() => setPurgeDialog({ paths: remotePurgePaths, mode: "selected" })}
              title="원격 패널에서 선택한 파일의 CDN 캐시를 무효화합니다"
            >
              선택 Purge
            </button>
            <button
              className={`${styles.toolBtn} ${styles.purgeInactive}`}
              onClick={() => setPurgeDialog({ paths: [allPrefix], mode: "all" })}
              title={`현재 원격 경로 전체 (${allPrefix})를 CDN에서 무효화합니다`}
            >
              전체 Purge
            </button>
          </>
        )}
      </div>

      <div className={styles.spacer} />

      <button className={styles.toolBtn} onClick={openSettingsModal} title="앱 설정">
        설정
      </button>

      {purgeDialog && (
        <PurgeDialog
          paths={purgeDialog.paths}
          mode={purgeDialog.mode}
          onConfirm={async () => {
            await executePurge(purgeDialog.paths);
            setPurgeDialog(null);
          }}
          onCancel={() => setPurgeDialog(null)}
        />
      )}
    </div>
  );
}
