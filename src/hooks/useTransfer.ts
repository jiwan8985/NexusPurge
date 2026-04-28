import { useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "../store/appStore";
import type { TransferItem, SyncPlan, SyncResult } from "../types";

// Tauri 이벤트형: Rust 측에서 emit하는 전송 진행률 이벤트
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

// C-4: Windows 경로 구분자 혼용 방지 — local.path가 '\' 포함 시 '\' 사용
function joinPath(dir: string, fileName: string): string {
  const normalized = dir.replace(/[/\\]$/, "");
  const sep = normalized.includes("\\") ? "\\" : "/";
  return `${normalized}${sep}${fileName}`;
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
    setSyncPlan,
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
    setSyncPlan: s.setSyncPlan,
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
            // M-10: transfer / cdn 카테고리 분리
            addLog("success", `전송 완료: ${payload.id}`, "transfer");
            if (payload.cdnPurged) {
              addLog("success", `CDN Purge 완료: ${payload.id}`, "cdn");
            }
            if (payload.cdnPurgeError) {
              addLog("warn", `CDN Purge 실패: ${payload.cdnPurgeError}`, "cdn");
            }
          } else if (payload.status === "error") {
            addLog("error", `전송 실패 [${payload.id}]: ${payload.error}`, "transfer");
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
    // M-8: dialog는 실제 전송 항목이 있을 때만 열기

    const selectedPaths = Array.from(local.selectedPaths);
    addLog("info", `업로드 시작: ${selectedPaths.length}개 파일 선택됨`, "transfer");

    try {
      // 1. Smart Sync 플랜 생성 (ETag 비교)
      const plan = await buildSyncPlan(selectedPaths, remote.path);
      setSyncPlan(plan);
      addLog(
        "info",
        `업로드 계획: ${plan.toUpload.length}개 업로드, ${plan.toSkip.length}개 스킵, ${plan.toOverwrite.length}개 덮어쓰기`,
        "transfer"
      );

      // M-8: 업로드할 파일이 없으면 progress dialog 없이 종료
      if (plan.toUpload.length === 0 && plan.toOverwrite.length === 0) {
        const skipCount = plan.toSkip.length;
        addLog(
          "info",
          skipCount > 0
            ? `모든 파일이 최신 상태입니다. (${skipCount}개 건너뜀)`
            : "업로드할 파일이 없습니다.",
          "transfer"
        );
        setSyncPlan(null);
        return;
      }

      setShowProgressDialog(true);

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
      // C-1: toOverwrite 항목에만 isOverwrite: true — 신규 파일 CDN Purge 방지
      const makeItems = (files: typeof plan.toUpload, isOverwrite: boolean) =>
        files.map((file) => {
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
          return { id, localPath: file.path, remotePath: remote.path + file.name, isOverwrite };
        });

      const uploadItems = [
        ...makeItems(plan.toUpload, false),
        ...makeItems(plan.toOverwrite, true),
      ];

      await invoke("upload_files", {
        profileId: activeProfile.id,
        items: uploadItems,
        cdnDistributionId: activeProfile.cdnDistributionId,
        cdnProvider: activeProfile.cdnProvider,
      });

      clearLocalSelection();
      setSyncPlan(null);
    } catch (err) {
      addLog("error", `업로드 오류: ${err}`, "transfer");
      setSyncPlan(null);
    } finally {
      setTransferring(false);
    }
  }, [
    activeProfile, local, remote, addTransfer, buildSyncPlan,
    setTransferring, setShowProgressDialog, clearLocalSelection, addLog, setSyncPlan,
  ]);

  const startDownload = useCallback(async () => {
    if (!activeProfile || remote.selectedPaths.size === 0) return;

    // M-7: 다운로드 대상 폴더 선택 다이얼로그
    const selectedDir = await open({
      directory: true,
      multiple: false,
      defaultPath: local.path || undefined,
      title: "다운로드 폴더 선택",
    });

    // 사용자가 취소했을 때
    if (!selectedDir || typeof selectedDir !== "string") return;

    setTransferring(true);
    setShowProgressDialog(true);

    const selectedKeys = Array.from(remote.selectedPaths);
    addLog("info", `다운로드 시작: ${selectedKeys.length}개 파일`, "transfer");

    try {
      const downloadItems = selectedKeys.map((key) => {
        const id = crypto.randomUUID();
        const fileName = key.split("/").pop() ?? key;
        // C-4: joinPath로 OS별 경로 구분자 통일 (사용자 선택 폴더 기준)
        const localPath = joinPath(selectedDir, fileName);
        addTransfer({
          id,
          direction: "download",
          localPath,
          remotePath: key,
          fileName,
          size: 0,
          status: "pending",
          progress: 0,
          transferredBytes: 0,
          startedAt: new Date().toISOString(),
        });
        return { id, remotePath: key, localPath };
      });

      await invoke("start_downloads", {
        profileId: activeProfile.id,
        items: downloadItems,
      });

      clearRemoteSelection();
    } catch (err) {
      addLog("error", `다운로드 오류: ${err}`, "transfer");
    } finally {
      setTransferring(false);
    }
  }, [
    activeProfile, local, remote, addTransfer,
    setTransferring, setShowProgressDialog, clearRemoteSelection, addLog,
  ]);

  // L-1: 로컬 디렉터리 전체 ↔ S3 prefix 비교 (dry-run)
  const buildPreview = useCallback(async (): Promise<SyncResult> => {
    if (!activeProfile) throw new Error("Not connected");
    return invoke<SyncResult>("sync_preview", {
      profileId: activeProfile.id,
      localDir: local.path,
      remotePrefix: remote.path,
    });
  }, [activeProfile, local.path, remote.path]);

  return { startUpload, startDownload, buildSyncPlan, buildPreview };
}
