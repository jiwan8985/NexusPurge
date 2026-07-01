import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { getVersion as tauriGetVersion } from "@tauri-apps/api/app";
import { listen as tauriListen } from "@tauri-apps/api/event";
import {
  getCurrentWindow,
  currentMonitor,
  primaryMonitor,
  availableMonitors,
  PhysicalPosition,
} from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import type { RuntimeBridge } from "./runtime-types";

// 멀티 모니터 + 스케일링 환경에서 tauri.conf.json의 center:true 가 작업 영역(work area) 밖으로
// 창을 배치해 상단(타이틀바/툴바)이 화면 밖으로 잘려 보이는 문제를 방지하기 위한 보정.
//
// center:true 배치 자체가 잘못된 모니터를 기준으로 계산될 수 있으므로, currentMonitor()가
// 반환하는 모니터도 신뢰하지 않고 "창이 어느 모니터에도 완전히 들어가지 않는 경우"에만
// primaryMonitor()를 기준으로 강제 재배치한다.
async function clampWindowToWorkArea() {
  const win = getCurrentWindow();
  const outerSize = await win.outerSize();
  const outerPos = await win.outerPosition();

  const monitors = await availableMonitors();
  const fitsSomeMonitor = monitors.some((m) => {
    const { position: p, size: s } = m.workArea;
    return (
      outerPos.x >= p.x &&
      outerPos.y >= p.y &&
      outerPos.x + outerSize.width <= p.x + s.width &&
      outerPos.y + outerSize.height <= p.y + s.height
    );
  });
  if (fitsSomeMonitor) return;

  const monitor = (await currentMonitor()) ?? (await primaryMonitor());
  if (!monitor) return;

  const { position: workPos, size: workSize } = monitor.workArea;
  const maxX = workPos.x + Math.max(workSize.width - outerSize.width, 0);
  const maxY = workPos.y + Math.max(workSize.height - outerSize.height, 0);
  const clampedX = Math.min(Math.max(outerPos.x, workPos.x), maxX);
  const clampedY = Math.min(Math.max(outerPos.y, workPos.y), maxY);

  await win.setPosition(new PhysicalPosition(clampedX, clampedY));
}

export const desktopRuntime: RuntimeBridge = {
  capabilities: {
    target: "desktop",
    deliveryMode: "desktop-executable",
    canAccessLocalFileSystem: true,
    canUseOsKeyring: true,
    canUseTauriIpc: true,
  },
  limits: {
    maxConcurrentTransfers: 4,
    maxCdnPurgeUrlsPerRequest: 1000,
    maxVisibleTransferRows: 1000,
    maxLogEntries: 1000,
  },
  invoke: <T>(command: string, args?: Record<string, unknown>) =>
    tauriInvoke<T>(command, args),
  listen: async <T>(event: string, handler: (payload: T) => void) => {
    const unlisten = await tauriListen<T>(event, ({ payload }) => handler(payload));
    return unlisten;
  },
  showMainWindow: async () => {
    try {
      await clampWindowToWorkArea();
    } catch {
      // 위치 보정 실패는 무시 — 창이 안 뜨는 것보다는 잘못된 위치라도 뜨는 게 낫다.
    }
    await getCurrentWindow().show();
  },
  openDirectory: async (options) => {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: options?.defaultPath,
      title: options?.title,
    });
    return typeof selected === "string" ? selected : null;
  },
  minimizeWindow: async () => {
    await getCurrentWindow().minimize();
  },
  toggleMaximizeWindow: async () => {
    await getCurrentWindow().toggleMaximize();
  },
  closeWindow: async () => {
    await getCurrentWindow().close();
  },
  getVersion: () => tauriGetVersion(),
};
