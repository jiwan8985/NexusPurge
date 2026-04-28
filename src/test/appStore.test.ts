import { describe, it, expect, beforeEach } from "vitest";
import { useAppStore } from "../store/appStore";

beforeEach(() => {
  useAppStore.setState({
    logs: [],
    isConnected: false,
    activeProfile: null,
  });
});

describe("appStore — addLog", () => {
  it("logs are appended with correct fields", () => {
    const { addLog } = useAppStore.getState();
    addLog("info", "test message", "system");
    const logs = useAppStore.getState().logs;
    expect(logs).toHaveLength(1);
    expect(logs[0].level).toBe("info");
    expect(logs[0].message).toBe("test message");
    expect(logs[0].category).toBe("system");
    expect(logs[0].id).toBeTruthy();
  });

  it("caps logs at 1000 entries", () => {
    const { addLog } = useAppStore.getState();
    for (let i = 0; i < 1010; i++) {
      addLog("debug", `msg ${i}`, "system");
    }
    expect(useAppStore.getState().logs).toHaveLength(1000);
  });
});

describe("appStore — transfers", () => {
  it("addTransfer and updateTransfer work correctly", () => {
    const { addTransfer, updateTransfer } = useAppStore.getState();
    addTransfer({
      id: "t1",
      direction: "upload",
      localPath: "/tmp/file.txt",
      remotePath: "file.txt",
      fileName: "file.txt",
      size: 1024,
      status: "pending",
      progress: 0,
      transferredBytes: 0,
    });
    updateTransfer("t1", { progress: 50, status: "uploading" });
    const transfer = useAppStore.getState().transfers.find((t) => t.id === "t1");
    expect(transfer?.progress).toBe(50);
    expect(transfer?.status).toBe("uploading");
  });
});
