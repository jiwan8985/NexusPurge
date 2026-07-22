import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import type {
  S3Profile,
  FileItem,
  TransferItem,
  LogEntry,
  LogLevel,
  LogCategory,
  PanelState,
  SyncPlan,
  CdnProvider,
  NetworkStatsEvent,
} from "../types";

// C-2: 프로필 목록을 전역 상태로 관리 — 훅 인스턴스별 분리 방지

// ─── State Shape ─────────────────────────────────────────────────────────────

interface AppState {
  // Connection
  activeProfile: S3Profile | null;
  isConnected: boolean;
  isConnecting: boolean;

  // 멀티 CDN 프로필에서 현재 Purge 대상 CDN 목록 (툴바에서 다중 선택 — 동시에 여러 CDN Purge 가능)
  activeCdns: CdnProvider[];

  // Profiles (전역 공유 — C-2 fix)
  profiles: S3Profile[];

  // H-7: 마지막 프로파일 ID (앱 재시작 시 복원용)
  lastProfileId: string | null;

  // H-1: 현재 포커스된 패널
  focusedSide: "local" | "remote";

  // H-1: 패널 새로고침 트리거 (increment → panel useEffect 재실행)
  localRefreshKey: number;
  remoteRefreshKey: number;

  // Panels
  local: PanelState;
  remote: PanelState;

  // Transfer queue
  transfers: TransferItem[];
  isTransferring: boolean;
  showProgressDialog: boolean;

  // Sync plan (업로드 전 ETag 비교 결과 — 상태 배지 표시용)
  syncPlan: SyncPlan | null;

  // Network stats (Rust network:stats 이벤트 — 상태바 표시용)
  networkStats: NetworkStatsEvent;

  // Log
  logs: LogEntry[];
  isLogPanelVisible: boolean;

  // Modal
  isProfileModalOpen: boolean;
  isSettingsModalOpen: boolean;

  // Theme
  theme: "light" | "dark" | "system";

  // Auto-Purge (세션 전역 토글 — 프로필 기본값 위에서 재정의 가능)
  autoPurgeEnabled: boolean;

  // 패널 간 pointer 드래그 / OS 파일 드래그 상태 (over: 현재 커서가 올라간 패널)
  panelDrag: { source: "local" | "remote" | "os"; over: "local" | "remote" | null } | null;
  setPanelDrag: (drag: { source: "local" | "remote" | "os"; over: "local" | "remote" | null } | null) => void;

  // Actions — Profiles
  setProfiles: (profiles: S3Profile[]) => void;

  // Actions — H-7
  setLastProfileId: (id: string | null) => void;

  // Actions — H-1 panel focus & refresh
  setFocusedSide: (side: "local" | "remote") => void;
  triggerLocalRefresh: () => void;
  triggerRemoteRefresh: () => void;

  // Actions — Connection
  setActiveProfile: (profile: S3Profile | null) => void;
  setConnected: (connected: boolean) => void;
  setConnecting: (connecting: boolean) => void;
  setActiveCdns: (providers: CdnProvider[]) => void;
  toggleActiveCdn: (provider: CdnProvider) => void;

  // Actions — Local panel
  setLocalPath: (path: string) => void;
  setLocalFiles: (files: FileItem[]) => void;
  setLocalLoading: (loading: boolean) => void;
  toggleLocalSelection: (path: string) => void;
  clearLocalSelection: () => void;

  // Actions — Remote panel
  setRemotePath: (path: string) => void;
  setRemoteFiles: (files: FileItem[]) => void;
  setRemoteLoading: (loading: boolean) => void;
  toggleRemoteSelection: (path: string) => void;
  clearRemoteSelection: () => void;

  // Actions — Transfer
  addTransfer: (item: TransferItem) => void;
  updateTransfer: (id: string, patch: Partial<TransferItem>) => void;
  clearCompletedTransfers: () => void;
  clearFinishedTransfers: () => void;
  setTransferring: (transferring: boolean) => void;
  setShowProgressDialog: (show: boolean) => void;
  setSyncPlan: (plan: SyncPlan | null) => void;
  setNetworkStats: (stats: NetworkStatsEvent) => void;

  // Actions — Log
  addLog: (level: LogLevel, message: string, category?: LogCategory, metadata?: Record<string, unknown>) => void;
  clearLogs: () => void;
  setLogPanelVisible: (visible: boolean) => void;
  toggleLogPanel: () => void;

  // Actions — Theme
  setTheme: (theme: "light" | "dark" | "system") => void;
  cycleTheme: () => void;

  // Actions — Auto-Purge
  setAutoPurgeEnabled: (enabled: boolean) => void;
  toggleAutoPurge: () => void;

  // Actions — Modal
  openProfileModal: () => void;
  closeProfileModal: () => void;
  openSettingsModal: () => void;
  closeSettingsModal: () => void;
}

// ─── Initial Panel State ─────────────────────────────────────────────────────

const initialPanel = (path: string): PanelState => ({
  path,
  files: [],
  selectedPaths: new Set(),
  isLoading: false,
  sortKey: "name",
  sortAsc: true,
});

// ─── Store ───────────────────────────────────────────────────────────────────

export const useAppStore = create<AppState>()(
  subscribeWithSelector((set) => ({
    // ── Connection ────────────────────────────────────────────────────────────
    activeProfile: null,
    isConnected: false,
    isConnecting: false,
    activeCdns: [],

    // ── Profiles ──────────────────────────────────────────────────────────────
    profiles: [],

    // ── H-7 ──────────────────────────────────────────────────────────────────
    lastProfileId: null,

    // ── H-1 panel state ───────────────────────────────────────────────────────
    focusedSide: "local" as "local" | "remote",
    localRefreshKey: 0,
    remoteRefreshKey: 0,

    // ── Panels ────────────────────────────────────────────────────────────────
    local: initialPanel(""),
    remote: initialPanel("/"),

    // ── Transfer ─────────────────────────────────────────────────────────────
    transfers: [],
    isTransferring: false,
    showProgressDialog: false,
    syncPlan: null,
    networkStats: { avgRttMs: null, activeS3Calls: 0 },

    // ── Log ───────────────────────────────────────────────────────────────────
    logs: [],
    isLogPanelVisible: true,

    // ── Modal ─────────────────────────────────────────────────────────────────
    isProfileModalOpen: false,
    isSettingsModalOpen: false,

    // ── Theme ─────────────────────────────────────────────────────────────────
    theme: "system" as "light" | "dark" | "system",

    // ── Auto-Purge ────────────────────────────────────────────────────────────
    autoPurgeEnabled: window.localStorage.getItem("nexuspurge.autoPurgeEnabled") === "true",

    // ── Panel Drag ────────────────────────────────────────────────────────────
    panelDrag: null,

    // ── Profile Actions ───────────────────────────────────────────────────────
    setProfiles: (profiles) => set({ profiles }),

    // ── H-7 Actions ───────────────────────────────────────────────────────────
    setLastProfileId: (lastProfileId) => set({ lastProfileId }),

    // ── H-1 Panel Focus & Refresh ────────────────────────────────────────────
    setFocusedSide: (focusedSide) => set({ focusedSide }),
    triggerLocalRefresh: () => set((s) => ({ localRefreshKey: s.localRefreshKey + 1 })),
    triggerRemoteRefresh: () => set((s) => ({ remoteRefreshKey: s.remoteRefreshKey + 1 })),

    // ── Connection Actions ────────────────────────────────────────────────────
    setActiveProfile: (profile) => set({ activeProfile: profile }),
    setConnected: (connected) => set({ isConnected: connected }),
    setConnecting: (connecting) => set({ isConnecting: connecting }),
    setActiveCdns: (activeCdns) => set({ activeCdns }),
    toggleActiveCdn: (provider) =>
      set((s) => {
        const has = s.activeCdns.includes(provider);
        // 최소 1개는 선택 상태 유지 — 마지막 하나는 해제 불가(Purge 대상이 사라지는 것을 방지)
        if (has && s.activeCdns.length === 1) return s;
        return {
          activeCdns: has
            ? s.activeCdns.filter((c) => c !== provider)
            : [...s.activeCdns, provider],
        };
      }),

    // ── Local Panel Actions ───────────────────────────────────────────────────
    setLocalPath: (path) =>
      set((s) => ({ local: { ...s.local, path } })),
    setLocalFiles: (files) =>
      set((s) => ({ local: { ...s.local, files } })),
    setLocalLoading: (isLoading) =>
      set((s) => ({ local: { ...s.local, isLoading } })),
    toggleLocalSelection: (path) =>
      set((s) => {
        const next = new Set(s.local.selectedPaths);
        next.has(path) ? next.delete(path) : next.add(path);
        return { local: { ...s.local, selectedPaths: next } };
      }),
    clearLocalSelection: () =>
      set((s) => ({ local: { ...s.local, selectedPaths: new Set() } })),

    // ── Remote Panel Actions ──────────────────────────────────────────────────
    setRemotePath: (path) =>
      set((s) => ({ remote: { ...s.remote, path } })),
    setRemoteFiles: (files) =>
      set((s) => ({ remote: { ...s.remote, files } })),
    setRemoteLoading: (isLoading) =>
      set((s) => ({ remote: { ...s.remote, isLoading } })),
    toggleRemoteSelection: (path) =>
      set((s) => {
        const next = new Set(s.remote.selectedPaths);
        next.has(path) ? next.delete(path) : next.add(path);
        return { remote: { ...s.remote, selectedPaths: next } };
      }),
    clearRemoteSelection: () =>
      set((s) => ({ remote: { ...s.remote, selectedPaths: new Set() } })),

    // ── Transfer Actions ──────────────────────────────────────────────────────
    addTransfer: (item) =>
      set((s) => ({ transfers: [...s.transfers, item] })),
    updateTransfer: (id, patch) =>
      set((s) => ({
        transfers: s.transfers.map((t) => (t.id === id ? { ...t, ...patch } : t)),
      })),
    clearCompletedTransfers: () =>
      set((s) => ({
        transfers: s.transfers.filter(
          (t) => t.status !== "complete" && t.status !== "skipped"
            && t.status !== "canceled"
        ),
      })),
    // 새 전송 배치 시작 시 호출 — 이전 배치(완료/스킵/취소/오류)를 비워 진행률이
    // 배치 단위로 1부터 집계되게 한다 (진행 중인 항목만 유지)
    clearFinishedTransfers: () =>
      set((s) => ({
        transfers: s.transfers.filter(
          (t) => t.status === "pending" || t.status === "uploading"
            || t.status === "downloading" || t.status === "hashing"
            || t.status === "overwriting"
        ),
      })),
    setTransferring: (isTransferring) => set({ isTransferring }),
    setShowProgressDialog: (showProgressDialog) => set({ showProgressDialog }),
    setSyncPlan: (syncPlan) => set({ syncPlan }),
    setNetworkStats: (networkStats) => set({ networkStats }),

    // ── Log Actions ───────────────────────────────────────────────────────────
    addLog: (level, message, category, metadata) =>
      set((s) => ({
        logs: [
          ...s.logs,
          {
            id: crypto.randomUUID(),
            level,
            message,
            category,
            timestamp: new Date().toISOString(),
            metadata,
          },
        ].slice(-1000), // 최대 1000개 유지
      })),
    clearLogs: () => set({ logs: [] }),
    setLogPanelVisible: (isLogPanelVisible) => set({ isLogPanelVisible }),
    toggleLogPanel: () =>
      set((s) => ({ isLogPanelVisible: !s.isLogPanelVisible })),

    // ── Theme Actions ─────────────────────────────────────────────────────────
    setTheme: (theme) => set({ theme }),
    cycleTheme: () =>
      set((s) => ({
        theme:
          s.theme === "light" ? "dark"
          : s.theme === "dark" ? "system"
          : "light",
      })),

    // ── Auto-Purge Actions ────────────────────────────────────────────────────
    setAutoPurgeEnabled: (enabled) => {
      window.localStorage.setItem("nexuspurge.autoPurgeEnabled", String(enabled));
      set({ autoPurgeEnabled: enabled });
    },
    toggleAutoPurge: () => set((s) => {
      const next = !s.autoPurgeEnabled;
      window.localStorage.setItem("nexuspurge.autoPurgeEnabled", String(next));
      return { autoPurgeEnabled: next };
    }),

    // ── Panel Drag Actions ────────────────────────────────────────────────────
    setPanelDrag: (drag) => set({ panelDrag: drag }),

    // ── Modal Actions ─────────────────────────────────────────────────────────
    openProfileModal: () => set({ isProfileModalOpen: true }),
    closeProfileModal: () => set({ isProfileModalOpen: false }),
    openSettingsModal: () => set({ isSettingsModalOpen: true }),
    closeSettingsModal: () => set({ isSettingsModalOpen: false }),
  }))
);
