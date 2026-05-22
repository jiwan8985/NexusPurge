import { useCallback } from "react";
import { saveOperationLog } from "../services/operation-log/operation-log-service";
import { runtime } from "../services/runtime";
import { useAppStore } from "../store/appStore";
import type {
  CdnPurgeResult,
  OperationLog,
  OperationStatus,
  OperationType,
  S3ListResponse,
} from "../types";

export function useS3() {
  const { activeProfile, setRemoteFiles, setRemoteLoading, setRemotePath, addLog } =
    useAppStore((s) => ({
      activeProfile: s.activeProfile,
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
      let purgeResult: CdnPurgeResult | undefined;
      let purgeError: string | undefined;

      try {
        const deletedKeys = await runtime.invoke<string[]>("delete_s3_objects", {
          profileId: activeProfile.id,
          keys,
        });
        addLog("success", `S3 delete completed: ${deletedKeys.length}`, "transfer");

        if (activeProfile.cdnProvider && deletedKeys.length > 0) {
          try {
            purgeResult = await runtime.invoke<CdnPurgeResult>("purge_cdn", {
              profileId: activeProfile.id,
              provider: activeProfile.cdnProvider,
              distributionId: activeProfile.cdnDistributionId ?? "",
              paths: deletedKeys,
            });

            if (purgeResult.success) {
              const id = purgeResult.invalidationId ? ` (${purgeResult.invalidationId})` : "";
              addLog("success", `Delete CDN purge completed: ${deletedKeys.length}${id}`, "cdn");
            } else {
              addLog("error", `Delete CDN purge failed: ${purgeResult.error}`, "cdn");
            }
          } catch (err) {
            purgeError = String(err);
            addLog("error", `Delete CDN purge failed: ${purgeError}`, "cdn");
          }
        }

        void saveOperationLog(buildOperationLog({
          profileId: activeProfile.id,
          operation: "delete",
          status: purgeResult?.success === false || purgeError ? "partial" : "success",
          bucket: activeProfile.bucket,
          paths: deletedKeys,
          startedAt,
          purgeResult,
          purgeError,
          purgeProvider: activeProfile.cdnProvider,
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
    [activeProfile, addLog]
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
  purgeResult?: CdnPurgeResult;
  purgeError?: string;
  purgeProvider?: CdnPurgeResult["provider"];
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
    purgeResults: params.purgeResult
      ? [{
          provider: params.purgeResult.provider,
          urls: params.purgeResult.paths,
          status: params.purgeResult.success ? "success" : "failed",
          requestId: params.purgeResult.invalidationId,
          error: params.purgeResult.error,
          startedAt: params.startedAt,
          finishedAt,
        }]
      : params.purgeError && params.purgeProvider
        ? [{
            provider: params.purgeProvider,
            urls: params.paths,
            status: "failed",
            error: params.purgeError,
            startedAt: params.startedAt,
            finishedAt,
          }]
        : [],
    startedAt: params.startedAt,
    finishedAt,
  };
}
