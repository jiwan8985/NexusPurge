import { OperationLogStore } from "./operation-log-store";
import type { OperationLog, LogShippingConfig } from "../../types";

const operationLogStore = new OperationLogStore();

export function saveOperationLog(log: OperationLog): Promise<void> {
  return operationLogStore.savePersisted(log);
}

export function listOperationLogs(): Promise<OperationLog[]> {
  return operationLogStore.listPersisted();
}

export function getOperationLog(id: string): Promise<OperationLog | null> {
  return operationLogStore.getPersisted(id);
}

export function clearOperationLogs(): Promise<void> {
  return operationLogStore.clearPersisted();
}

// 고객 제공 버킷으로 로그 적재 (saveOperationLog 이후 호출)
// 실패해도 업로드 흐름을 막지 않음 — fire-and-forget 방식으로 호출
export function shipLogToS3(
  log: OperationLog,
  profileId: string,
  logShipping: LogShippingConfig,
): Promise<void> {
  return operationLogStore.shipToS3(log, profileId, logShipping);
}
