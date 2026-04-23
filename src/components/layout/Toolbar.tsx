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
  const { disconnect } = useProfile();

  return (
    <div className={styles.toolbar}>
      {/* 프로파일 선택 / 연결 영역 */}
      <div className={styles.connectionArea}>
        <button
          className={styles.toolBtn}
          onClick={openProfileModal}
          title="프로파일 관리"
        >
          🔌 프로파일
        </button>

        {activeProfile && !isConnected && (
          <button
            className={`${styles.toolBtn} ${styles.primary}`}
            disabled={isConnecting}
            title="S3 연결"
          >
            {isConnecting ? "연결 중..." : `연결: ${activeProfile.name}`}
          </button>
        )}

        {isConnected && (
          <button
            className={styles.toolBtn}
            onClick={disconnect}
            title="연결 끊기"
          >
            ✕ {activeProfile?.name}
          </button>
        )}
      </div>

      <div className={styles.separator} />

      {/* 파일 작업 버튼 */}
      <div className={styles.actionArea}>
        <button className={styles.toolBtn} disabled={!isConnected} title="새 폴더">
          📁 새 폴더
        </button>
        <button className={styles.toolBtn} disabled={!isConnected} title="삭제">
          🗑 삭제
        </button>
        <button className={styles.toolBtn} disabled={!isConnected} title="이름 변경">
          ✏ 이름 변경
        </button>
      </div>

      <div className={styles.spacer} />

      {/* 설정 */}
      <button className={styles.toolBtn} title="설정">
        ⚙
      </button>
    </div>
  );
}
