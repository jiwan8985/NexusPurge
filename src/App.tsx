import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ErrorBoundary from "./components/ErrorBoundary";
import TitleBar from "./components/layout/TitleBar";
import Toolbar from "./components/layout/Toolbar";
import StatusBar from "./components/layout/StatusBar";
import LocalPanel from "./components/panels/LocalPanel";
import RemotePanel from "./components/panels/RemotePanel";
import TransferButtons from "./components/transfer/TransferButtons";
import ProgressDialog from "./components/transfer/ProgressDialog";
import LogPanel from "./components/log/LogPanel";
import ProfileModal from "./components/modals/ProfileModal";
import { useAppStore } from "./store/appStore";
import { useProfile } from "./hooks/useProfile";

export default function App() {
  const isLogPanelVisible  = useAppStore((s) => s.isLogPanelVisible);
  const showProgressDialog = useAppStore((s) => s.showProgressDialog);
  const isProfileModalOpen = useAppStore((s) => s.isProfileModalOpen);
  const setActiveProfile   = useAppStore((s) => s.setActiveProfile);
  const setLastProfileId   = useAppStore((s) => s.setLastProfileId);
  const { loadProfiles, profiles } = useProfile();

  // 앱 준비 후 윈도우 표시 (tauri.conf.json: visible: false)
  useEffect(() => {
    getCurrentWindow().show();
  }, []);

  // 초기 프로파일 로드
  useEffect(() => {
    loadProfiles();
  }, [loadProfiles]);

  // H-7: 프로파일 로드 후 마지막 연결 프로파일 복원 (연결은 하지 않음)
  useEffect(() => {
    if (profiles.length === 0) return;
    invoke<string | null>("get_last_profile_id")
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
        {isProfileModalOpen && <ProfileModal />}
      </div>
    </ErrorBoundary>
  );
}
