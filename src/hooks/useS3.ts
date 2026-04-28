import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../store/appStore";
import type { S3ListResponse } from "../types";

// S3 파일 탐색 및 단순 조작 훅
// 실제 업로드/다운로드는 useTransfer에서 처리
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
        const result = await invoke<S3ListResponse>("list_s3_objects", {
          profileId: activeProfile.id,
          prefix,
        });
        setRemoteFiles(result.files);
        setRemotePath(prefix);
        addLog("debug", `S3 목록 로드: ${prefix} (${result.files.length}개)`, "system");
      } catch (err) {
        addLog("error", `S3 목록 로드 실패: ${err}`, "system");
      } finally {
        setRemoteLoading(false);
      }
    },
    [activeProfile, setRemoteFiles, setRemoteLoading, setRemotePath, addLog]
  );

  const deleteObjects = useCallback(
    async (keys: string[]) => {
      if (!activeProfile) return;
      try {
        await invoke("delete_s3_objects", {
          profileId: activeProfile.id,
          keys,
        });
        addLog("success", `S3 삭제 완료: ${keys.length}개`, "transfer");
      } catch (err) {
        addLog("error", `S3 삭제 실패: ${err}`, "transfer");
        throw err;
      }
    },
    [activeProfile, addLog]
  );

  const createDirectory = useCallback(
    async (prefix: string) => {
      if (!activeProfile) return;
      // S3는 실제 디렉토리가 없으므로 빈 객체로 표현 (key = prefix + "/")
      await invoke("put_s3_object", {
        profileId: activeProfile.id,
        key: prefix.endsWith("/") ? prefix : prefix + "/",
        content: new Uint8Array(0),
        contentType: "application/x-directory",
      });
      addLog("info", `S3 폴더 생성: ${prefix}`, "transfer");
    },
    [activeProfile, addLog]
  );

  // S3 presigned URL 생성 (다운로드 공유용)
  const getPresignedUrl = useCallback(
    async (key: string, expiresInSeconds = 3600): Promise<string> => {
      if (!activeProfile) throw new Error("Not connected");
      return invoke<string>("get_presigned_url", {
        profileId: activeProfile.id,
        key,
        expiresInSeconds,
      });
    },
    [activeProfile]
  );

  // H-1: S3 오브젝트 이름 변경 (CopyObject + DeleteObject)
  const renameObject = useCallback(
    async (oldKey: string, newKey: string) => {
      if (!activeProfile) return;
      try {
        await invoke("rename_s3_object", {
          profileId: activeProfile.id,
          oldKey,
          newKey,
        });
        addLog("success", `S3 이름 변경: ${oldKey} → ${newKey}`, "transfer");
      } catch (err) {
        addLog("error", `S3 이름 변경 실패: ${err}`, "transfer");
        throw err;
      }
    },
    [activeProfile, addLog]
  );

  return { listObjects, deleteObjects, createDirectory, getPresignedUrl, renameObject };
}
