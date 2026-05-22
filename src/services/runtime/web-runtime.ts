import type { RuntimeBridge } from "./runtime-types";

export const webRuntime: RuntimeBridge = {
  capabilities: {
    target: "web",
    deliveryMode: "web-hosted",
    canAccessLocalFileSystem: false,
    canUseOsKeyring: false,
    canUseTauriIpc: false,
  },
  limits: {
    maxConcurrentTransfers: 4,
    maxCdnPurgeUrlsPerRequest: 1000,
    maxVisibleTransferRows: 1000,
    maxLogEntries: 1000,
  },
  async invoke() {
    throw new Error("Web runtime API bridge is not configured yet.");
  },
  async listen() {
    return () => undefined;
  },
  async showMainWindow() {
    return undefined;
  },
  async openDirectory() {
    throw new Error("Browser directory selection for downloads is not configured yet.");
  },
  async minimizeWindow() {
    return undefined;
  },
  async toggleMaximizeWindow() {
    return undefined;
  },
  async closeWindow() {
    return undefined;
  },
};
