import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { getVersion as tauriGetVersion } from "@tauri-apps/api/app";
import { listen as tauriListen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import type { RuntimeBridge } from "./runtime-types";

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
  isWindowMaximized: async () => {
    try {
      return await getCurrentWindow().isMaximized();
    } catch {
      return false;
    }
  },
  onWindowResize: async (handler: () => void) => {
    try {
      const appWindow = getCurrentWindow();
      const unlistenResize = await appWindow.onResized(() => {
        handler();
      });

      const unlistens = [unlistenResize];
      const events = ["tauri://maximize", "tauri://unmaximize", "tauri://resize"];
      
      for (const ev of events) {
        const un = await appWindow.listen(ev, () => {
          handler();
        });
        unlistens.push(un);
      }

      return () => {
        unlistens.forEach((un) => {
          try {
            un();
          } catch {}
        });
      };
    } catch {
      return () => {};
    }
  },
};
