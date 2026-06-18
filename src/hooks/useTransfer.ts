import { useCallback, useEffect, useRef } from "react";
import { saveOperationLog } from "../services/operation-log/operation-log-service";
import { runtime } from "../services/runtime";
import { useAppStore } from "../store/appStore";
import { buildCdnUrl, defaultCacheControlFor } from "../utils/cdn";
import type { CdnPurgeResult, CdnUrlCheck, TransferItem, SyncPlan, SyncPreviewResult } from "../types";
import type { UploadOptions } from "../components/transfer/UploadOptionsModal";
import { readBatchSettings } from "../utils/batch-settings";

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
  cdnInvalidationId?: string;
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
    triggerRemoteRefresh,
    triggerLocalRefresh,
    addLog,
    setSyncPlan,
    autoPurgeEnabled,
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
    triggerRemoteRefresh: s.triggerRemoteRefresh,
    triggerLocalRefresh: s.triggerLocalRefresh,
    addLog: s.addLog,
    setSyncPlan: s.setSyncPlan,
    autoPurgeEnabled: s.autoPurgeEnabled,
  }));

  const unlistenRef = useRef<Array<() => void>>([]);

  // Rust 이벤트 리스너 등록
  useEffect(() => {
    const setupListeners = async () => {
      const unlistenProgress = await runtime.listen<TransferProgressEvent>(
        "transfer:progress",
        (payload) => {
          updateTransfer(payload.id, {
            progress: payload.progress,
            transferredBytes: payload.transferredBytes,
            speed: payload.speed,
            status: payload.status,
          });
        }
      );

      const unlistenComplete = await runtime.listen<TransferCompleteEvent>(
        "transfer:complete",
        (payload) => {
          updateTransfer(payload.id, {
            status: payload.status,
            progress: payload.status === "complete" ? 100 : undefined,
            cdnPurged: payload.cdnPurged,
            cdnPurgeError: payload.cdnPurgeError,
            cdnInvalidationId: payload.cdnInvalidationId,
            cdnPurgeStatus: payload.cdnPurgeError
              ? "error"
              : payload.cdnPurged
                ? payload.cdnInvalidationId
                  ? "inProgress"
                  : "complete"
                : "notRequested",
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

            const state = useAppStore.getState();
            const transfer = state.transfers.find((item) => item.id === payload.id);
            const profile = state.activeProfile;
            if (profile?.cdnProvider && transfer?.direction === "upload") {
              runtime.invoke<CdnUrlCheck[]>("verify_cdn_urls", {
                profileId: profile.id,
                paths: [transfer.remotePath],
              })
                .then((checks) => {
                  const check = checks[0];
                  if (check?.ok) {
                    const isRestricted = check.statusCode === 403;
                    updateTransfer(payload.id, {
                      cdnVerified: true,
                      cdnStatusCode: check.statusCode,
                      cdnCheckError: undefined,
                    });
                    state.addLog(
                      "info",
                      isRestricted
                        ? `CDN 반영 확인 (접근 제한 403): ${transfer.remotePath}`
                        : `CDN 반영 확인: ${transfer.remotePath} (${check.statusCode})`,
                      "cdn"
                    );
                  } else {
                    updateTransfer(payload.id, {
                      cdnVerified: false,
                      cdnStatusCode: check?.statusCode,
                      cdnCheckError: check?.error ?? "응답 없음",
                    });
                    state.addLog(
                      "warn",
                      `CDN 반영 미확인: ${transfer.remotePath} (${check?.error ?? check?.statusCode ?? "응답 없음"})`,
                      "cdn"
                    );
                  }
                })
                .catch((err) => state.addLog("warn", `CDN 반영 확인 실패: ${err}`, "cdn"));

              if (payload.cdnInvalidationId && profile.cdnProvider === "cloudfront") {
                runtime.invoke<{ success: boolean; status?: string; error?: string }>("get_purge_status", {
                  profileId: profile.id,
                  provider: profile.cdnProvider,
                  distributionId: profile.cdnDistributionId ?? "",
                  invalidationId: payload.cdnInvalidationId,
                })
                  .then((status) => {
                    updateTransfer(payload.id, {
                      cdnPurgeStatus: status.status === "Completed" ? "complete" : "inProgress",
                      cdnPurgeError: status.success ? undefined : status.error,
                    });
                  })
                  .catch((err) => updateTransfer(payload.id, {
                    cdnPurgeStatus: "error",
                    cdnPurgeError: String(err),
                  }));
              }
            }
          } else if (payload.status === "canceled") {
            addLog("warn", `전송 취소: ${payload.id}`, "transfer");
          } else if (payload.status === "error") {
            addLog("error", `전송 실패 [${payload.id}]: ${payload.error}`, "transfer");
          }
        }
      );

      unlistenRef.current = [unlistenProgress, unlistenComplete];
    };

    setupListeners().catch((err) =>
      console.error("[useTransfer] 이벤트 리스너 등록 실패:", err)
    );
    return () => unlistenRef.current.forEach((fn) => fn());
  }, [updateTransfer, addLog]);

  // Smart Sync: MD5 비교 후 업로드 플랜 생성
  const buildSyncPlan = useCallback(
    async (localPaths: string[], remotePrefix: string): Promise<SyncPlan> => {
      if (!activeProfile) throw new Error("Not connected");
      return runtime.invoke<SyncPlan>("build_sync_plan", {
        profileId: activeProfile.id,
        localPaths,
        remotePrefix,
      });
    },
    [activeProfile]
  );

  const startUpload = useCallback(async (uploadOptions?: UploadOptions) => {
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

      // 2. 스킵 항목 등록 (autoPurgeEnabled ON이면 나중에 Purge 예정으로 표시)
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
          cdnPurgeStatus:
            autoPurgeEnabled && activeProfile.cdnProvider ? "pending" : "notRequested",
          startedAt: new Date().toISOString(),
          completedAt: new Date().toISOString(),
        });
      }

      // 3. 업로드 대상 Rust에 전달 (병렬 처리)
      // file.name은 폴더 포함 상대경로 ("folder/sub/file.txt" 또는 "file.txt")
      const makeItems = (files: typeof plan.toUpload, isOverwrite: boolean) =>
        files.map((file) => {
          const id = crypto.randomUUID();
          // file.name이 "folder/sub/file.txt"처럼 상대경로를 포함하므로 그대로 연결
          const remotePath = remote.path + file.name;
          const cdnUrl = buildCdnUrl(activeProfile.cdnDomain, remotePath, activeProfile.cdnBasePath) ?? undefined;
          const cacheControl =
            activeProfile.defaultCacheControl || defaultCacheControlFor(remotePath) || undefined;
          addTransfer({
            id,
            direction: "upload",
            localPath: file.path,
            remotePath,
            fileName: file.name,
            size: file.size,
            status: "pending",
            progress: 0,
            transferredBytes: 0,
            cdnPurgeStatus: activeProfile.cdnProvider && isOverwrite ? "pending" : "notRequested",
            cdnUrl,
            startedAt: new Date().toISOString(),
          });
          const extraHeaders = uploadOptions?.headers
            ? Object.fromEntries(uploadOptions.headers.filter((h) => h.key).map((h) => [h.key, h.value]))
            : {};
          const extraMetadata = uploadOptions?.metadata
            ? Object.fromEntries(uploadOptions.metadata.filter((m) => m.key).map((m) => [m.key, m.value]))
            : {};
          return {
            id,
            localPath: file.path,
            remotePath,
            isOverwrite,
            contentTypeOverride: uploadOptions?.contentTypeOverride || activeProfile.contentTypeOverride,
            cacheControl: uploadOptions?.cacheControl || cacheControl,
            headers: extraHeaders,
            metadata: extraMetadata,
          };
        });

      // CDN이 설정된 경우 신규 파일도 항상 Purge:
      // 해당 경로에 CDN이 404를 캐싱하고 있을 수 있으므로 업로드 즉시 무효화 필요
      const hasCdn = !!activeProfile.cdnProvider;
      const uploadItems = [
        ...makeItems(plan.toUpload, hasCdn),       // 신규: CDN 설정 시 항상 Purge
        ...makeItems(plan.toOverwrite, true),       // 변경: 항상 Purge
      ];

      await runtime.invoke("upload_files", {
        profileId: activeProfile.id,
        items: uploadItems,
        cdnDistributionId: activeProfile.cdnDistributionId,
        cdnProvider: activeProfile.cdnProvider,
        maxConcurrentFiles: readBatchSettings().maxConcurrentTransfers,
      });
      const finishedAt = new Date().toISOString();
      const transferState = useAppStore.getState();
      const uploadTransfers = uploadItems
        .map((item) => transferState.transfers.find((transfer) => transfer.id === item.id))
        .filter((transfer): transfer is TransferItem => Boolean(transfer));
      const uploadStatus = summarizeTransferStatus(uploadTransfers);
      const uploadLog = {
        id: crypto.randomUUID(),
        profileId: activeProfile.id,
        operation: "upload" as const,
        status: uploadStatus,
        bucket: activeProfile.bucket,
        prefix: remote.path,
        files: uploadTransfers.map((item) => ({
          path: item.remotePath,
          operation: "upload" as const,
          status: item.status === "complete" ? "success" as const : item.status === "skipped" ? "success" as const : "failed" as const,
          error: item.error,
          startedAt: item.startedAt ?? finishedAt,
          finishedAt: item.completedAt ?? finishedAt,
        })),
        purgeResults: uploadTransfers
          .filter((item) => item.cdnPurged || item.cdnPurgeError)
          .map((item) => ({
            provider: activeProfile.cdnProvider!,
            urls: [item.remotePath],
            status: item.cdnPurgeError ? "failed" as const : "success" as const,
            requestId: item.cdnInvalidationId,
            error: item.cdnPurgeError,
            startedAt: item.startedAt ?? finishedAt,
            finishedAt: item.completedAt ?? finishedAt,
          })),
        startedAt: finishedAt,
        finishedAt,
      };
      void saveOperationLog(uploadLog);

      // autoPurgeEnabled ON: 스킵된 파일(변경 없음)도 포함해 선택한 전체 경로 Purge
      // 이유: CDN 캐시가 S3와 어긋난 경우(이전 Purge 실패, CDN 장애 등)를 커버
      if (autoPurgeEnabled && activeProfile.cdnProvider && plan.toSkip.length > 0) {
        const skipPaths = plan.toSkip.map((f) => remote.path + f.name);
        addLog(
          "info",
          `자동 Purge (스킵 포함): 미변경 ${skipPaths.length}개 경로 추가 Purge`,
          "cdn"
        );
        const { purgeBatchSize } = readBatchSettings();
        for (let i = 0; i < skipPaths.length; i += purgeBatchSize) {
          const batch = skipPaths.slice(i, i + purgeBatchSize);
          const batchLabel =
            skipPaths.length > purgeBatchSize
              ? ` (배치 ${Math.floor(i / purgeBatchSize) + 1}/${Math.ceil(skipPaths.length / purgeBatchSize)})`
              : "";
          try {
            const result = await runtime.invoke<CdnPurgeResult>("purge_cdn", {
              profileId: activeProfile.id,
              provider: activeProfile.cdnProvider,
              distributionId: activeProfile.cdnDistributionId ?? "",
              paths: batch,
            });
            if (result.success) {
              addLog("success", `스킵 경로 Purge 완료${batchLabel}: ${batch.length}개`, "cdn");
            } else {
              addLog("error", `스킵 경로 Purge 실패${batchLabel}: ${result.error}`, "cdn");
            }
          } catch (err) {
            addLog("error", `스킵 경로 Purge 오류${batchLabel}: ${err}`, "cdn");
          }
        }
      }

      clearLocalSelection();
      setSyncPlan(null);
      triggerRemoteRefresh(); // 업로드 완료 후 원격 패널 자동 갱신
    } catch (err) {
      addLog("error", `업로드 오류: ${err}`, "transfer");
      setSyncPlan(null);
    } finally {
      setTransferring(false);
    }
  }, [
    activeProfile, local, remote, addTransfer, buildSyncPlan,
    setTransferring, setShowProgressDialog, clearLocalSelection, triggerRemoteRefresh,
    addLog, setSyncPlan, autoPurgeEnabled,
  ]);

  const startDownload = useCallback(async () => {
    if (!activeProfile || remote.selectedPaths.size === 0) return;

    // M-7: 다운로드 대상 폴더 선택 다이얼로그
    const selectedDir = await runtime.openDirectory({
      defaultPath: local.path || undefined,
      title: "다운로드 폴더 선택",
    });

    // 사용자가 취소했을 때
    if (!selectedDir) return;

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

      await runtime.invoke("start_downloads", {
        profileId: activeProfile.id,
        items: downloadItems,
        maxConcurrentFiles: readBatchSettings().maxConcurrentTransfers,
      });
      const finishedAt = new Date().toISOString();
      const transferState = useAppStore.getState();
      const downloadTransfers = downloadItems
        .map((item) => transferState.transfers.find((transfer) => transfer.id === item.id))
        .filter((transfer): transfer is TransferItem => Boolean(transfer));
      const downloadStatus = summarizeTransferStatus(downloadTransfers);
      const downloadLog = {
        id: crypto.randomUUID(),
        profileId: activeProfile.id,
        operation: "download" as const,
        status: downloadStatus,
        bucket: activeProfile.bucket,
        prefix: remote.path,
        files: downloadTransfers.map((item) => ({
          path: item.remotePath,
          operation: "download" as const,
          status: item.status === "complete" ? "success" as const : "failed" as const,
          error: item.error,
          startedAt: item.startedAt ?? finishedAt,
          finishedAt: item.completedAt ?? finishedAt,
        })),
        purgeResults: [],
        startedAt: finishedAt,
        finishedAt,
      };
      void saveOperationLog(downloadLog);

      clearRemoteSelection();
      triggerLocalRefresh(); // 다운로드 완료 후 로컬 패널 자동 갱신
    } catch (err) {
      addLog("error", `다운로드 오류: ${err}`, "transfer");
    } finally {
      setTransferring(false);
    }
  }, [
    activeProfile, local, remote, addTransfer,
    setTransferring, setShowProgressDialog, clearRemoteSelection, triggerLocalRefresh, addLog,
  ]);

  // L-1: 로컬 디렉터리 전체 ↔ S3 prefix 비교 (dry-run)
  const buildPreview = useCallback(async (): Promise<SyncPreviewResult> => {
    if (!activeProfile) throw new Error("Not connected");
    return runtime.invoke<SyncPreviewResult>("sync_preview", {
      profileId: activeProfile.id,
      localDir: local.path,
      remotePrefix: remote.path,
    });
  }, [activeProfile, local.path, remote.path]);

  const retryTransfer = useCallback(async (item: TransferItem) => {
    if (!activeProfile) return;

    updateTransfer(item.id, { status: "pending", progress: 0, transferredBytes: 0, error: undefined });

    if (item.direction === "upload") {
      const retryItem = {
        id: item.id,
        localPath: item.localPath,
        remotePath: item.remotePath,
        isOverwrite: false,
        contentTypeOverride: undefined,
        cacheControl: undefined,
        headers: {},
        metadata: {},
      };
      try {
        await runtime.invoke("upload_files", {
          profileId: activeProfile.id,
          items: [retryItem],
          cdnDistributionId: activeProfile.cdnDistributionId,
          cdnProvider: activeProfile.cdnProvider,
          maxConcurrentFiles: 1,
        });
      } catch (err) {
        updateTransfer(item.id, { status: "error", error: String(err) });
        addLog("error", `재시도 실패 [${item.fileName}]: ${err}`, "transfer");
      }
    } else {
      try {
        await runtime.invoke("start_downloads", {
          profileId: activeProfile.id,
          items: [{ id: item.id, remotePath: item.remotePath, localPath: item.localPath }],
          maxConcurrentFiles: 1,
        });
      } catch (err) {
        updateTransfer(item.id, { status: "error", error: String(err) });
        addLog("error", `재시도 실패 [${item.fileName}]: ${err}`, "transfer");
      }
    }
  }, [activeProfile, updateTransfer, addLog]);

  return { startUpload, startDownload, buildSyncPlan, buildPreview, retryTransfer };
}

function summarizeTransferStatus(transfers: TransferItem[]): "success" | "failed" | "partial" {
  if (transfers.length === 0) return "failed";
  const failed = transfers.filter((item) => item.status === "error" || item.status === "canceled").length;
  if (failed === 0) return "success";
  return failed === transfers.length ? "failed" : "partial";
}
