import { runtime } from "../runtime";
import type { OperationLog } from "./operation-log-types";

const STORAGE_KEY = "nexuspurge.operationLogs";
const MAX_LOGS = 500;

export class OperationLogStore {
  list(): OperationLog[] {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];

    try {
      return JSON.parse(raw) as OperationLog[];
    } catch {
      return [];
    }
  }

  get(id: string): OperationLog | null {
    return this.list().find((log) => log.id === id) ?? null;
  }

  save(log: OperationLog): void {
    const logs = [log, ...this.list().filter((item) => item.id !== log.id)].slice(0, MAX_LOGS);
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(logs));
  }

  clear(): void {
    window.localStorage.removeItem(STORAGE_KEY);
  }

  async listPersisted(): Promise<OperationLog[]> {
    try {
      return await runtime.invoke<OperationLog[]>("list_operation_logs");
    } catch {
      return this.list();
    }
  }

  async getPersisted(id: string): Promise<OperationLog | null> {
    try {
      return await runtime.invoke<OperationLog | null>("get_operation_log", { id });
    } catch {
      return this.get(id);
    }
  }

  async savePersisted(log: OperationLog): Promise<void> {
    this.save(log);
    await runtime.invoke("save_operation_log", { log });
  }

  async clearPersisted(): Promise<void> {
    this.clear();
    await runtime.invoke("clear_operation_logs");
  }

  // TODO: Add CSV export after the customer confirms required report columns.
}
