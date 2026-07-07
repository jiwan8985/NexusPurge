import { useCallback, useRef, useState } from "react";
import { cdnDistributionIdFor, cdnDomainFor } from "../utils/cdn";
import { readBatchSettings } from "../utils/batch-settings";
import { saveOperationLog } from "../services/operation-log/operation-log-service";
import { useAppStore } from "../store/appStore";
import { runtime } from "../services/runtime";
import type { CdnPurgeResult, PurgeExecutionResult } from "../types";

export function usePurge() {
  const { activeProfile, activeCdn, remote, addLog } = useAppStore((s) => ({
    activeProfile: s.activeProfile,
    activeCdn:     s.activeCdn,
    remote:        s.remote,
    addLog:        s.addLog,
  }));

  const [isPurging, setIsPurging] = useState(false);
  const isPurgingRef = useRef(false);

  const executePurge = useCallback(
    async (paths: string[]): Promise<PurgeExecutionResult | null> => {
      const provider = activeCdn ?? activeProfile?.cdnProvider;
      if (!activeProfile || !provider) return null;
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

      for (let i = 0; i < batchArrays.length; i++) {
        const batch = batchArrays[i];
        const batchLabel = totalBatches > 1 ? ` (배치 ${i + 1}/${totalBatches})` : "";
        const batchStartedAt = new Date().toISOString();

        try {
          const result = await runtime.invoke<CdnPurgeResult>("purge_cdn", {
            profileId: activeProfile.id,
            provider,
            distributionId: cdnDistributionIdFor(activeProfile, provider) ?? "",
            paths: batch,
          });

          batchResults.push({
            paths: batch,
            success: result.success,
            invalidationId: result.invalidationId ?? undefined,
            error: result.error ?? undefined,
            startedAt: batchStartedAt,
            finishedAt: new Date().toISOString(),
          });

          if (result.success) {
            const inv = result.invalidationId ? ` (${result.invalidationId})` : "";
            addLog("success", `CDN Purge 완료${batchLabel}: ${batch.length}개${inv}`, "cdn");
          } else {
            failedCount += batch.length;
            addLog("error", `CDN Purge 실패${batchLabel}: ${result.error}`, "cdn");
          }
        } catch (err) {
          failedCount += batch.length;
          batchResults.push({
            paths: batch,
            success: false,
            error: String(err),
            startedAt: batchStartedAt,
            finishedAt: new Date().toISOString(),
          });
          addLog("error", `CDN Purge 오류${batchLabel}: ${err}`, "cdn");
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
          provider,
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
        provider,
        domain: cdnDomainFor(activeProfile, provider),
        totalPaths: paths.length,
        batches: batchResults,
        successCount,
        failedCount,
        startedAt,
        finishedAt,
      };
    },
    [activeProfile, activeCdn, remote.path, addLog]
  );

  // 폴더 선택("…/") 은 하위 전체를 커버하도록 와일드카드로 변환
  const selectedPaths = Array.from(remote.selectedPaths).map((p) =>
    p.endsWith("/") ? `${p}*` : p
  );
  const allPrefix = remote.path
    ? `${remote.path.replace(/\/$/, "")}/*`
    : "/*";

  return { executePurge, isPurging, selectedPaths, allPrefix };
}
