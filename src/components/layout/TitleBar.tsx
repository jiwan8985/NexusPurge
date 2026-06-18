import { useState } from "react";
import { useAppStore } from "../../store/appStore";
import { useProfile } from "../../hooks/useProfile";
import { useS3 } from "../../hooks/useS3";
import { runtime } from "../../services/runtime";
import ConfirmDialog from "../common/ConfirmDialog";
import styles from "./TitleBar.module.css";
import type { S3Profile } from "../../types";

function ThemeIcon({ theme }: { theme: "light" | "dark" | "system" }) {
  if (theme === "light") {
    return (
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="5" />
        <line x1="12" y1="1" x2="12" y2="3" />
        <line x1="12" y1="21" x2="12" y2="23" />
        <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
        <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
        <line x1="1" y1="12" x2="3" y2="12" />
        <line x1="21" y1="12" x2="23" y2="12" />
        <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
        <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
      </svg>
    );
  }
  if (theme === "dark") {
    return (
      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" stroke="none">
        <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
      </svg>
    );
  }
  // system
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
      <line x1="8" y1="21" x2="16" y2="21" />
      <line x1="12" y1="17" x2="12" y2="21" />
    </svg>
  );
}

export default function TitleBar() {
  const { activeProfile, isConnected, isConnecting, isTransferring, openProfileModal, theme, cycleTheme } =
    useAppStore((s) => ({
      activeProfile:   s.activeProfile,
      isConnected:     s.isConnected,
      isConnecting:    s.isConnecting,
      isTransferring:  s.isTransferring,
      openProfileModal:s.openProfileModal,
      theme:           s.theme,
      cycleTheme:      s.cycleTheme,
    }));

  const { disconnect, connectWithProfile, profiles } = useProfile();
  const { listObjects } = useS3();
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [showCloseConfirm, setShowCloseConfirm] = useState(false);

  const handleClose = () => {
    if (isTransferring) {
      setShowCloseConfirm(true);
    } else {
      void runtime.closeWindow();
    }
  };

  const handleConnect = async (profile: S3Profile) => {
    setDropdownOpen(false);
    await connectWithProfile(profile);
    await listObjects("");
  };

  const handleDisconnect = () => {
    disconnect();
    setDropdownOpen(false);
  };

  const connectionLabel = isConnecting
    ? "연결 중"
    : isConnected
      ? activeProfile?.name ?? "연결됨"
      : "프로필 선택";

  return (
    <div className={styles.titlebar} data-tauri-drag-region>
      <div className={styles.left} data-tauri-drag-region>
        <span className={styles.brandMark} aria-hidden>NP</span>
        <div className={styles.brandText}>
          <span className={styles.appName}>NexusPurge</span>
          <span className={styles.appSub}>S3 배포 운영 콘솔</span>
        </div>

        <div className={styles.leftSep} />

        <div className={styles.profileArea}>
          <button
            className={`${styles.profileBtn} ${isConnected ? styles.connected : ""}`}
            onClick={() => setDropdownOpen((open) => !open)}
            aria-expanded={dropdownOpen}
            aria-label="프로필 선택"
          >
            <span className={`${styles.connDot} ${isConnected ? styles.on : ""}`} />
            <span className={styles.profileLabel}>{connectionLabel}</span>
            {isConnected && activeProfile && (
              <span className={styles.bucketLabel}>{activeProfile.bucket}</span>
            )}
            <span className={styles.caret}>⌄</span>
          </button>

          {dropdownOpen && (
            <div className={styles.dropdown}>
              <div className={styles.ddHeader}>워크스페이스</div>
              {profiles.length === 0 ? (
                <div className={styles.ddEmpty}>저장된 프로필이 없습니다.</div>
              ) : (
                profiles.map((profile) => (
                  <button
                    key={profile.id}
                    className={`${styles.ddItem} ${
                      activeProfile?.id === profile.id && isConnected ? styles.ddActive : ""
                    }`}
                    onClick={() => handleConnect(profile)}
                  >
                    <span className={styles.ddName}>{profile.name}</span>
                    <span className={styles.ddMeta}>
                      {profile.bucket} · {profile.region}
                    </span>
                  </button>
                ))
              )}
              <div className={styles.ddDivider} />
              <button
                className={styles.ddItem}
                onClick={() => {
                  setDropdownOpen(false);
                  openProfileModal();
                }}
              >
                <span className={styles.ddName}>프로필 관리</span>
                <span className={styles.ddMeta}>S3, CDN, 엔드포인트 설정</span>
              </button>
              {isConnected && (
                <button className={`${styles.ddItem} ${styles.ddDanger}`} onClick={handleDisconnect}>
                  연결 해제
                </button>
              )}
            </div>
          )}
        </div>
      </div>

      <div className={styles.center} data-tauri-drag-region />

      <button
        className={styles.themeBtn}
        onClick={cycleTheme}
        aria-label={
          theme === "light" ? "라이트 모드 (클릭하면 다크)"
          : theme === "dark" ? "다크 모드 (클릭하면 시스템)"
          : "시스템 모드 (클릭하면 라이트)"
        }
        title={
          theme === "light" ? "라이트 모드"
          : theme === "dark" ? "다크 모드"
          : "시스템 따라가기"
        }
      >
        <ThemeIcon theme={theme} />
      </button>

      <div className={styles.controls}>
        <button className={styles.controlBtn} onClick={() => runtime.minimizeWindow()} aria-label="최소화">
          <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor" /></svg>
        </button>
        <button className={styles.controlBtn} onClick={() => runtime.toggleMaximizeWindow()} aria-label="최대화">
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
            <rect x="0.5" y="0.5" width="9" height="9" stroke="currentColor" />
          </svg>
        </button>
        <button className={`${styles.controlBtn} ${styles.closeBtn}`} onClick={handleClose} aria-label="닫기">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <path d="M1 1L9 9M9 1L1 9" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </button>
      </div>

      {dropdownOpen && <div className={styles.ddBackdrop} onClick={() => setDropdownOpen(false)} />}

      {showCloseConfirm && (
        <ConfirmDialog
          title="전송 중 종료"
          message="파일 전송이 진행 중입니다. 지금 종료하면 전송이 중단됩니다. 계속하시겠습니까?"
          confirmLabel="종료"
          danger
          onConfirm={() => runtime.closeWindow()}
          onCancel={() => setShowCloseConfirm(false)}
        />
      )}
    </div>
  );
}
