import { useCallback } from "react";
import { useAppStore } from "../store/appStore";
import { runtime } from "../services/runtime";
import type { CdnPurgeResult } from "../types";

export const PURGE_WARN_THRESHOLD = 1_000;
export const PURGE_LIMIT_THRESHOLD = 5_000;

export function usePurge() {
  const { activeProfile, remote, addLog } = useAppStore((s) => ({
    activeProfile: s.activeProfile,
    remote: s.remote,
    addLog: s.addLog,
  }));

  const executePurge = useCallback(
    async (paths: string[]): Promise<CdnPurgeResult | null> => {
      if (!activeProfile?.cdnProvider) return null;

      addLog("info", `CDN Purge 시작: ${paths.length}개 경로`, "cdn");
      try {
        const result = await runtime.invoke<CdnPurgeResult>("purge_cdn", {
          profileId: activeProfile.id,
          provider: activeProfile.cdnProvider,
          distributionId: activeProfile.cdnDistributionId ?? "",
          paths,
        });

        if (result.success) {
          addLog(
            "success",
            `CDN Purge 완료: ${paths.length}개 (Invalidation: ${result.invalidationId ?? "-"})`,
            "cdn"
          );
        } else {
          addLog("error", `CDN Purge 실패: ${result.error}`, "cdn");
        }
        return result;
      } catch (err) {
        addLog("error", `CDN Purge 오류: ${err}`, "cdn");
        return null;
      }
    },
    [activeProfile, addLog]
  );

  const selectedPaths = Array.from(remote.selectedPaths);

  const allPrefix = remote.path
    ? `${remote.path.replace(/\/$/, "")}/*`
    : "/*";

  return { executePurge, selectedPaths, allPrefix };
}
