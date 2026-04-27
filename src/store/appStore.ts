import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import type {
  S3Profile,
  FileItem,
  TransferItem,
  LogEntry,
  LogLevel,
  PanelState,
  SyncPlan,
} from "../types";

// ─── State Shape ─────────────────────────────────────────────────────────────

interface AppState {
  // Connection
  activeProfile: S3Profile | null;
  isConnected: boolean;
  isConnecting: boolean;

  // Panels
  local: PanelState;
  remote: PanelState;

  // Transfer queue
  transfers: TransferItem[];
  isTransferring: boolean;
  showProgressDialog: boolean;

  // Sync plan (업로드 전 ETag 비교 결과 — 상태 배지 표시용)
  syncPlan: SyncPlan | null;

  // Log
  logs: LogEntry[];
  isLogPanelVisible: boolean;

  // Modal
  isProfileModalOpen: boolean;

  // Actions — Connection
  setActiveProfile: (profile: S3Profile | null) => void;
  setConnected: (connected: boolean) => void;
  setConnecting: (connecting: boolean) => void;

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
  setTransferring: (transferring: boolean) => void;
  setShowProgressDialog: (show: boolean) => void;
  setSyncPlan: (plan: SyncPlan | null) => void;

  // Actions — Log
  addLog: (level: LogLevel, message: string, metadata?: Record<string, unknown>) => void;
  clearLogs: () => void;
  toggleLogPanel: () => void;

  // Actions — Modal
  openProfileModal: () => void;
  closeProfileModal: () => void;
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

    // ── Panels ────────────────────────────────────────────────────────────────
    local: initialPanel("C:\\"),
    remote: initialPanel("/"),

    // ── Transfer ─────────────────────────────────────────────────────────────
    transfers: [],
    isTransferring: false,
    showProgressDialog: false,
    syncPlan: null,

    // ── Log ───────────────────────────────────────────────────────────────────
    logs: [],
    isLogPanelVisible: true,

    // ── Modal ─────────────────────────────────────────────────────────────────
    isProfileModalOpen: false,

    // ── Connection Actions ────────────────────────────────────────────────────
    setActiveProfile: (profile) => set({ activeProfile: profile }),
    setConnected: (connected) => set({ isConnected: connected }),
    setConnecting: (connecting) => set({ isConnecting: connecting }),

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
        ),
      })),
    setTransferring: (isTransferring) => set({ isTransferring }),
    setShowProgressDialog: (showProgressDialog) => set({ showProgressDialog }),
    setSyncPlan: (syncPlan) => set({ syncPlan }),

    // ── Log Actions ───────────────────────────────────────────────────────────
    addLog: (level, message, metadata) =>
      set((s) => ({
        logs: [
          ...s.logs,
          {
            id: crypto.randomUUID(),
            level,
            message,
            timestamp: new Date().toISOString(),
            metadata,
          },
        ].slice(-1000), // 최대 1000개 유지
      })),
    clearLogs: () => set({ logs: [] }),
    toggleLogPanel: () =>
      set((s) => ({ isLogPanelVisible: !s.isLogPanelVisible })),

    // ── Modal Actions ─────────────────────────────────────────────────────────
    openProfileModal: () => set({ isProfileModalOpen: true }),
    closeProfileModal: () => set({ isProfileModalOpen: false }),
  }))
);
