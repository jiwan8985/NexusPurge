import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../store/appStore";
import type { S3Profile } from "../types";

// 프로파일은 Tauri keyring + 로컬 JSON으로 관리
// 민감 정보(accessKeyId, secretAccessKey)는 OS keyring에 저장
export function useProfile() {
  const [profiles, setProfiles] = useState<S3Profile[]>([]);
  const { setActiveProfile, setConnected, setConnecting, addLog } = useAppStore((s) => ({
    setActiveProfile: s.setActiveProfile,
    setConnected: s.setConnected,
    setConnecting: s.setConnecting,
    addLog: s.addLog,
  }));

  const loadProfiles = useCallback(async () => {
    try {
      const saved = await invoke<S3Profile[]>("load_profiles");
      setProfiles(saved);
    } catch (err) {
      addLog("error", `프로파일 로드 실패: ${err}`);
    }
  }, [addLog]);

  const saveProfile = useCallback(
    async (profile: S3Profile) => {
      await invoke("save_profile", { profile });
      await loadProfiles();
      addLog("success", `프로파일 저장됨: ${profile.name}`);
    },
    [loadProfiles, addLog]
  );

  const deleteProfile = useCallback(
    async (id: string) => {
      await invoke("delete_profile", { id });
      await loadProfiles();
      addLog("info", "프로파일 삭제됨");
    },
    [loadProfiles, addLog]
  );

  const connectWithProfile = useCallback(
    async (profile: S3Profile) => {
      setConnecting(true);
      setActiveProfile(profile);
      addLog("info", `연결 시도: ${profile.name} (${profile.bucket})`);
      try {
        // Tauri Rust 측에서 AWS 자격증명 검증
        await invoke("connect_s3", {
          profileId: profile.id,
          region: profile.region,
          bucket: profile.bucket,
          endpoint: profile.endpoint,
        });
        setConnected(true);
        addLog("success", `연결 성공: ${profile.bucket} (${profile.region})`);
      } catch (err) {
        setConnected(false);
        setActiveProfile(null);
        addLog("error", `연결 실패: ${err}`);
        throw err;
      } finally {
        setConnecting(false);
      }
    },
    [setActiveProfile, setConnected, setConnecting, addLog]
  );

  const disconnect = useCallback(() => {
    setActiveProfile(null);
    setConnected(false);
    addLog("info", "연결 해제됨");
  }, [setActiveProfile, setConnected, addLog]);

  return { profiles, loadProfiles, saveProfile, deleteProfile, connectWithProfile, disconnect };
}
