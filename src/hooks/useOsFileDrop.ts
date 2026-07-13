import { useEffect, useRef } from "react";
import { runtime } from "../services/runtime";
import { useAppStore } from "../store/appStore";
import { useTransfer } from "./useTransfer";
import { resolvePanelAt } from "./usePanelDrag";

// Tauri 네이티브 drag-drop 이벤트 페이로드 (물리 픽셀 좌표)
interface OsDragPayload {
  position: { x: number; y: number };
}
interface OsDropPayload {
  paths: string[];
  position: { x: number; y: number };
}

/** Tauri PhysicalPosition → CSS 논리 좌표 (elementFromPoint용) */
export function physicalToLogical(
  pos: { x: number; y: number },
  scale: number
): { x: number; y: number } {
  const s = scale > 0 ? scale : 1;
  return { x: pos.x / s, y: pos.y / s };
}

/**
 * 여러 리스너 등록을 병렬로 수행한다. 하나라도 실패하면(순차 등록과 달리 등록 공백
 * 창이 없다) 이미 성공한 리스너를 모두 되감아 해제한 뒤 원래 에러를 다시 던진다 —
 * 부분 등록 성공분이 영구 누수되지 않도록 보장한다. 나중에(catch 실행 이후) resolve되는
 * 등록은 failed 플래그를 보고 스스로 즉시 해제한다.
 */
export function registerAllListeners(
  registrations: Array<() => Promise<() => void>>
): Promise<(() => void)[]> {
  let failed = false;
  const succeeded: (() => void)[] = [];

  return Promise.all(
    registrations.map((register) =>
      register().then(
        (unlisten) => {
          if (failed) {
            unlisten();
          } else {
            succeeded.push(unlisten);
          }
          return unlisten;
        },
        (err) => {
          failed = true;
          throw err;
        }
      )
    )
  ).catch((err) => {
    succeeded.splice(0).forEach((fn) => fn());
    throw err;
  });
}

/**
 * Windows 탐색기 등 OS에서 드래그한 파일/폴더를 S3 패널에 드랍하면 업로드.
 * tauri.conf.json의 dragDropEnabled: true 필요 (패널 간 드래그는 usePanelDrag가 담당).
 */
export function useOsFileDrop() {
  const { startUpload } = useTransfer();

  // 리스너는 1회만 등록하고, 렌더마다 바뀌는 startUpload는 ref로 참조
  const startUploadRef = useRef(startUpload);
  startUploadRef.current = startUpload;

  useEffect(() => {
    let disposed = false;
    let unlisteners: (() => void)[] = [];

    const overRemote = (pos: { x: number; y: number }) => {
      const { x, y } = physicalToLogical(pos, window.devicePixelRatio);
      return resolvePanelAt(x, y) === "remote";
    };

    registerAllListeners([
      () =>
        runtime.listen<OsDragPayload>("tauri://drag-over", (payload) => {
          const store = useAppStore.getState();
          const over = overRemote(payload.position);
          const cur = store.panelDrag;
          if (over && (cur?.source !== "os" || cur.over !== "remote")) {
            store.setPanelDrag({ source: "os", over: "remote" });
          } else if (!over && cur?.source === "os") {
            store.setPanelDrag(null);
          }
        }),
      () =>
        runtime.listen<OsDropPayload>("tauri://drag-drop", (payload) => {
          const store = useAppStore.getState();
          store.setPanelDrag(null);
          if (!overRemote(payload.position) || payload.paths.length === 0) return;
          if (!store.isConnected) {
            store.addLog("warn", "S3에 연결된 상태에서만 파일을 드랍해 업로드할 수 있습니다.", "transfer");
            return;
          }
          void startUploadRef.current(payload.paths);
        }),
      () =>
        runtime.listen<null>("tauri://drag-leave", () => {
          const store = useAppStore.getState();
          if (store.panelDrag?.source === "os") store.setPanelDrag(null);
        }),
    ])
      .then((fns) => {
        if (disposed) {
          fns.forEach((fn) => fn());
          return;
        }
        unlisteners = fns;
      })
      .catch((err) => console.error("[useOsFileDrop] 리스너 등록 실패:", err));

    return () => {
      disposed = true;
      unlisteners.splice(0).forEach((fn) => fn());
      // usePanelDrag의 cleanup과 동일하게, 이 훅이 시작한 OS 드래그 상태가 teardown 시
      // 스토어에 잔존하지 않도록 정리한다.
      if (useAppStore.getState().panelDrag?.source === "os") {
        useAppStore.getState().setPanelDrag(null);
      }
    };
  }, []);
}
