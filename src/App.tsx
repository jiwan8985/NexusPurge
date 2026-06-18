import { useEffect } from "react";
import ErrorBoundary from "./components/ErrorBoundary";
import TitleBar from "./components/layout/TitleBar";
import Toolbar from "./components/layout/Toolbar";
import StatusBar from "./components/layout/StatusBar";
import LocalPanel from "./components/panels/LocalPanel";
import RemotePanel from "./components/panels/RemotePanel";
import TransferButtons from "./components/transfer/TransferButtons";
import ProgressDialog from "./components/transfer/ProgressDialog";
import SyncPreviewDialog from "./components/sync/SyncPreviewDialog";
import LogPanel from "./components/log/LogPanel";
import ProfileModal from "./components/modals/ProfileModal";
import SettingsModal from "./components/modals/SettingsModal";
import { useAppStore } from "./store/appStore";
import { useProfile } from "./hooks/useProfile";
import { runtime } from "./services/runtime";

export default function App() {
  const isLogPanelVisible  = useAppStore((s) => s.isLogPanelVisible);
  const showProgressDialog = useAppStore((s) => s.showProgressDialog);
  const showSyncPreview    = useAppStore((s) => s.showSyncPreview);
  const isProfileModalOpen = useAppStore((s) => s.isProfileModalOpen);
  const isSettingsModalOpen = useAppStore((s) => s.isSettingsModalOpen);
  const setActiveProfile   = useAppStore((s) => s.setActiveProfile);
  const setLastProfileId   = useAppStore((s) => s.setLastProfileId);
  const setLogPanelVisible = useAppStore((s) => s.setLogPanelVisible);
  const theme              = useAppStore((s) => s.theme);
  const setTheme           = useAppStore((s) => s.setTheme);
  const { loadProfiles, profiles } = useProfile();

  // 앱 준비 후 윈도우 표시 (tauri.conf.json: visible: false)
  useEffect(() => {
    void runtime.showMainWindow();
  }, []);

  // localStorage에서 저장된 테마 복원
  useEffect(() => {
    const saved = window.localStorage.getItem("nexuspurge.theme") as "light" | "dark" | "system" | null;
    if (saved === "light" || saved === "dark" || saved === "system") {
      setTheme(saved);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // theme 변경 시 <html data-theme="..."> 적용 + 저장
  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") {
      root.removeAttribute("data-theme");
    } else {
      root.setAttribute("data-theme", theme);
    }
    window.localStorage.setItem("nexuspurge.theme", theme);
  }, [theme]);

  // 초기 프로파일 로드
  useEffect(() => {
    loadProfiles();
    const showLog = window.localStorage.getItem("nexuspurge.showLogOnStartup");
    if (showLog === "false") setLogPanelVisible(false);
  }, [loadProfiles, setLogPanelVisible]);

  // H-7: 프로파일 로드 후 마지막 연결 프로파일 복원 (연결은 하지 않음)
  useEffect(() => {
    if (profiles.length === 0) return;
    if (window.localStorage.getItem("nexuspurge.restoreLastProfile") === "false") return;
    runtime.invoke<string | null>("get_last_profile_id")
      .then((lastId) => {
        if (!lastId) return;
        const found = profiles.find((p) => p.id === lastId);
        if (found) {
          setActiveProfile(found);
          setLastProfileId(lastId);
        }
      })
      .catch(() => {/* 무시 */});
  }, [profiles, setActiveProfile, setLastProfileId]);

  return (
    <ErrorBoundary>
      <div className="app-container">
        <TitleBar />
        <Toolbar />

        <div className="main-content">
          <div className="panels-row">
            <LocalPanel />
            <TransferButtons />
            <RemotePanel />
          </div>

          {isLogPanelVisible && <LogPanel />}
        </div>

        <StatusBar />

        {showProgressDialog && <ProgressDialog />}
        {showSyncPreview && <SyncPreviewDialog />}
        {isProfileModalOpen && <ProfileModal />}
        {isSettingsModalOpen && <SettingsModal />}
      </div>
    </ErrorBoundary>
  );
}
