import { useCallback } from "react";
import { saveOperationLog } from "../services/operation-log/operation-log-service";
import { CDN_LABELS, cdnDistributionIdFor } from "../utils/cdn";
import { runtime } from "../services/runtime";
import { useAppStore } from "../store/appStore";
import type {
  CdnProvider,
  CdnPurgeResult,
  OperationLog,
  OperationStatus,
  OperationType,
  S3ListResponse,
} from "../types";

export function useS3() {
  const { activeProfile, activeCdns, setRemoteFiles, setRemoteLoading, setRemotePath, addLog } =
    useAppStore((s) => ({
      activeProfile: s.activeProfile,
      activeCdns: s.activeCdns,
      setRemoteFiles: s.setRemoteFiles,
      setRemoteLoading: s.setRemoteLoading,
      setRemotePath: s.setRemotePath,
      addLog: s.addLog,
    }));

  const listObjects = useCallback(
    async (prefix: string) => {
      if (!activeProfile) return;
      setRemoteLoading(true);
      try {
        const result = await runtime.invoke<S3ListResponse>("list_s3_objects", {
          profileId: activeProfile.id,
          prefix,
        });
        setRemoteFiles(result.files);
        setRemotePath(prefix);
        addLog("debug", `S3 list loaded: ${prefix} (${result.files.length})`, "system");
      } catch (err) {
        addLog("error", `S3 list failed: ${err}`, "system");
      } finally {
        setRemoteLoading(false);
      }
    },
    [activeProfile, setRemoteFiles, setRemoteLoading, setRemotePath, addLog]
  );

  const deleteObjects = useCallback(
    async (keys: string[]) => {
      if (!activeProfile) return;
      const startedAt = new Date().toISOString();
      const purgeEntries: {
        provider: CdnProvider;
        paths: string[];
        success: boolean;
        invalidationId?: string;
        error?: string;
        requestEndpoint?: string;
        durationMs?: number;
      }[] = [];

      try {
        const deletedKeys = await runtime.invoke<string[]>("delete_s3_objects", {
          profileId: activeProfile.id,
          keys,
        });
        addLog("success", `S3 delete completed: ${deletedKeys.length}`, "transfer");

        const providers = activeCdns.length > 0
          ? activeCdns
          : activeProfile.cdnProvider ? [activeProfile.cdnProvider] : [];
        if (providers.length > 0 && deletedKeys.length > 0) {
          // 폴더 삭제는 하위 키를 개별 나열하는 대신 "폴더/*" 와일드카드 1건으로 Purge
          const purgePaths = keys.map((k) => (k.endsWith("/") ? `${k}*` : k));

          // 선택된 모든 CDN에 동시(병렬) Purge — 고객사 요청: 여러 CDN 한 번에 Purge
          await Promise.all(providers.map(async (provider) => {
            const label = CDN_LABELS[provider];
            try {
              const result = await runtime.invoke<CdnPurgeResult>("purge_cdn", {
                profileId: activeProfile.id,
                provider,
                distributionId: cdnDistributionIdFor(activeProfile, provider) ?? "",
                paths: purgePaths,
              });
              purgeEntries.push({
                provider, paths: purgePaths,
                success: result.success, invalidationId: result.invalidationId ?? undefined, error: result.error ?? undefined,
                requestEndpoint: result.requestEndpoint, durationMs: result.durationMs,
              });
              if (result.success) {
                const id = result.invalidationId ? ` (${result.invalidationId})` : "";
                const dur = result.durationMs !== undefined ? ` [${result.durationMs}ms]` : "";
                addLog("success", `[${label}] Delete CDN purge completed: ${purgePaths.length}${id}${dur}`, "cdn");
              } else {
                addLog("error", `[${label}] Delete CDN purge failed: ${result.error}`, "cdn");
              }
            } catch (err) {
              purgeEntries.push({ provider, paths: purgePaths, success: false, error: String(err) });
              addLog("error", `[${label}] Delete CDN purge failed: ${err}`, "cdn");
            }
          }));
        }

        const anyPurgeFailed = purgeEntries.some((p) => !p.success);
        void saveOperationLog(buildOperationLog({
          profileId: activeProfile.id,
          operation: "delete",
          status: anyPurgeFailed ? "partial" : "success",
          bucket: activeProfile.bucket,
          paths: deletedKeys,
          startedAt,
          purgeEntries,
        }));
      } catch (err) {
        addLog("error", `S3 delete failed: ${err}`, "transfer");
        void saveOperationLog(buildOperationLog({
          profileId: activeProfile.id,
          operation: "delete",
          status: "failed",
          bucket: activeProfile.bucket,
          paths: keys,
          startedAt,
          error: String(err),
        }));
        throw err;
      }
    },
    [activeProfile, activeCdns, addLog]
  );

  const createDirectory = useCallback(
    async (prefix: string) => {
      if (!activeProfile) return;
      const startedAt = new Date().toISOString();
      try {
        await runtime.invoke("put_s3_object", {
          profileId: activeProfile.id,
          key: prefix.endsWith("/") ? prefix : prefix + "/",
          content: new Uint8Array(0),
          contentType: "application/x-directory",
        });
        addLog("info", `S3 folder created: ${prefix}`, "transfer");
        void saveOperationLog(buildOperationLog({
          profileId: activeProfile.id,
          operation: "mkdir",
          status: "success",
          bucket: activeProfile.bucket,
          prefix,
          paths: [prefix],
          startedAt,
        }));
      } catch (err) {
        void saveOperationLog(buildOperationLog({
          profileId: activeProfile.id,
          operation: "mkdir",
          status: "failed",
          bucket: activeProfile.bucket,
          prefix,
          paths: [prefix],
          startedAt,
          error: String(err),
        }));
        throw err;
      }
    },
    [activeProfile, addLog]
  );

  const getPresignedUrl = useCallback(
    async (key: string, expiresInSeconds = 3600): Promise<string> => {
      if (!activeProfile) throw new Error("Not connected");
      return runtime.invoke<string>("get_presigned_url", {
        profileId: activeProfile.id,
        key,
        expiresInSeconds,
      });
    },
    [activeProfile]
  );

  const renameObject = useCallback(
    async (oldKey: string, newKey: string) => {
      if (!activeProfile) return;
      const startedAt = new Date().toISOString();
      try {
        await runtime.invoke("rename_s3_object", {
          profileId: activeProfile.id,
          oldKey,
          newKey,
        });
        addLog("success", `S3 renamed: ${oldKey} -> ${newKey}`, "transfer");
        void saveOperationLog(buildOperationLog({
          profileId: activeProfile.id,
          operation: "rename",
          status: "success",
          bucket: activeProfile.bucket,
          paths: [oldKey, newKey],
          startedAt,
        }));
      } catch (err) {
        addLog("error", `S3 rename failed: ${err}`, "transfer");
        void saveOperationLog(buildOperationLog({
          profileId: activeProfile.id,
          operation: "rename",
          status: "failed",
          bucket: activeProfile.bucket,
          paths: [oldKey, newKey],
          startedAt,
          error: String(err),
        }));
        throw err;
      }
    },
    [activeProfile, addLog]
  );

  return { listObjects, deleteObjects, createDirectory, getPresignedUrl, renameObject };
}

function buildOperationLog(params: {
  profileId: string;
  operation: OperationType;
  status: OperationStatus;
  bucket?: string;
  prefix?: string;
  paths: string[];
  startedAt: string;
  error?: string;
  purgeEntries?: {
    provider: CdnProvider;
    paths: string[];
    success: boolean;
    invalidationId?: string;
    error?: string;
    requestEndpoint?: string;
    durationMs?: number;
  }[];
}): OperationLog {
  const finishedAt = new Date().toISOString();
  return {
    id: crypto.randomUUID(),
    profileId: params.profileId,
    operation: params.operation,
    status: params.status,
    bucket: params.bucket,
    prefix: params.prefix,
    files: params.paths.map((path) => ({
      path,
      operation: params.operation,
      status: params.error ? "failed" : "success",
      error: params.error,
      startedAt: params.startedAt,
      finishedAt,
    })),
    purgeResults: (params.purgeEntries ?? []).map((p) => ({
      provider: p.provider,
      urls: p.paths,
      status: p.success ? "success" as const : "failed" as const,
      requestId: p.invalidationId,
      error: p.error,
      requestEndpoint: p.requestEndpoint,
      durationMs: p.durationMs,
      startedAt: params.startedAt,
      finishedAt,
    })),
    startedAt: params.startedAt,
    finishedAt,
  };
}
