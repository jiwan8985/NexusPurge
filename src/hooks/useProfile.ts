import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../store/appStore";
import type { S3Profile } from "../types";

export function useProfile() {
  const {
    setActiveProfile,
    setConnected,
    setConnecting,
    addLog,
    profiles,
    setProfiles,
    setLastProfileId,
  } = useAppStore((s) => ({
    setActiveProfile: s.setActiveProfile,
    setConnected: s.setConnected,
    setConnecting: s.setConnecting,
    addLog: s.addLog,
    profiles: s.profiles,
    setProfiles: s.setProfiles,
    setLastProfileId: s.setLastProfileId,
  }));

  const loadProfiles = useCallback(async () => {
    try {
      const saved = await invoke<S3Profile[]>("load_profiles");
      setProfiles(saved);
    } catch (err) {
      addLog("error", `프로필 로드 실패: ${err}`, "profile");
    }
  }, [addLog, setProfiles]);

  const saveProfile = useCallback(
    async (profile: S3Profile) => {
      await invoke("save_profile", { profile });
      await loadProfiles();
      addLog("success", `프로필 저장됨: ${profile.name}`, "profile");
    },
    [loadProfiles, addLog]
  );

  const deleteProfile = useCallback(
    async (id: string) => {
      await invoke("delete_profile", { id });
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
      accessKey: string;
      secretKey: string;
      endpoint?: string;
    }): Promise<{ success: boolean; error?: string }> => {
      try {
        await invoke("test_s3_connection", {
          region:    params.region,
          bucket:    params.bucket,
          accessKey: params.accessKey,
          secretKey: params.secretKey,
          endpoint:  params.endpoint ?? null,
        });
        return { success: true };
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
        await invoke("connect_s3", { profileId: profile.id });
        setConnected(true);
        // H-7: 마지막 연결 프로파일 저장
        setLastProfileId(profile.id);
        await invoke("save_last_profile_id", { id: profile.id });
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
    [setActiveProfile, setConnected, setConnecting, setLastProfileId, addLog]
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
