import { useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useAppStore } from "../store/appStore";
import type { TransferItem, SyncPlan } from "../types";

// Tauri イベント型: Rust 측에서 emit하는 전송 진행률 이벤트
interface TransferProgressEvent {
  id: string;
  progress: number;
  transferredBytes: number;
  speed: number;
  status: TransferItem["status"];
}

interface TransferCompleteEvent {
  id: string;
  status: TransferItem["status"];
  cdnPurged: boolean;
  cdnPurgeError?: string;
  error?: string;
}

export function useTransfer() {
  const {
    activeProfile,
    local,
    remote,
    addTransfer,
    updateTransfer,
    setTransferring,
    setShowProgressDialog,
    clearLocalSelection,
    clearRemoteSelection,
    addLog,
  } = useAppStore((s) => ({
    activeProfile: s.activeProfile,
    local: s.local,
    remote: s.remote,
    addTransfer: s.addTransfer,
    updateTransfer: s.updateTransfer,
    setTransferring: s.setTransferring,
    setShowProgressDialog: s.setShowProgressDialog,
    clearLocalSelection: s.clearLocalSelection,
    clearRemoteSelection: s.clearRemoteSelection,
    addLog: s.addLog,
  }));

  const unlistenRef = useRef<UnlistenFn[]>([]);

  // Rust 이벤트 리스너 등록
  useEffect(() => {
    const setupListeners = async () => {
      const unlistenProgress = await listen<TransferProgressEvent>(
        "transfer:progress",
        ({ payload }) => {
          updateTransfer(payload.id, {
            progress: payload.progress,
            transferredBytes: payload.transferredBytes,
            speed: payload.speed,
            status: payload.status,
          });
        }
      );

      const unlistenComplete = await listen<TransferCompleteEvent>(
        "transfer:complete",
        ({ payload }) => {
          updateTransfer(payload.id, {
            status: payload.status,
            progress: payload.status === "complete" ? 100 : undefined,
            cdnPurged: payload.cdnPurged,
            cdnPurgeError: payload.cdnPurgeError,
            error: payload.error,
            completedAt: new Date().toISOString(),
          });

          if (payload.status === "complete") {
            const purgeMsg = payload.cdnPurged ? " + CDN Purge 완료" : "";
            addLog("success", `전송 완료${purgeMsg}: ${payload.id}`);
          } else if (payload.status === "error") {
            addLog("error", `전송 실패 [${payload.id}]: ${payload.error}`);
          }
        }
      );

      unlistenRef.current = [unlistenProgress, unlistenComplete];
    };

    setupListeners();
    return () => unlistenRef.current.forEach((fn) => fn());
  }, [updateTransfer, addLog]);

  // Smart Sync: MD5 비교 후 업로드 플랜 생성
  const buildSyncPlan = useCallback(
    async (localPaths: string[], remotePrefix: string): Promise<SyncPlan> => {
      if (!activeProfile) throw new Error("Not connected");
      return invoke<SyncPlan>("build_sync_plan", {
        profileId: activeProfile.id,
        localPaths,
        remotePrefix,
      });
    },
    [activeProfile]
  );

  const startUpload = useCallback(async () => {
    if (!activeProfile || local.selectedPaths.size === 0) return;

    setTransferring(true);
    setShowProgressDialog(true);

    const selectedPaths = Array.from(local.selectedPaths);
    addLog("info", `업로드 시작: ${selectedPaths.length}개 파일`);

    try {
      // 1. Smart Sync 플랜 생성 (ETag 비교)
      const plan = await buildSyncPlan(selectedPaths, remote.path);
      addLog("info", `업로드 계획: ${plan.toUpload.length}개 업로드, ${plan.toSkip.length}개 스킵, ${plan.toOverwrite.length}개 덮어쓰기`);

      // 2. 스킵 항목 등록
      for (const file of plan.toSkip) {
        const id = crypto.randomUUID();
        addTransfer({
          id,
          direction: "upload",
          localPath: file.path,
          remotePath: remote.path + file.name,
          fileName: file.name,
          size: file.size,
          status: "skipped",
          progress: 100,
          transferredBytes: file.size,
          startedAt: new Date().toISOString(),
          completedAt: new Date().toISOString(),
        });
      }

      // 3. 업로드 대상 Rust에 전달 (병렬 처리)
      const uploadItems = [...plan.toUpload, ...plan.toOverwrite].map((file) => {
        const id = crypto.randomUUID();
        addTransfer({
          id,
          direction: "upload",
          localPath: file.path,
          remotePath: remote.path + file.name,
          fileName: file.name,
          size: file.size,
          status: "pending",
          progress: 0,
          transferredBytes: 0,
          startedAt: new Date().toISOString(),
        });
        return { id, localPath: file.path, remotePath: remote.path + file.name };
      });

      if (uploadItems.length > 0) {
        await invoke("start_uploads", {
          profileId: activeProfile.id,
          items: uploadItems,
          cdnDistributionId: activeProfile.cdnDistributionId,
          cdnProvider: activeProfile.cdnProvider,
        });
      }

      clearLocalSelection();
    } catch (err) {
      addLog("error", `업로드 오류: ${err}`);
    } finally {
      setTransferring(false);
    }
  }, [
    activeProfile, local, remote, addTransfer, buildSyncPlan,
    setTransferring, setShowProgressDialog, clearLocalSelection, addLog,
  ]);

  const startDownload = useCallback(async () => {
    if (!activeProfile || remote.selectedPaths.size === 0) return;

    setTransferring(true);
    setShowProgressDialog(true);

    const selectedKeys = Array.from(remote.selectedPaths);
    addLog("info", `다운로드 시작: ${selectedKeys.length}개 파일`);

    try {
      const downloadItems = selectedKeys.map((key) => {
        const id = crypto.randomUUID();
        const fileName = key.split("/").pop() ?? key;
        addTransfer({
          id,
          direction: "download",
          localPath: local.path + "/" + fileName,
          remotePath: key,
          fileName,
          size: 0,
          status: "pending",
          progress: 0,
          transferredBytes: 0,
          startedAt: new Date().toISOString(),
        });
        return { id, remotePath: key, localPath: local.path + "/" + fileName };
      });

      await invoke("start_downloads", {
        profileId: activeProfile.id,
        items: downloadItems,
      });

      clearRemoteSelection();
    } catch (err) {
      addLog("error", `다운로드 오류: ${err}`);
    } finally {
      setTransferring(false);
    }
  }, [
    activeProfile, local, remote, addTransfer,
    setTransferring, setShowProgressDialog, clearRemoteSelection, addLog,
  ]);

  return { startUpload, startDownload, buildSyncPlan };
}
