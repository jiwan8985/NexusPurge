import { useCallback, useRef, useState } from "react";
import { readBatchSettings } from "../utils/batch-settings";
import { saveOperationLog } from "../services/operation-log/operation-log-service";
import { useAppStore } from "../store/appStore";
import { runtime } from "../services/runtime";
import type { CdnPurgeResult, PurgeExecutionResult, CdnProvider } from "../types";

export function usePurge() {
  const { activeProfile, remote, addLog } = useAppStore((s) => ({
    activeProfile: s.activeProfile,
    remote:        s.remote,
    addLog:        s.addLog,
  }));

  const [isPurging, setIsPurging] = useState(false);
  const isPurgingRef = useRef(false);

  const executePurge = useCallback(
    async (paths: string[]): Promise<PurgeExecutionResult | null> => {
      if (!activeProfile?.cdnProvider) return null;
      if (isPurgingRef.current) {
        addLog("warn", "CDN Purge가 이미 진행 중입니다. 완료 후 재시도하세요.", "cdn");
        return null;
      }

      isPurgingRef.current = true;
      setIsPurging(true);

      const { purgeBatchSize } = readBatchSettings();
      const batchArrays: string[][] = [];
      for (let i = 0; i < paths.length; i += purgeBatchSize) {
        batchArrays.push(paths.slice(i, i + purgeBatchSize));
      }

      const totalBatches = batchArrays.length;
      const startedAt = new Date().toISOString();
      addLog(
        "info",
        totalBatches > 1
          ? `CDN Purge 시작: 총 ${paths.length}개 경로 → ${totalBatches}개 배치로 분할`
          : `CDN Purge 시작: ${paths.length}개 경로`,
        "cdn"
      );

      let failedCount = 0;
      const batchResults: PurgeExecutionResult["batches"] = [];

      // Determine active providers to purge
      const providersToPurge: { provider: CdnProvider; distributionId?: string }[] = [];
      if ((activeProfile.cdnProvider as string) === "multiple" && activeProfile.cdnProviders) {
        activeProfile.cdnProviders.forEach((c) => {
          if (c.enabled) {
            providersToPurge.push({ provider: c.provider, distributionId: c.distributionId });
          }
        });
      } else if (activeProfile.cdnProvider) {
        providersToPurge.push({ provider: activeProfile.cdnProvider, distributionId: activeProfile.cdnDistributionId });
      }

      for (let i = 0; i < batchArrays.length; i++) {
        const batch = batchArrays[i];
        const batchLabel = totalBatches > 1 ? ` (배치 ${i + 1}/${totalBatches})` : "";
        const batchStartedAt = new Date().toISOString();

        for (const p of providersToPurge) {
          try {
            const result = await runtime.invoke<CdnPurgeResult>("purge_cdn", {
              profileId: activeProfile.id,
              provider: p.provider,
              distributionId: p.distributionId ?? "",
              paths: batch,
            });

            batchResults.push({
              provider: p.provider,
              paths: batch,
              success: result.success,
              invalidationId: result.invalidationId ?? undefined,
              error: result.error ?? undefined,
              startedAt: batchStartedAt,
              finishedAt: new Date().toISOString(),
            });

            if (result.success) {
              const inv = result.invalidationId ? ` (${result.invalidationId})` : "";
              addLog("success", `CDN Purge 완료 (${p.provider})${batchLabel}: ${batch.length}개${inv}`, "cdn");
            } else {
              failedCount += batch.length;
              addLog("error", `CDN Purge 실패 (${p.provider})${batchLabel}: ${result.error}`, "cdn");
            }
          } catch (err) {
            failedCount += batch.length;
            batchResults.push({
              provider: p.provider,
              paths: batch,
              success: false,
              error: String(err),
              startedAt: batchStartedAt,
              finishedAt: new Date().toISOString(),
            });
            addLog("error", `CDN Purge 오류 (${p.provider})${batchLabel}: ${err}`, "cdn");
          }
        }
      }

      const finishedAt = new Date().toISOString();
      const successCount = paths.length - failedCount;

      if (totalBatches > 1) {
        if (failedCount === 0) {
          addLog("success", `CDN Purge 전체 완료: 총 ${paths.length}개 (${totalBatches}배치)`, "cdn");
        } else {
          addLog("warn", `CDN Purge 부분 완료: 성공 ${successCount}개 / 실패 ${failedCount}개`, "cdn");
        }
      }

      const overallStatus = failedCount === 0
        ? "success" as const
        : failedCount === paths.length ? "failed" as const : "partial" as const;

      void saveOperationLog({
        id: crypto.randomUUID(),
        profileId: activeProfile.id,
        operation: "purge",
        status: overallStatus,
        bucket: activeProfile.bucket,
        prefix: remote.path,
        files: [],
        purgeResults: batchResults.map((r) => ({
          provider: r.provider || activeProfile.cdnProvider || ("multiple" as CdnProvider),
          urls: r.paths,
          status: r.success ? "success" as const : "failed" as const,
          requestId: r.invalidationId,
          error: r.error,
          startedAt: r.startedAt,
          finishedAt: r.finishedAt,
        })),
        startedAt,
        finishedAt,
      });

      isPurgingRef.current = false;
      setIsPurging(false);

      return {
        provider: activeProfile.cdnProvider,
        domain: activeProfile.cdnDomain,
        totalPaths: paths.length,
        batches: batchResults,
        successCount,
        failedCount,
        startedAt,
        finishedAt,
      };
    },
    [activeProfile, remote.path, addLog]
  );

  const selectedPaths = Array.from(remote.selectedPaths);
  const allPrefix = remote.path
    ? `${remote.path.replace(/\/$/, "")}/*`
    : "/*";

  return { executePurge, isPurging, selectedPaths, allPrefix };
}
