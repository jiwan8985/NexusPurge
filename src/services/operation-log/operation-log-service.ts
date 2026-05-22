import { OperationLogStore } from "./operation-log-store";
import type { OperationLog } from "./operation-log-types";

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
