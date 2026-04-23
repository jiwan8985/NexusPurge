import { useEffect } from "react";
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
  const isLogPanelVisible = useAppStore((s) => s.isLogPanelVisible);
  const showProgressDialog = useAppStore((s) => s.showProgressDialog);
  const isProfileModalOpen = useAppStore((s) => s.isProfileModalOpen);
  const { loadProfiles } = useProfile();

  useEffect(() => {
    loadProfiles();
  }, [loadProfiles]);

  return (
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
  );
}
