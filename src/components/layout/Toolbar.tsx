import { useAppStore } from "../../store/appStore";
import { useProfile } from "../../hooks/useProfile";
import styles from "./Toolbar.module.css";

export default function Toolbar() {
  const { activeProfile, isConnected, isConnecting, openProfileModal } = useAppStore((s) => ({
    activeProfile: s.activeProfile,
    isConnected: s.isConnected,
    isConnecting: s.isConnecting,
    openProfileModal: s.openProfileModal,
  }));
  const { disconnect, connectWithProfile } = useProfile();

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
        <button className={styles.toolBtn} disabled={!isConnected} title="새 S3 폴더 만들기">
          새 폴더
        </button>
        <button className={styles.toolBtn} disabled={!isConnected} title="선택 항목 삭제">
          삭제
        </button>
        <button className={styles.toolBtn} disabled={!isConnected} title="선택 항목 이름 변경">
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
