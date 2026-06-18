import { runtime } from "../runtime";
import type { OperationLog, LogShippingConfig } from "../../types";

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

  // 고객 제공 S3 버킷으로 로그 적재
  // logShipping.enabled가 false이면 조용히 스킵
  async shipToS3(log: OperationLog, profileId: string, logShipping: LogShippingConfig): Promise<void> {
    if (!logShipping.enabled) return;
    if (logShipping.includeOperations.length > 0 && !logShipping.includeOperations.includes(log.operation)) return;

    const dateStr = new Date().toISOString().slice(0, 10); // YYYY-MM-DD
    const prefix = logShipping.prefix ? logShipping.prefix.replace(/\/$/, "") : "nexuspurge-logs";
    const key = `${prefix}/${dateStr}/${log.operation}_${log.id}.json`;
    const content = new TextEncoder().encode(JSON.stringify(log, null, 2));

    const maxAttempts = logShipping.retry?.enabled ? (logShipping.retry.maxAttempts ?? 3) : 1;
    const backoffMs = logShipping.retry?.backoffMs ?? 500;

    for (let attempt = 0; attempt < maxAttempts; attempt++) {
      try {
        await runtime.invoke("ship_operation_log", {
          profileId,
          logBucket: logShipping.bucket ?? null,
          key,
          content: Array.from(content),
        });
        return;
      } catch {
        if (attempt < maxAttempts - 1) {
          await new Promise((r) => setTimeout(r, backoffMs * Math.pow(2, attempt)));
        }
        // 마지막 시도 실패 시 조용히 무시 (로그 적재 실패가 업로드 흐름을 막으면 안 됨)
      }
    }
  }

  // TODO: Add CSV export after the customer confirms required report columns.
}
