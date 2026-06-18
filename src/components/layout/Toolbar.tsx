import { useEffect, useState } from "react";
import { useAppStore } from "../../store/appStore";
import { useProfile } from "../../hooks/useProfile";
import { useS3 } from "../../hooks/useS3";
import { useLocalFs } from "../../hooks/useLocalFs";
import { usePurge } from "../../hooks/usePurge";
import PurgeDialog from "../modals/PurgeDialog";
import PurgeResultDialog from "../modals/PurgeResultDialog";
import { runtime } from "../../services/runtime";
import type { PurgeExecutionResult, SyncPreviewResult } from "../../types";
import styles from "./Toolbar.module.css";

/* ── Inline SVG icon primitives ──────────────────────────────────────────── */
const ICON_PROPS = {
  width: "14", height: "14", viewBox: "0 0 16 16",
  fill: "none", stroke: "currentColor",
  strokeWidth: "1.5", strokeLinecap: "round" as const, strokeLinejoin: "round" as const,
};

function IconProfile() {
  return (
    <svg {...ICON_PROPS}>
      <circle cx="8" cy="5" r="3" />
      <path d="M2 14c0-3.3 2.7-6 6-6s6 2.7 6 6" />
    </svg>
  );
}

function IconLink() {
  return (
    <svg {...ICON_PROPS}>
      <path d="M10 6l-4 4" />
      <path d="M7 4l1-1a3 3 0 014.2 4.2l-2 2a3 3 0 01-4.2 0" />
      <path d="M9 12l-1 1a3 3 0 01-4.2-4.2l2-2a3 3 0 014.2 0" />
    </svg>
  );
}

function IconFolderPlus() {
  return (
    <svg {...ICON_PROPS}>
      <path d="M2 4a1 1 0 011-1h3.5L8 5h5a1 1 0 011 1v7a1 1 0 01-1 1H3a1 1 0 01-1-1V4z" />
      <path d="M8 8v4M6 10h4" />
    </svg>
  );
}

function IconTrash() {
  return (
    <svg {...ICON_PROPS}>
      <path d="M3 5h10M5 5V4a1 1 0 011-1h4a1 1 0 011 1v1M6 8v5M10 8v5M4 5l.7 8.3A1 1 0 005.7 14h4.6a1 1 0 001-.7L12 5" />
    </svg>
  );
}

function IconPen() {
  return (
    <svg {...ICON_PROPS}>
      <path d="M11 2.5a1.5 1.5 0 012.1 2.1L5 12.8l-3 .8.8-3L11 2.5z" />
    </svg>
  );
}

function IconEye() {
  return (
    <svg {...ICON_PROPS}>
      <path d="M1 8s3-5 7-5 7 5 7 5-3 5-7 5-7-5-7-5z" />
      <circle cx="8" cy="8" r="2" />
    </svg>
  );
}

function IconBolt() {
  return (
    <svg {...ICON_PROPS} fill="currentColor" stroke="none">
      <path d="M9 2L3 9h5l-1 5 6-7H8l1-5z" />
    </svg>
  );
}

function IconFlame() {
  return (
    <svg {...ICON_PROPS}>
      <path d="M8 2c0 3-3 4-3 7a3 3 0 006 0c0-2-1-3-1-4 0 0-1 1-1 2s-1 2-1 2c0-2 0-5-2-7z" />
      <path d="M8 13.5a1.5 1.5 0 000-3" />
    </svg>
  );
}

function IconGear() {
  return (
    <svg {...ICON_PROPS}>
      <circle cx="8" cy="8" r="2.5" />
      <path d="M8 2v1.5M8 12.5V14M2 8h1.5M12.5 8H14M3.8 3.8l1.1 1.1M11.1 11.1l1.1 1.1M3.8 12.2l1.1-1.1M11.1 4.9l1.1-1.1" />
    </svg>
  );
}

function IconToggleOn() {
  return (
    <svg {...ICON_PROPS} fill="currentColor" stroke="none">
      <rect x="1" y="5" width="14" height="6" rx="3" />
      <circle cx="11" cy="8" r="2.2" fill="white" />
    </svg>
  );
}

function IconToggleOff() {
  return (
    <svg {...ICON_PROPS}>
      <rect x="1" y="5" width="14" height="6" rx="3" />
      <circle cx="5" cy="8" r="2.2" fill="currentColor" stroke="none" />
    </svg>
  );
}

/* ── Toolbar ─────────────────────────────────────────────────────────────── */
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

  const perms    = activeProfile?.permissions;
  const canPurge = !perms || perms.canPurge;
  const canCreate = !perms || perms.canCreate;

  const { disconnect, connectWithProfile } = useProfile();
  const { deleteObjects, createDirectory, renameObject } = useS3();
  const { createDir, deleteFiles, renameFile } = useLocalFs();
  const { executePurge, isPurging, selectedPaths: remotePurgePaths, allPrefix } = usePurge();

  const [purgeDialog, setPurgeDialog] = useState<{ paths: string[]; mode: "selected" | "all" } | null>(null);
  const [purgeResult, setPurgeResult] = useState<PurgeExecutionResult | null>(null);

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
      addLog("info", `Dry-run: 신규 ${preview.new.length}, 수정 ${preview.modified.length}, Purge ${preview.purgeTargets.length}`);
    } catch (err) {
      addLog("error", `Dry-run 실패: ${err}`);
    }
  };

  const handleDelete = async () => {
    if (focusedSide === "remote" && isConnected) {
      const keys = Array.from(remote.selectedPaths);
      if (keys.length === 0) return;
      const purgeNotice = activeProfile?.cdnProvider ? "\n삭제 성공한 항목은 CDN 캐시도 Purge됩니다." : "";
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

  const handleRename = async () => {
    if (focusedSide === "remote" && isConnected) {
      const keys = Array.from(remote.selectedPaths);
      if (keys.length !== 1) { window.alert("이름 변경은 항목 1개만 선택하세요."); return; }
      const oldKey  = keys[0];
      const oldName = oldKey.replace(/\/$/, "").split("/").pop() ?? oldKey;
      const newName = window.prompt("새 이름을 입력하세요:", oldName);
      if (!newName || !newName.trim() || newName.trim() === oldName) return;
      const newKey = oldKey.replace(/[^/]*\/?$/, newName.trim() + (oldKey.endsWith("/") ? "/" : ""));
      await renameObject(oldKey, newKey);
      triggerRemoteRefresh();
    } else {
      const paths = Array.from(local.selectedPaths);
      if (paths.length !== 1) { window.alert("이름 변경은 항목 1개만 선택하세요."); return; }
      const oldPath = paths[0];
      const oldName = oldPath.replace(/[/\\]+$/, "").split(/[/\\]/).pop() ?? oldPath;
      const newName = window.prompt("새 이름을 입력하세요:", oldName);
      if (!newName || !newName.trim() || newName.trim() === oldName) return;
      await renameFile(oldPath, newName.trim());
      triggerLocalRefresh();
    }
  };

  const hasSelection = focusedSide === "remote"
    ? remote.selectedPaths.size > 0
    : local.selectedPaths.size > 0;

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "p") {
        e.preventDefault(); openProfileModal();
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "r") {
        e.preventDefault();
        focusedSide === "remote" ? triggerRemoteRefresh() : triggerLocalRefresh();
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "d") {
        e.preventDefault(); void handleDryRun();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  return (
    <div className={styles.toolbar}>
      {/* ── 연결 영역 ─────────────────────────────────────────────────────── */}
      <div className={styles.group}>
        <button className={styles.toolBtn} onClick={openProfileModal} title="프로필 관리 (Ctrl/Cmd+P)">
          <span className={styles.icon}><IconProfile /></span>
          프로필
        </button>

        {activeProfile && !isConnected && (
          <button
            className={`${styles.toolBtn} ${styles.primary}`}
            disabled={isConnecting}
            onClick={() => connectWithProfile(activeProfile)}
            title="선택한 프로필로 S3 연결"
          >
            <span className={styles.icon}><IconLink /></span>
            {isConnecting ? "연결 중..." : `${activeProfile.name} 연결`}
          </button>
        )}

        {isConnected && (
          <button className={`${styles.toolBtn} ${styles.connected}`} onClick={disconnect} title="현재 연결 해제">
            <span className={styles.connDotInline} />
            {activeProfile?.name}
          </button>
        )}
      </div>

      <div className={styles.sep} />

      {/* ── 자동 Purge 토글 ───────────────────────────────────────────────── */}
      {isConnected && activeProfile?.cdnProvider && canPurge && (
        <>
          <button
            className={`${styles.toolBtn} ${autoPurgeEnabled ? styles.purgeOn : styles.purgeOff}`}
            onClick={toggleAutoPurge}
            title={autoPurgeEnabled
              ? "자동 Purge 켜짐 — 업로드 후 CDN 캐시 자동 무효화. 클릭하면 끄기"
              : "자동 Purge 꺼짐 — 클릭하면 켜기"}
          >
            <span className={styles.icon}>
              {autoPurgeEnabled ? <IconToggleOn /> : <IconToggleOff />}
            </span>
            자동 Purge {autoPurgeEnabled ? "ON" : "OFF"}
          </button>
          <div className={styles.sep} />
        </>
      )}

      {/* ── 파일 작업 ──────────────────────────────────────────────────────── */}
      <div className={styles.group}>
        {canCreate && (
          <button
            className={styles.toolBtn}
            disabled={focusedSide === "remote" && !isConnected}
            onClick={handleNewFolder}
            title="새 폴더 만들기"
          >
            <span className={styles.icon}><IconFolderPlus /></span>
            새 폴더
          </button>
        )}
        <button
          className={styles.toolBtn}
          disabled={(focusedSide === "remote" && !isConnected) || !hasSelection}
          onClick={handleDelete}
          title="선택 항목 삭제"
        >
          <span className={styles.icon}><IconTrash /></span>
          삭제
        </button>
        <button
          className={styles.toolBtn}
          disabled={(focusedSide === "remote" && !isConnected) || !hasSelection}
          onClick={handleRename}
          title="선택 항목 이름 변경"
        >
          <span className={styles.icon}><IconPen /></span>
          이름 변경
        </button>
        <button
          className={styles.toolBtn}
          disabled={!isConnected || !local.path}
          onClick={handleDryRun}
          title="업로드 전 변경 사항과 Purge 대상을 미리 봅니다 (Ctrl/Cmd+D)"
        >
          <span className={styles.icon}><IconEye /></span>
          미리보기
        </button>
      </div>

      {/* ── 수동 Purge ─────────────────────────────────────────────────────── */}
      {isConnected && activeProfile?.cdnProvider && canPurge && (
        <>
          <div className={styles.sep} />
          <div className={styles.group}>
            <button
              className={styles.toolBtn}
              disabled={remotePurgePaths.length === 0 || isPurging}
              onClick={() => setPurgeDialog({ paths: remotePurgePaths, mode: "selected" })}
              title="선택한 파일의 CDN 캐시를 무효화합니다"
            >
              <span className={styles.icon}><IconBolt /></span>
              선택 Purge
            </button>
            <button
              className={`${styles.toolBtn} ${styles.purgeOff}`}
              disabled={isPurging}
              onClick={() => setPurgeDialog({ paths: [allPrefix], mode: "all" })}
              title={`현재 원격 경로 전체 (${allPrefix})를 CDN에서 무효화합니다`}
            >
              <span className={styles.icon}><IconFlame /></span>
              전체 Purge
            </button>
          </div>
        </>
      )}

      <div className={styles.spacer} />

      <button className={styles.toolBtn} onClick={openSettingsModal} title="앱 설정">
        <span className={styles.icon}><IconGear /></span>
        설정
      </button>

      {/* ── 다이얼로그 ──────────────────────────────────────────────────────── */}
      {purgeDialog && (
        <PurgeDialog
          paths={purgeDialog.paths}
          mode={purgeDialog.mode}
          onConfirm={async () => {
            const paths = purgeDialog.paths;
            setPurgeDialog(null);
            const result = await executePurge(paths);
            if (result) setPurgeResult(result);
          }}
          onCancel={() => setPurgeDialog(null)}
        />
      )}
      {purgeResult && (
        <PurgeResultDialog result={purgeResult} onClose={() => setPurgeResult(null)} />
      )}

      {isPurging && (
        <div className={styles.purgeChip}>
          <span className={styles.purgeChipDot} />
          CDN Purge 진행 중...
        </div>
      )}
    </div>
  );
}
