import { useCallback, useRef, useState } from "react";
import { readBatchSettings } from "../utils/batch-settings";
import { useAppStore } from "../store/appStore";
import { runtime } from "../services/runtime";
import type { CdnPurgeResult } from "../types";

export function usePurge() {
  const { activeProfile, remote, addLog } = useAppStore((s) => ({
    activeProfile: s.activeProfile,
    remote: s.remote,
    addLog: s.addLog,
  }));

  const [isPurging, setIsPurging] = useState(false);
  const isPurgingRef = useRef(false);

  const executePurge = useCallback(
    async (paths: string[]): Promise<CdnPurgeResult | null> => {
      if (!activeProfile?.cdnProvider) return null;
      if (isPurgingRef.current) {
        addLog("warn", "CDN Purge가 이미 진행 중입니다. 완료 후 재시도하세요.", "cdn");
        return null;
      }

      isPurgingRef.current = true;
      setIsPurging(true);

      const { purgeBatchSize } = readBatchSettings();
      const batches: string[][] = [];
      for (let i = 0; i < paths.length; i += purgeBatchSize) {
        batches.push(paths.slice(i, i + purgeBatchSize));
      }

      const totalBatches = batches.length;
      addLog(
        "info",
        totalBatches > 1
          ? `CDN Purge 시작: 총 ${paths.length}개 경로 → ${totalBatches}개 배치로 분할`
          : `CDN Purge 시작: ${paths.length}개 경로`,
        "cdn"
      );

      let lastResult: CdnPurgeResult | null = null;
      let failedCount = 0;

      for (let i = 0; i < batches.length; i++) {
        const batch = batches[i];
        const batchLabel = totalBatches > 1 ? ` (배치 ${i + 1}/${totalBatches})` : "";

        try {
          const result = await runtime.invoke<CdnPurgeResult>("purge_cdn", {
            profileId: activeProfile.id,
            provider: activeProfile.cdnProvider,
            distributionId: activeProfile.cdnDistributionId ?? "",
            paths: batch,
          });

          lastResult = result;

          if (result.success) {
            const inv = result.invalidationId ? ` (${result.invalidationId})` : "";
            addLog("success", `CDN Purge 완료${batchLabel}: ${batch.length}개${inv}`, "cdn");
          } else {
            failedCount += batch.length;
            addLog("error", `CDN Purge 실패${batchLabel}: ${result.error}`, "cdn");
          }
        } catch (err) {
          failedCount += batch.length;
          addLog("error", `CDN Purge 오류${batchLabel}: ${err}`, "cdn");
        }
      }

      if (totalBatches > 1) {
        const successCount = paths.length - failedCount;
        if (failedCount === 0) {
          addLog("success", `CDN Purge 전체 완료: 총 ${paths.length}개 (${totalBatches}배치)`, "cdn");
        } else {
          addLog("warn", `CDN Purge 부분 완료: 성공 ${successCount}개 / 실패 ${failedCount}개`, "cdn");
        }
      }

      isPurgingRef.current = false;
      setIsPurging(false);
      return lastResult;
    },
    [activeProfile, addLog]
  );

  const selectedPaths = Array.from(remote.selectedPaths);

  const allPrefix = remote.path
    ? `${remote.path.replace(/\/$/, "")}/*`
    : "/*";

  return { executePurge, isPurging, selectedPaths, allPrefix };
}
