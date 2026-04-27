import { useAppStore } from "../../store/appStore";
import { useProfile } from "../../hooks/useProfile";
import { useS3 } from "../../hooks/useS3";
import { useLocalFs } from "../../hooks/useLocalFs";
import styles from "./Toolbar.module.css";

export default function Toolbar() {
  const {
    activeProfile,
    isConnected,
    isConnecting,
    openProfileModal,
    focusedSide,
    local,
    remote,
    triggerLocalRefresh,
    triggerRemoteRefresh,
  } = useAppStore((s) => ({
    activeProfile:        s.activeProfile,
    isConnected:          s.isConnected,
    isConnecting:         s.isConnecting,
    openProfileModal:     s.openProfileModal,
    focusedSide:          s.focusedSide,
    local:                s.local,
    remote:               s.remote,
    triggerLocalRefresh:  s.triggerLocalRefresh,
    triggerRemoteRefresh: s.triggerRemoteRefresh,
  }));

  const { disconnect, connectWithProfile } = useProfile();
  const { deleteObjects, createDirectory, renameObject } = useS3();
  const { createDir, deleteFiles, renameFile } = useLocalFs();

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

  // H-1: 삭제
  const handleDelete = async () => {
    if (focusedSide === "remote" && isConnected) {
      const keys = Array.from(remote.selectedPaths);
      if (keys.length === 0) return;
      if (!window.confirm(`S3에서 ${keys.length}개 항목을 삭제할까요?`)) return;
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

  return (
    <div className={styles.toolbar}>
      <div className={styles.connectionArea}>
        <button className={styles.toolBtn} onClick={openProfileModal} title="프로필 관리">
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

      <div className={styles.actionArea}>
        <button
          className={styles.toolBtn}
          disabled={focusedSide === "remote" && !isConnected}
          onClick={handleNewFolder}
          title="새 폴더 만들기"
        >
          새 폴더
        </button>
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
      </div>

      <div className={styles.spacer} />

      <button className={styles.toolBtn} title="설정">
        설정
      </button>
    </div>
  );
}
