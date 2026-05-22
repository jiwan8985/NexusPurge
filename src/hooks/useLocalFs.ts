import { useCallback } from "react";
import { runtime } from "../services/runtime";
import { useAppStore } from "../store/appStore";

// H-1: 로컬 파일시스템 조작 훅
export function useLocalFs() {
  const addLog = useAppStore((s) => s.addLog);

  const createDir = useCallback(
    async (path: string) => {
      try {
        await runtime.invoke("create_local_dir", { path });
        addLog("success", `폴더 생성됨: ${path}`);
      } catch (err) {
        addLog("error", `폴더 생성 실패: ${err}`);
        throw err;
      }
    },
    [addLog]
  );

  const deleteFiles = useCallback(
    async (paths: string[]) => {
      try {
        await runtime.invoke("delete_local_files", { paths });
        addLog("success", `로컬 삭제 완료: ${paths.length}개`);
      } catch (err) {
        addLog("error", `로컬 삭제 실패: ${err}`);
        throw err;
      }
    },
    [addLog]
  );

  const renameFile = useCallback(
    async (oldPath: string, newName: string) => {
      try {
        await runtime.invoke("rename_local_file", { oldPath, newName });
        addLog("success", `이름 변경됨: ${newName}`);
      } catch (err) {
        addLog("error", `이름 변경 실패: ${err}`);
        throw err;
      }
    },
    [addLog]
  );

  return { createDir, deleteFiles, renameFile };
}
