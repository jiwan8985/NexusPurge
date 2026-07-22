import { useEffect, useState } from "react";
import { useAppStore } from "../../store/appStore";
import { runtime } from "../../services/runtime";
import { readBatchSettings, writeBatchSetting, BATCH_DEFAULTS } from "../../utils/batch-settings";
import styles from "./SettingsModal.module.css";

interface AppSettings {
  lastProfileId: string | null;
  detailedAuditLog: boolean;
}

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

  const [batch, setBatch] = useState(() => readBatchSettings());

  const updateBatch = <K extends keyof typeof batch>(key: K, value: number) => {
    const next = { ...batch, [key]: value };
    setBatch(next);
    writeBatchSetting(key as Parameters<typeof writeBatchSetting>[0], value);
  };

  const resetBatch = () => {
    setBatch({ ...BATCH_DEFAULTS });
    (Object.keys(BATCH_DEFAULTS) as Array<keyof typeof BATCH_DEFAULTS>).forEach((k) =>
      writeBatchSetting(k, BATCH_DEFAULTS[k])
    );
  };

  const [restoreLastProfile, setRestoreLastProfile] = useState(() =>
    readPref("nexuspurge.restoreLastProfile", true)
  );
  const [showLogOnStartup, setShowLogOnStartup] = useState(() =>
    readPref("nexuspurge.showLogOnStartup", true)
  );

  const updateRestoreLastProfile = (checked: boolean) => {
    setRestoreLastProfile(checked);
    writePref("nexuspurge.restoreLastProfile", checked);
  };

  const updateShowLogOnStartup = (checked: boolean) => {
    setShowLogOnStartup(checked);
    writePref("nexuspurge.showLogOnStartup", checked);
  };

  // 감사 로그 상세 레벨 — Rust 로깅 레이어에 직접 반영되어야 하므로 localStorage가 아니라
  // 백엔드 settings.json(get_app_settings/save_detailed_audit_log)과 왕복한다.
  const [detailedAuditLog, setDetailedAuditLog] = useState(false);
  useEffect(() => {
    runtime
      .invoke<AppSettings>("get_app_settings")
      .then((settings) => setDetailedAuditLog(settings.detailedAuditLog))
      .catch((err) => console.error("[SettingsModal] 설정 조회 실패:", err));
  }, []);

  const updateDetailedAuditLog = (checked: boolean) => {
    setDetailedAuditLog(checked);
    runtime
      .invoke("save_detailed_audit_log", { enabled: checked })
      .catch((err) => console.error("[SettingsModal] 감사 로그 레벨 저장 실패:", err));
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
            <div className={styles.sectionTitle}>
              전송 성능
              <button type="button" className={styles.resetBtn} onClick={resetBatch}>기본값으로</button>
            </div>
            <div className={styles.numberGrid}>
              <label className={styles.numberRow}>
                <span>
                  <strong>동시 전송 수</strong>
                  <small>업로드/다운로드 병렬 처리 개수 (1~16)</small>
                </span>
                <input
                  type="number" min={1} max={16}
                  className={styles.numberInput}
                  value={batch.maxConcurrentTransfers}
                  onChange={(e) => updateBatch("maxConcurrentTransfers", parseInt(e.target.value) || BATCH_DEFAULTS.maxConcurrentTransfers)}
                />
              </label>
            </div>
          </section>

          <section className={styles.section}>
            <div className={styles.sectionTitle}>로그</div>
            <label className={styles.toggleRow}>
              <span>
                <strong>CDN API 상세 로그 (응답 본문 포함)</strong>
                <small>감사 로그(audit-*.log)에 CDN 응답 본문까지 기록합니다. 기본은 메서드·URL·상태코드·소요시간만 남기는 요약 모드입니다.</small>
              </span>
              <input
                type="checkbox"
                checked={detailedAuditLog}
                onChange={(e) => updateDetailedAuditLog(e.target.checked)}
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
