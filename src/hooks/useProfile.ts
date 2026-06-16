import { useCallback } from "react";
import { runtime } from "../services/runtime";
import { useAppStore } from "../store/appStore";
import type { S3Profile } from "../types";

type S3ConnectionTestResult = {
  success: boolean;
  warnings: string[];
};

function normalizePrefix(prefix: string | undefined): string {
  const trimmed = prefix?.trim();
  if (!trimmed) return "/";
  const withoutLeadingSlash = trimmed.replace(/^\/+/, "");
  return withoutLeadingSlash.endsWith("/") ? withoutLeadingSlash : `${withoutLeadingSlash}/`;
}

export function useProfile() {
  const {
    setActiveProfile,
    setConnected,
    setConnecting,
    addLog,
    profiles,
    setProfiles,
    setLastProfileId,
    setRemotePath,
  } = useAppStore((s) => ({
    setActiveProfile: s.setActiveProfile,
    setConnected: s.setConnected,
    setConnecting: s.setConnecting,
    addLog: s.addLog,
    profiles: s.profiles,
    setProfiles: s.setProfiles,
    setLastProfileId: s.setLastProfileId,
    setRemotePath: s.setRemotePath,
  }));

  const loadProfiles = useCallback(async () => {
    try {
      const saved = await runtime.invoke<S3Profile[]>("load_profiles");
      setProfiles(saved);
    } catch (err) {
      addLog("error", `프로필 로드 실패: ${err}`, "profile");
    }
  }, [addLog, setProfiles]);

  const saveProfile = useCallback(
    async (profile: S3Profile) => {
      await runtime.invoke("save_profile", { profile });
      await loadProfiles();
      addLog("success", `프로필 저장됨: ${profile.name}`, "profile");
    },
    [loadProfiles, addLog]
  );

  const deleteProfile = useCallback(
    async (id: string) => {
      await runtime.invoke("delete_profile", { id });
      await loadProfiles();
      addLog("info", "프로필 삭제됨", "profile");
    },
    [loadProfiles, addLog]
  );

  /** H-3: 저장 없이 입력값으로 직접 연결 테스트 */
  const testConnection = useCallback(
    async (params: {
      region: string;
      bucket: string;
      basePrefix?: string;
      accessKey: string;
      secretKey: string;
      endpoint?: string;
    }): Promise<{ success: boolean; error?: string; warnings?: string[] }> => {
      try {
        const result = await runtime.invoke<S3ConnectionTestResult>("test_s3_connection", {
          region:     params.region,
          bucket:     params.bucket,
          basePrefix: params.basePrefix ?? null,
          accessKey:  params.accessKey,
          secretKey:  params.secretKey,
          endpoint:   params.endpoint ?? null,
        });
        return { success: true, warnings: result.warnings };
      } catch (err) {
        return { success: false, error: String(err) };
      }
    },
    []
  );

  const connectWithProfile = useCallback(
    async (profile: S3Profile) => {
      setConnecting(true);
      setActiveProfile(profile);
      addLog("info", `연결 시도: ${profile.name} (${profile.bucket})`, "system");
      try {
        const result = await runtime.invoke<S3ConnectionTestResult>("connect_s3", { profileId: profile.id });
        setRemotePath(normalizePrefix(profile.basePrefix));
        setConnected(true);
        // H-7: 마지막 연결 프로파일 저장
        setLastProfileId(profile.id);
        await runtime.invoke("save_last_profile_id", { id: profile.id });
        result.warnings.forEach((warning) => addLog("warn", warning, "system"));
        addLog("success", `연결 성공: ${profile.bucket} (${profile.region})`, "system");
      } catch (err) {
        setConnected(false);
        setActiveProfile(null);
        addLog("error", `연결 실패: ${err}`, "system");
        throw err;
      } finally {
        setConnecting(false);
      }
    },
    [setActiveProfile, setConnected, setConnecting, setLastProfileId, setRemotePath, addLog]
  );

  const disconnect = useCallback(() => {
    setActiveProfile(null);
    setConnected(false);
    addLog("info", "연결 해제됨", "system");
  }, [setActiveProfile, setConnected, addLog]);

  return {
    profiles,
    loadProfiles,
    saveProfile,
    deleteProfile,
    testConnection,
    connectWithProfile,
    disconnect,
  };
}
