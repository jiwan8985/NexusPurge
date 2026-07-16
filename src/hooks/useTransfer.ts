import { useCallback, useEffect, useRef } from "react";
import { saveOperationLog } from "../services/operation-log/operation-log-service";
import { runtime } from "../services/runtime";
import { useAppStore } from "../store/appStore";
import { buildCdnUrl, CDN_LABELS, cdnDistributionIdFor, cdnDomainFor, defaultCacheControlFor } from "../utils/cdn";
import type { CdnProvider, CdnPurgeResult, CdnRequestStep, TransferItem, SyncPlan } from "../types";
import { readBatchSettings } from "../utils/batch-settings";
import { fmtClockTime } from "../utils/format-time";

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
// fileName에 "/"가 포함된 상대경로(폴더 다운로드)도 OS 구분자로 통일
function joinPath(dir: string, fileName: string): string {
  const normalized = dir.replace(/[/\\]$/, "");
  const sep = normalized.includes("\\") ? "\\" : "/";
  return `${normalized}${sep}${fileName.split("/").join(sep)}`;
}

export function useTransfer() {
  const {
    activeProfile,
    activeCdns,
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
    clearFinishedTransfers,
  } = useAppStore((s) => ({
    activeProfile: s.activeProfile,
    activeCdns: s.activeCdns,
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
    clearFinishedTransfers: s.clearFinishedTransfers,
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

  // paths 미지정 시 로컬 패널에서 선택된 항목을 업로드 (DnD/탐색기 드랍은 paths로 명시 전달)
  const startUpload = useCallback(async (paths?: string[]) => {
    const selectedPaths = paths && paths.length > 0 ? paths : Array.from(local.selectedPaths);
    if (!activeProfile || selectedPaths.length === 0) return;

    // 이전 배치의 완료/오류 항목 정리 — 진행률(N/M)이 이번 배치 기준으로 집계되게 함
    clearFinishedTransfers();
    setTransferring(true);
    // M-8: dialog는 실제 전송 항목이 있을 때만 열기

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

      // 현재 선택된 Purge 대상 CDN들 (고객사 요청: 여러 CDN 동시 선택 가능)
      const providers: CdnProvider[] = activeCdns.length > 0
        ? activeCdns
        : activeProfile.cdnProvider ? [activeProfile.cdnProvider] : [];
      const provider = providers[0]; // cdnUrl 표시 등 단일 값이 필요한 곳에 사용 (대표 CDN)
      const cdnDomain = cdnDomainFor(activeProfile, provider);

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
            autoPurgeEnabled && providers.length > 0 ? "pending" : "notRequested",
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
          const cdnUrl = buildCdnUrl(cdnDomain, remotePath, activeProfile.cdnBasePath) ?? undefined;
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
            cdnPurgeStatus: providers.length > 0 && isOverwrite ? "pending" : "notRequested",
            cdnUrl,
            startedAt: new Date().toISOString(),
          });
          return {
            id,
            localPath: file.path,
            remotePath,
            isOverwrite,
            contentTypeOverride: activeProfile.contentTypeOverride,
            cacheControl,
            headers: {},
            metadata: {},
          };
        });

      // CDN이 설정된 경우 신규 파일도 Purge 대상:
      // 해당 경로에 CDN이 404를 캐싱하고 있을 수 있으므로 업로드 후 무효화 필요
      const hasCdn = providers.length > 0;
      const uploadItems = [
        ...makeItems(plan.toUpload, hasCdn),
        ...makeItems(plan.toOverwrite, true),
      ];

      // Purge는 업로드 완료 후 일괄 배치로 실행 (cdnProvider 미전달 → Rust 파일별 Purge 비활성)
      // 파일별 개별 Purge는 CDN API burst 제한에 걸리고(효성 노드 타임아웃 등),
      // CloudFront는 파일 수만큼 Invalidation 요청이 발생하므로 배치가 올바른 구조.
      await runtime.invoke("upload_files", {
        profileId: activeProfile.id,
        items: uploadItems,
        maxConcurrentFiles: readBatchSettings().maxConcurrentTransfers,
      });
      const finishedAt = new Date().toISOString();
      const transferState = useAppStore.getState();
      const uploadTransfers = uploadItems
        .map((item) => transferState.transfers.find((transfer) => transfer.id === item.id))
        .filter((transfer): transfer is TransferItem => Boolean(transfer));
      const uploadStatus = summarizeTransferStatus(uploadTransfers);

      // ── 업로드 완료 후 일괄 Purge (선택된 모든 CDN에 동시 실행) ──────────────
      // 대상: 업로드 성공 파일 전체 + (자동 Purge ON이면) 스킵된 미변경 파일 경로
      const batchPurgeResults: {
        provider: CdnProvider;
        paths: string[];
        success: boolean;
        invalidationId?: string;
        error?: string;
        requestEndpoint?: string;
        durationMs?: number;
        requestSteps?: CdnRequestStep[];
        startedAt: string;
        finishedAt: string;
      }[] = [];

      if (providers.length > 0) {
        const uploadedPaths = uploadTransfers
          .filter((t) => t.status === "complete")
          .map((t) => t.remotePath);
        const skipPaths = autoPurgeEnabled
          ? plan.toSkip.map((f) => remote.path + f.name)
          : [];
        const purgePaths = [...uploadedPaths, ...skipPaths];

        if (purgePaths.length > 0) {
          addLog(
            "info",
            `CDN 일괄 Purge 시작 (${providers.map((p) => CDN_LABELS[p]).join(", ")}): 업로드 ${uploadedPaths.length}개${skipPaths.length > 0 ? ` + 스킵 ${skipPaths.length}개` : ""}`,
            "cdn"
          );
          const skipPathSet = new Set(skipPaths);
          const purgeStatusTargets = [
            ...uploadTransfers.filter((t) => t.status === "complete"),
            ...transferState.transfers.filter(
              (t) => t.status === "skipped" && skipPathSet.has(t.remotePath)
            ),
          ];
          const { purgeBatchSize } = readBatchSettings();
          const totalBatches = Math.ceil(purgePaths.length / purgeBatchSize);

          // provider별 결과를 remotePath 단위로 집계 (여러 CDN이 같은 파일을 동시에 Purge)
          const perPathResults = new Map<
            string,
            { provider: CdnProvider; success: boolean; invalidationId?: string; error?: string }[]
          >();

          const purgeOneProvider = async (cdnProvider: CdnProvider) => {
            const label = CDN_LABELS[cdnProvider];
            const distributionId = cdnDistributionIdFor(activeProfile, cdnProvider) ?? "";
            for (let i = 0; i < purgePaths.length; i += purgeBatchSize) {
              const batch = purgePaths.slice(i, i + purgeBatchSize);
              const batchLabel = totalBatches > 1 ? ` (배치 ${Math.floor(i / purgeBatchSize) + 1}/${totalBatches})` : "";
              const batchStartedAt = new Date().toISOString();
              let success = false;
              let invalidationId: string | undefined;
              let error: string | undefined;
              let requestEndpoint: string | undefined;
              let durationMs: number | undefined;
              let requestSteps: CdnRequestStep[] | undefined;
              try {
                const result = await runtime.invoke<CdnPurgeResult>("purge_cdn", {
                  profileId: activeProfile.id,
                  provider: cdnProvider,
                  distributionId,
                  paths: batch,
                });
                success = result.success;
                invalidationId = result.invalidationId ?? undefined;
                error = result.error ?? undefined;
                requestEndpoint = result.requestEndpoint;
                durationMs = result.durationMs;
                requestSteps = result.requestSteps;
              } catch (err) {
                error = String(err);
              }
              const finishedAtIso = new Date().toISOString();
              batchPurgeResults.push({
                provider: cdnProvider, paths: batch, success, invalidationId, error, requestEndpoint, durationMs, requestSteps,
                startedAt: batchStartedAt, finishedAt: finishedAtIso,
              });
              const timeRange = ` (시작 ${fmtClockTime(batchStartedAt)} · 종료 ${fmtClockTime(finishedAtIso)})`;
              if (success) {
                const inv = invalidationId ? ` (${invalidationId})` : "";
                const dur = durationMs !== undefined ? ` [${durationMs}ms]` : "";
                addLog("success", `[${label}] CDN Purge 완료${batchLabel}: ${batch.length}개${inv}${dur}${timeRange}`, "cdn");
              } else {
                addLog("error", `[${label}] CDN Purge 실패${batchLabel}: ${error}${timeRange}`, "cdn");
              }

              for (const path of batch) {
                const list = perPathResults.get(path) ?? [];
                list.push({ provider: cdnProvider, success, invalidationId, error });
                perPathResults.set(path, list);
              }
            }
          };

          // 선택된 CDN 전체를 동시(병렬) 실행 — 고객사 요청: 여러 CDN 한 번에 Purge
          await Promise.all(providers.map(purgeOneProvider));

          // 진행률 팝업의 파일별 Purge 배지 갱신 — 모든 CDN 결과를 집계해 한 번에 반영
          for (const t of purgeStatusTargets) {
            const results = perPathResults.get(t.remotePath);
            if (!results || results.length === 0) continue;
            const allOk = results.every((r) => r.success);
            const anyOk = results.some((r) => r.success);
            const errorText = results
              .filter((r) => !r.success)
              .map((r) => `[${CDN_LABELS[r.provider]}] ${r.error}`)
              .join(" / ");
            const invalidationIds = results
              .filter((r) => r.success && r.invalidationId)
              .map((r) => `[${CDN_LABELS[r.provider]}] ${r.invalidationId}`)
              .join(", ");
            updateTransfer(t.id, {
              cdnPurged: allOk,
              cdnPurgeStatus: allOk ? "complete" : anyOk ? "complete" : "error",
              cdnPurgeError: errorText || undefined,
              cdnInvalidationId: invalidationIds || undefined,
            });
          }
        }
      }

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
        purgeResults: batchPurgeResults.map((r) => ({
          provider: r.provider,
          urls: r.paths,
          status: r.success ? "success" as const : "failed" as const,
          requestId: r.invalidationId,
          error: r.error,
          // 감사 로그에는 실제 배치 시작/종료 시각 기록 (Purge 지연 추적용)
          startedAt: r.startedAt,
          finishedAt: r.finishedAt,
          requestEndpoint: r.requestEndpoint,
          durationMs: r.durationMs,
          requestSteps: r.requestSteps,
        })),
        startedAt: finishedAt,
        finishedAt,
      };
      void saveOperationLog(uploadLog);

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
    activeProfile, activeCdns, local, remote, addTransfer, updateTransfer, buildSyncPlan,
    setTransferring, setShowProgressDialog, clearLocalSelection, triggerRemoteRefresh,
    addLog, setSyncPlan, autoPurgeEnabled, clearFinishedTransfers,
  ]);

  // keys 미지정 시 원격 패널에서 선택된 항목을 다운로드 (우클릭 "다운로드"는 keys로 단일 파일 전달)
  const startDownload = useCallback(async (keys?: string[]) => {
    const selectedKeys = keys && keys.length > 0 ? keys : Array.from(remote.selectedPaths);
    if (!activeProfile || selectedKeys.length === 0) return;

    // M-7: 다운로드 대상 폴더 선택 다이얼로그
    const selectedDir = await runtime.openDirectory({
      defaultPath: local.path || undefined,
      title: "다운로드 폴더 선택",
    });

    // 사용자가 취소했을 때
    if (!selectedDir) return;

    // 이전 배치의 완료/오류 항목 정리 — 진행률(N/M)이 이번 배치 기준으로 집계되게 함
    clearFinishedTransfers();
    setTransferring(true);
    setShowProgressDialog(true);

    try {
      // 폴더("…/")는 하위 전체 키로 확장, 로컬에는 폴더 구조 그대로 저장
      const entries: { key: string; rel: string }[] = [];
      for (const key of selectedKeys) {
        if (key.endsWith("/")) {
          const children = await runtime.invoke<string[]>("list_s3_keys", {
            profileId: activeProfile.id,
            prefix: key,
          });
          const parent = key.replace(/[^/]+\/$/, ""); // 폴더의 부모 prefix
          for (const child of children) {
            if (child.endsWith("/")) continue; // 폴더 placeholder 제외
            entries.push({ key: child, rel: child.slice(parent.length) });
          }
        } else {
          entries.push({ key, rel: key.split("/").pop() ?? key });
        }
      }

      if (entries.length === 0) {
        addLog("warn", "다운로드할 파일이 없습니다. (빈 폴더)", "transfer");
        return;
      }
      addLog("info", `다운로드 시작: ${entries.length}개 파일`, "transfer");

      const downloadItems = entries.map(({ key, rel }) => {
        const id = crypto.randomUUID();
        // C-4: joinPath로 OS별 경로 구분자 통일 (사용자 선택 폴더 기준, 하위 폴더 구조 유지)
        const localPath = joinPath(selectedDir, rel);
        addTransfer({
          id,
          direction: "download",
          localPath,
          remotePath: key,
          fileName: rel,
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
    clearFinishedTransfers,
  ]);

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
        const provider = activeCdns[0] ?? activeProfile.cdnProvider;
        await runtime.invoke("upload_files", {
          profileId: activeProfile.id,
          items: [retryItem],
          cdnDistributionId: cdnDistributionIdFor(activeProfile, provider),
          cdnProvider: provider,
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
  }, [activeProfile, activeCdns, updateTransfer, addLog]);

  return { startUpload, startDownload, buildSyncPlan, retryTransfer };
}

function summarizeTransferStatus(transfers: TransferItem[]): "success" | "failed" | "partial" {
  if (transfers.length === 0) return "failed";
  const failed = transfers.filter((item) => item.status === "error" || item.status === "canceled").length;
  if (failed === 0) return "success";
  return failed === transfers.length ? "failed" : "partial";
}
