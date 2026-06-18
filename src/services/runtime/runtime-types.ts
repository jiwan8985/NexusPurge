import type { PerformanceLimits, RuntimeCapabilities } from "../../types";

export type { PerformanceLimits, RuntimeCapabilities };

export interface RuntimeBridge {
  capabilities: RuntimeCapabilities;
  limits: PerformanceLimits;
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  listen<T>(event: string, handler: (payload: T) => void): Promise<() => void>;
  showMainWindow(): Promise<void>;
  openDirectory(options?: { defaultPath?: string; title?: string }): Promise<string | null>;
  minimizeWindow(): Promise<void>;
  toggleMaximizeWindow(): Promise<void>;
  closeWindow(): Promise<void>;
  getVersion(): Promise<string>;
}
