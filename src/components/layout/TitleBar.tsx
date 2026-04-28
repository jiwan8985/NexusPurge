import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAppStore } from "../../store/appStore";
import { useProfile } from "../../hooks/useProfile";
import { useS3 } from "../../hooks/useS3";
import ConfirmDialog from "../common/ConfirmDialog";
import styles from "./TitleBar.module.css";
import type { S3Profile } from "../../types";

export default function TitleBar() {
  const appWindow = getCurrentWindow();
  const { activeProfile, isConnected, isConnecting, isTransferring, openProfileModal } =
    useAppStore((s) => ({
      activeProfile: s.activeProfile,
      isConnected: s.isConnected,
      isConnecting: s.isConnecting,
      isTransferring: s.isTransferring,
      openProfileModal: s.openProfileModal,
    }));

  const { disconnect, connectWithProfile, profiles } = useProfile();
  const { listObjects } = useS3();
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [showCloseConfirm, setShowCloseConfirm] = useState(false);

  const handleClose = () => {
    if (isTransferring) {
      setShowCloseConfirm(true);
    } else {
      appWindow.close();
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
      </div>

      <div className={styles.center} data-tauri-drag-region="false">
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

      <div className={styles.controls}>
        <button className={styles.controlBtn} onClick={() => appWindow.minimize()} aria-label="최소화">
          <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor" /></svg>
        </button>
        <button className={styles.controlBtn} onClick={() => appWindow.toggleMaximize()} aria-label="최대화">
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
          onConfirm={() => appWindow.close()}
          onCancel={() => setShowCloseConfirm(false)}
        />
      )}
    </div>
  );
}
