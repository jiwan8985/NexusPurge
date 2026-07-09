import { useCallback, useRef, useState } from "react";
import { CDN_LABELS, cdnDistributionIdFor, cdnDomainFor } from "../utils/cdn";
import { readBatchSettings } from "../utils/batch-settings";
import { saveOperationLog } from "../services/operation-log/operation-log-service";
import { useAppStore } from "../store/appStore";
import { runtime } from "../services/runtime";
import type { CdnProvider, CdnPurgeResult, PurgeExecutionResult } from "../types";

export function usePurge() {
  const { activeProfile, activeCdns, remote, addLog } = useAppStore((s) => ({
    activeProfile: s.activeProfile,
    activeCdns:    s.activeCdns,
    remote:        s.remote,
    addLog:        s.addLog,
  }));

  const [isPurging, setIsPurging] = useState(false);
  const isPurgingRef = useRef(false);

  // 단일 CDN 대상 Purge (배치 분할 + 로그) — executePurge에서 provider별 병렬 실행에 사용
  const purgeOneProvider = useCallback(
    async (
      profile: NonNullable<typeof activeProfile>,
      provider: CdnProvider,
      paths: string[]
    ): Promise<PurgeExecutionResult> => {
      const label = CDN_LABELS[provider];
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
          ? `[${label}] CDN Purge 시작: 총 ${paths.length}개 경로 → ${totalBatches}개 배치로 분할`
          : `[${label}] CDN Purge 시작: ${paths.length}개 경로`,
        "cdn"
      );

      let failedCount = 0;
      const batchResults: PurgeExecutionResult["batches"] = [];
      const distributionId = cdnDistributionIdFor(profile, provider) ?? "";

      for (let i = 0; i < batchArrays.length; i++) {
        const batch = batchArrays[i];
        const batchLabel = totalBatches > 1 ? ` (배치 ${i + 1}/${totalBatches})` : "";
        const batchStartedAt = new Date().toISOString();

        try {
          const result = await runtime.invoke<CdnPurgeResult>("purge_cdn", {
            profileId: profile.id,
            provider,
            distributionId,
            paths: batch,
          });

          batchResults.push({
            paths: batch,
            success: result.success,
            invalidationId: result.invalidationId ?? undefined,
            error: result.error ?? undefined,
            startedAt: batchStartedAt,
            finishedAt: new Date().toISOString(),
            requestEndpoint: result.requestEndpoint,
            durationMs: result.durationMs,
          });

          if (result.success) {
            const inv = result.invalidationId ? ` (${result.invalidationId})` : "";
            const dur = result.durationMs !== undefined ? ` [${result.durationMs}ms]` : "";
            addLog("success", `[${label}] CDN Purge 완료${batchLabel}: ${batch.length}개${inv}${dur}`, "cdn");
          } else {
            failedCount += batch.length;
            addLog("error", `[${label}] CDN Purge 실패${batchLabel}: ${result.error}`, "cdn");
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
          addLog("error", `[${label}] CDN Purge 오류${batchLabel}: ${err}`, "cdn");
        }
      }

      const finishedAt = new Date().toISOString();
      const successCount = paths.length - failedCount;

      if (totalBatches > 1) {
        if (failedCount === 0) {
          addLog("success", `[${label}] CDN Purge 전체 완료: 총 ${paths.length}개 (${totalBatches}배치)`, "cdn");
        } else {
          addLog("warn", `[${label}] CDN Purge 부분 완료: 성공 ${successCount}개 / 실패 ${failedCount}개`, "cdn");
        }
      }

      return {
        provider,
        domain: cdnDomainFor(profile, provider),
        totalPaths: paths.length,
        batches: batchResults,
        successCount,
        failedCount,
        startedAt,
        finishedAt,
      };
    },
    [addLog]
  );

  // 선택된 모든 CDN에 동시(병렬) Purge — 고객사 요청: 여러 CDN 한번에 Purge
  const executePurge = useCallback(
    async (paths: string[]): Promise<PurgeExecutionResult[] | null> => {
      const providers = activeCdns.length > 0
        ? activeCdns
        : activeProfile?.cdnProvider
          ? [activeProfile.cdnProvider]
          : [];
      if (!activeProfile || providers.length === 0) return null;
      if (isPurgingRef.current) {
        addLog("warn", "CDN Purge가 이미 진행 중입니다. 완료 후 재시도하세요.", "cdn");
        return null;
      }

      isPurgingRef.current = true;
      setIsPurging(true);

      const overallStartedAt = new Date().toISOString();
      if (providers.length > 1) {
        addLog(
          "info",
          `${providers.length}개 CDN 동시 Purge 시작: ${providers.map((p) => CDN_LABELS[p]).join(", ")}`,
          "cdn"
        );
      }

      const results = await Promise.all(
        providers.map((provider) => purgeOneProvider(activeProfile, provider, paths))
      );

      const overallFinishedAt = new Date().toISOString();
      const totalFailed = results.reduce((sum, r) => sum + r.failedCount, 0);
      if (providers.length > 1) {
        if (totalFailed === 0) {
          addLog("success", `${providers.length}개 CDN Purge 전체 완료`, "cdn");
        } else {
          addLog("warn", `${providers.length}개 CDN 중 일부 Purge 실패 (실패 ${totalFailed}건)`, "cdn");
        }
      }

      const overallStatus = totalFailed === 0
        ? "success" as const
        : results.every((r) => r.failedCount === r.totalPaths) ? "failed" as const : "partial" as const;

      void saveOperationLog({
        id: crypto.randomUUID(),
        profileId: activeProfile.id,
        operation: "purge",
        status: overallStatus,
        bucket: activeProfile.bucket,
        prefix: remote.path,
        files: [],
        purgeResults: results.flatMap((r) =>
          r.batches.map((b) => ({
            provider: r.provider,
            urls: b.paths,
            status: b.success ? "success" as const : "failed" as const,
            requestId: b.invalidationId,
            error: b.error,
            startedAt: b.startedAt,
            finishedAt: b.finishedAt,
            requestEndpoint: b.requestEndpoint,
            durationMs: b.durationMs,
          }))
        ),
        startedAt: overallStartedAt,
        finishedAt: overallFinishedAt,
      });

      isPurgingRef.current = false;
      setIsPurging(false);

      return results;
    },
    [activeProfile, activeCdns, remote.path, addLog, purgeOneProvider]
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
