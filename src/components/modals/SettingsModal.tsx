import { useState } from "react";
import { useAppStore } from "../../store/appStore";
import styles from "./SettingsModal.module.css";

const readPref = (key: string, fallback: boolean) => {
  const value = window.localStorage.getItem(key);
  return value === null ? fallback : value === "true";
};

const writePref = (key: string, value: boolean) => {
  window.localStorage.setItem(key, String(value));
};

export default function SettingsModal() {
  const {
    isLogPanelVisible,
    closeSettingsModal,
    openProfileModal,
    setLogPanelVisible,
    clearCompletedTransfers,
    clearLogs,
    addLog,
  } = useAppStore((s) => ({
    isLogPanelVisible: s.isLogPanelVisible,
    closeSettingsModal: s.closeSettingsModal,
    openProfileModal: s.openProfileModal,
    setLogPanelVisible: s.setLogPanelVisible,
    clearCompletedTransfers: s.clearCompletedTransfers,
    clearLogs: s.clearLogs,
    addLog: s.addLog,
  }));

  const [restoreLastProfile, setRestoreLastProfile] = useState(() =>
    readPref("nexuspurge.restoreLastProfile", true)
  );
  const [showLogOnStartup, setShowLogOnStartup] = useState(() =>
    readPref("nexuspurge.showLogOnStartup", true)
  );
  const [confirmExternalRequests, setConfirmExternalRequests] = useState(() =>
    readPref("nexuspurge.confirmExternalRequests", true)
  );

  const updateRestoreLastProfile = (checked: boolean) => {
    setRestoreLastProfile(checked);
    writePref("nexuspurge.restoreLastProfile", checked);
  };

  const updateShowLogOnStartup = (checked: boolean) => {
    setShowLogOnStartup(checked);
    writePref("nexuspurge.showLogOnStartup", checked);
  };

  const updateConfirmExternalRequests = (checked: boolean) => {
    setConfirmExternalRequests(checked);
    writePref("nexuspurge.confirmExternalRequests", checked);
  };

  const handleOpenProfiles = () => {
    closeSettingsModal();
    openProfileModal();
  };

  const handleClearCompleted = () => {
    clearCompletedTransfers();
    addLog("info", "완료된 전송 기록을 정리했습니다.", "system");
  };

  const handleClearLogs = () => {
    clearLogs();
    addLog("info", "작업 로그를 지웠습니다.", "system");
  };

  return (
    <div className={styles.overlay} onClick={(e) => e.target === e.currentTarget && closeSettingsModal()}>
      <div className={styles.modal} role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <div className={styles.header}>
          <span id="settings-title" className={styles.title}>설정</span>
          <button type="button" className={styles.closeBtn} onClick={closeSettingsModal} aria-label="닫기">
            ✕
          </button>
        </div>

        <div className={styles.content}>
          <section className={styles.section}>
            <div className={styles.sectionTitle}>시작 동작</div>
            <label className={styles.toggleRow}>
              <span>
                <strong>마지막 프로필 선택 복원</strong>
                <small>앱 시작 시 마지막으로 연결했던 프로필을 선택 상태로 둡니다.</small>
              </span>
              <input
                type="checkbox"
                checked={restoreLastProfile}
                onChange={(e) => updateRestoreLastProfile(e.target.checked)}
              />
            </label>
            <label className={styles.toggleRow}>
              <span>
                <strong>시작 시 로그 패널 표시</strong>
                <small>다음 실행부터 하단 작업 로그 패널을 기본으로 표시합니다.</small>
              </span>
              <input
                type="checkbox"
                checked={showLogOnStartup}
                onChange={(e) => updateShowLogOnStartup(e.target.checked)}
              />
            </label>
          </section>

          <section className={styles.section}>
            <div className={styles.sectionTitle}>안전 확인</div>
            <label className={styles.toggleRow}>
              <span>
                <strong>실제 Provider 테스트 전 확인</strong>
                <small>AWS, S3-compatible, CDN API 테스트 전에 확인 창을 표시합니다.</small>
              </span>
              <input
                type="checkbox"
                checked={confirmExternalRequests}
                onChange={(e) => updateConfirmExternalRequests(e.target.checked)}
              />
            </label>
          </section>

          <section className={styles.section}>
            <div className={styles.sectionTitle}>워크스페이스</div>
            <label className={styles.toggleRow}>
              <span>
                <strong>현재 로그 패널 표시</strong>
                <small>현재 화면에서 하단 로그 패널을 표시하거나 숨깁니다.</small>
              </span>
              <input
                type="checkbox"
                checked={isLogPanelVisible}
                onChange={(e) => setLogPanelVisible(e.target.checked)}
              />
            </label>
            <div className={styles.buttonGrid}>
              <button type="button" className={styles.actionBtn} onClick={handleOpenProfiles}>
                프로필 관리
              </button>
              <button type="button" className={styles.actionBtn} onClick={handleClearCompleted}>
                완료 전송 정리
              </button>
              <button type="button" className={styles.actionBtn} onClick={handleClearLogs}>
                로그 지우기
              </button>
            </div>
          </section>
        </div>

        <div className={styles.footer}>
          <button type="button" className={styles.doneBtn} onClick={closeSettingsModal}>
            닫기
          </button>
        </div>
      </div>
    </div>
  );
}
