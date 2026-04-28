import { useEffect, useRef, useState } from "react";
import { useAppStore } from "../../store/appStore";
import type { LogEntry, TransferItem } from "../../types";
import styles from "./LogPanel.module.css";

type Tab = "log" | "queue" | "purge";

function LogRow({ entry }: { entry: LogEntry }) {
  const time = new Date(entry.timestamp).toLocaleTimeString("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  const prefix: Record<LogEntry["level"], string> = {
    info: "INFO",
    warn: "WARN",
    error: "ERR",
    success: "OK",
    debug: "DBG",
  };

  return (
    <div className={`${styles.logRow} ${styles[entry.level]}`}>
      <span className={styles.logTime}>{time}</span>
      <span className={styles.logLevel}>{prefix[entry.level]}</span>
      <span className={styles.logMsg}>{entry.message}</span>
    </div>
  );
}

function TransferRow({ item }: { item: TransferItem }) {
  const statusLabel: Record<TransferItem["status"], string> = {
    pending: "대기",
    uploading: "업로드",
    downloading: "다운로드",
    hashing: "검증",
    skipped: "건너뜀",
    overwriting: "교체",
    complete: "완료",
    error: "오류",
  };

  return (
    <div className={styles.transferRow}>
      <span className={styles.tFileName}>{item.fileName}</span>
      <span className={`${styles.tStatus} ${styles[`ts_${item.status}`]}`}>
        {statusLabel[item.status]}
        {item.cdnPurged && " + CDN"}
      </span>
      <span className={styles.tSize}>{item.transferredBytes > 0 ? fmtSize(item.transferredBytes) : "-"}</span>
    </div>
  );
}

function fmtSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1048576).toFixed(1)} MB`;
}

export default function LogPanel() {
  const { logs, transfers, clearLogs } = useAppStore((s) => ({
    logs: s.logs,
    transfers: s.transfers,
    clearLogs: s.clearLogs,
  }));

  const [tab, setTab] = useState<Tab>("log");
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (tab === "log") bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs.length, tab]);

  // M-10: 문자열 검색 대신 category 필드로 필터링
  const purgeLogs = logs.filter((log) => log.category === "cdn");

  const saveLog = () => {
    const lines = logs.map(
      (log) =>
        `[${new Date(log.timestamp).toLocaleString("ko-KR")}] [${log.level.toUpperCase()}] ${log.message}`
    );
    const blob = new Blob([lines.join("\n")], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `nexuspurge-${new Date().toISOString().slice(0, 10)}.log`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  };

  const tabs: { key: Tab; label: string; count: number }[] = [
    { key: "log", label: "작업 로그", count: logs.length },
    { key: "queue", label: "전송 큐", count: transfers.length },
    { key: "purge", label: "Purge 이력", count: purgeLogs.length },
  ];

  return (
    <div className={styles.panel}>
      <div className={styles.header}>
        <div className={styles.tabs}>
          {tabs.map(({ key, label, count }) => (
            <button
              key={key}
              className={`${styles.tab} ${tab === key ? styles.tabActive : ""}`}
              onClick={() => setTab(key)}
            >
              {label}
              {count > 0 && <span className={styles.tabCount}>{count}</span>}
            </button>
          ))}
        </div>

        <div className={styles.headerActions}>
          <button className={styles.actionBtn} onClick={saveLog} title="로그 파일 저장">
            저장
          </button>
          <button className={styles.actionBtn} onClick={clearLogs} title="로그 지우기">
            지우기
          </button>
        </div>
      </div>

      <div className={styles.content}>
        {tab === "log" && (
          <div className={styles.logList}>
            {logs.length === 0 ? <div className={styles.empty}>아직 기록된 로그가 없습니다.</div> : logs.map((entry) => <LogRow key={entry.id} entry={entry} />)}
            <div ref={bottomRef} />
          </div>
        )}

        {tab === "queue" && (
          <div className={styles.logList}>
            {transfers.length === 0 ? (
              <div className={styles.empty}>전송 대기 항목이 없습니다.</div>
            ) : (
              [...transfers].reverse().map((transfer) => <TransferRow key={transfer.id} item={transfer} />)
            )}
          </div>
        )}

        {tab === "purge" && (
          <div className={styles.logList}>
            {purgeLogs.length === 0 ? (
              <div className={styles.empty}>CDN Purge 이력이 없습니다.</div>
            ) : (
              purgeLogs.map((entry) => <LogRow key={entry.id} entry={entry} />)
            )}
          </div>
        )}
      </div>
    </div>
  );
}
