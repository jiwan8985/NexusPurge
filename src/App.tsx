import { useEffect, useState } from "react";
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
  const [isMaximized, setIsMaximized] = useState(false);

  // maximized 상태 모니터링 (Windows에서 decorations: false 일 때 짤리는 현상 방지용)
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const checkMaximized = async () => {
      try {
        const max = await runtime.isWindowMaximized();
        if (active) setIsMaximized(max);

        // OS 창 전환 애니메이션 지연에 대응하기 위해 150ms 후 재확인
        setTimeout(async () => {
          if (!active) return;
          try {
            const secondCheck = await runtime.isWindowMaximized();
            setIsMaximized(secondCheck);
          } catch {}
        }, 150);
      } catch (err) {
        // 무시
      }
    };

    const setupListener = async () => {
      try {
        await checkMaximized();
        unlisten = await runtime.onWindowResize(() => {
          void checkMaximized();
        });
      } catch (err) {
        // 무시
      }
    };

    void setupListener();

    return () => {
      active = false;
      if (unlisten) unlisten();
    };
  }, []);

  // 앱 준비 후 윈도우 표시 (tauri.conf.json: visible: false)
  useEffect(() => {
    const showAndCheck = async () => {
      try {
        await runtime.showMainWindow();
        // 윈도우가 표시된 후 다양한 지연 시간으로 최대화 상태 재조회 (OS 복원 상태 반영)
        for (const delay of [100, 300, 600]) {
          setTimeout(async () => {
            try {
              const max = await runtime.isWindowMaximized();
              setIsMaximized(max);
            } catch {}
          }, delay);
        }
      } catch (err) {
        // 무시
      }
    };
    void showAndCheck();
  }, []);

  // 브라우저 뷰포트 강제 스크롤 방지 (input 포커스 등으로 인해 TitleBar/Toolbar/Modal이 화면 밖으로 스크롤되는 현상 차단)
  useEffect(() => {
    const handleScroll = (e?: Event) => {
      const target = e?.target as HTMLElement | null;
      
      // 스크롤 이벤트가 발생한 대상을 직접 리셋
      if (target && target.tagName) {
        if (target.scrollTop !== 0 || target.scrollLeft !== 0) {
          const className = typeof target.className === "string" ? target.className : "";
          // 레이아웃 전체를 감싸는 컨테이너 및 모달 외곽부만 스크롤 차단 (내부 리스트/폼 스크롤 영역은 유지)
          if (
            target.id === "root" ||
            target.tagName === "BODY" ||
            target.tagName === "HTML" ||
            className.includes("app-container") ||
            className.includes("overlay") ||
            (className.includes("modal") && !className.includes("Scroll") && !className.includes("scroll")) ||
            (className.includes("form") && !className.includes("formScroll") && !className.includes("formActions") && !className.includes("formHeader")) ||
            className.includes("profileList")
          ) {
            target.scrollTop = 0;
            target.scrollLeft = 0;
          }
        }
      }

      // 전체 레이아웃 및 모달 영역 강제 리셋 (e가 누락된 경우 등 대응)
      const rootEl = document.getElementById("root");
      if (rootEl && (rootEl.scrollTop !== 0 || rootEl.scrollLeft !== 0)) {
        rootEl.scrollTop = 0;
        rootEl.scrollLeft = 0;
      }
      const appContainer = document.querySelector(".app-container");
      if (appContainer && (appContainer.scrollTop !== 0 || appContainer.scrollLeft !== 0)) {
        appContainer.scrollTop = 0;
        appContainer.scrollLeft = 0;
      }

      // overlay 및 modal 들에 대한 리셋도 추가
      const overlays = document.querySelectorAll("[class*='overlay']");
      overlays.forEach((el) => {
        if (el.scrollTop !== 0 || el.scrollLeft !== 0) {
          el.scrollTop = 0;
          el.scrollLeft = 0;
        }
      });
      const modals = document.querySelectorAll("[class*='modal']");
      modals.forEach((el) => {
        const cls = el.className || "";
        if (!cls.includes("Scroll") && !cls.includes("scroll")) {
          if (el.scrollTop !== 0 || el.scrollLeft !== 0) {
            el.scrollTop = 0;
            el.scrollLeft = 0;
          }
        }
      });
      const forms = document.querySelectorAll("form");
      forms.forEach((el) => {
        const cls = el.className || "";
        if (!cls.includes("Scroll") && !cls.includes("scroll")) {
          if (el.scrollTop !== 0 || el.scrollLeft !== 0) {
            el.scrollTop = 0;
            el.scrollLeft = 0;
          }
        }
      });

      if (window.scrollY !== 0 || window.scrollX !== 0) {
        window.scrollTo(0, 0);
      }
    };

    // scroll 이벤트는 버블링되지 않으므로 capture: true 설정 필수
    window.addEventListener("scroll", handleScroll, { capture: true, passive: true });
    
    const handleFocusIn = () => {
      // 포커스 이동 시 강제로 전체 뷰포트 원점 회귀 트리거 (포커스 중심 스크롤 대응)
      setTimeout(() => handleScroll(), 0);
    };
    document.addEventListener("focusin", handleFocusIn, { passive: true });

    return () => {
      window.removeEventListener("scroll", handleScroll, { capture: true });
      document.removeEventListener("focusin", handleFocusIn);
    };
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

  const isWindows = typeof navigator !== "undefined" && (
    navigator.userAgent.includes("Windows") || 
    navigator.platform.includes("Win")
  );

  return (
    <ErrorBoundary>
      <div className={`app-container ${isMaximized && isWindows ? "maximized" : ""}`}>
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
