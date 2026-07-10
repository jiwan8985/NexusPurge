import { memo, useCallback, useEffect, useRef, useState } from "react";
import { useAppStore } from "../../store/appStore";
import { useTransfer } from "../../hooks/useTransfer";
import { runtime } from "../../services/runtime";
import type { LogEntry, TransferItem } from "../../types";
import styles from "./LogPanel.module.css";

type Tab = "log" | "errors";
type LevelFilter = "all" | "error" | "warn";

const CATEGORY_LABEL: Record<string, string> = {
  cdn:      "CDN",
  transfer: "전송",
  profile:  "프로필",
  system:   "시스템",
};

// React.memo: 로그 항목은 추가되기만 하고 기존 항목은 불변이므로,
// 메모이제이션하면 새 로그가 쌓여도 이미 렌더된 행은 다시 그리지 않는다 (대량 로그 시 버벅임 방지)
const LogRow = memo(function LogRow({ entry }: { entry: LogEntry }) {
  const time = new Date(entry.timestamp).toLocaleTimeString("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  const prefix: Record<LogEntry["level"], string> = {
    info:    "INFO",
    warn:    "WARN",
    error:   "ERR",
    success: "OK",
    debug:   "DBG",
  };

  return (
    <div className={`${styles.logRow} ${styles[entry.level]}`}>
      <span className={styles.logTime}>{time}</span>
      <span className={styles.logLevel}>{prefix[entry.level]}</span>
      {entry.category && (
        <span className={`${styles.logCategory} ${styles[`cat_${entry.category}`]}`}>
          {CATEGORY_LABEL[entry.category] ?? entry.category}
        </span>
      )}
      <span className={styles.logMsg}>{entry.message}</span>
    </div>
  );
});

function fmtTime(iso?: string) {
  if (!iso) return "-";
  return new Date(iso).toLocaleTimeString("ko-KR", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function TransferRow({ item, onRetry }: { item: TransferItem; onRetry?: (item: TransferItem) => void }) {
  const statusLabel: Record<TransferItem["status"], string> = {
    pending:     "대기",
    uploading:   "업로드",
    downloading: "다운로드",
    hashing:     "검증",
    skipped:     "건너뜀",
    overwriting: "교체",
    complete:    "완료",
    canceled:    "취소",
    error:       "오류",
  };

  return (
    <div className={`${styles.transferRow} ${item.status === "error" ? styles.transferError : ""}`}>
      <span className={styles.tFileName} title={item.localPath}>{item.fileName}</span>
      <span className={`${styles.tStatus} ${styles[`ts_${item.status}`]}`}>
        {statusLabel[item.status]}
        {item.cdnPurged && " + CDN"}
      </span>
      <span className={styles.tTimeRange} title={`시작: ${fmtTime(item.startedAt)} / 종료: ${fmtTime(item.completedAt)}`}>
        {fmtTime(item.startedAt)} → {fmtTime(item.completedAt)}
      </span>
      <span className={styles.tSize}>{item.transferredBytes > 0 ? fmtSize(item.transferredBytes) : "-"}</span>
      {item.status === "error" && onRetry && (
        <button className={styles.retryBtn} onClick={() => onRetry(item)} title={item.error ?? "재시도"}>
          재시도
        </button>
      )}
    </div>
  );
}

function fmtSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1048576).toFixed(1)} MB`;
}

export default function LogPanel() {
  const { logs, transfers, clearLogs, addLog } = useAppStore((s) => ({
    logs: s.logs,
    transfers: s.transfers,
    clearLogs: s.clearLogs,
    addLog: s.addLog,
  }));
  const { retryTransfer } = useTransfer();

  const [tab, setTab] = useState<Tab>("log");
  const [levelFilter, setLevelFilter] = useState<LevelFilter>("all");
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied" | "failed">("idle");
  const [retryingIds, setRetryingIds] = useState<Set<string>>(new Set());
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (tab !== "log") return;
    const container = bottomRef.current?.parentElement;
    container?.scrollTo({ top: container.scrollHeight, behavior: "smooth" });
  }, [logs.length, tab]);

  const errorTransfers = transfers.filter((t) => t.status === "error");

  const filteredLogs = logs.filter((log) => {
    if (levelFilter === "error") return log.level === "error";
    if (levelFilter === "warn") return log.level === "error" || log.level === "warn";
    return true;
  });

  const errorCount = logs.filter((l) => l.level === "error").length;

  const formatLogs = () =>
    logs
      .map(
        (log) =>
          `[${new Date(log.timestamp).toLocaleString("ko-KR")}] [${log.level.toUpperCase()}] ${log.message}`
      )
      .join("\n");

  const copyTextFallback = (text: string) => {
    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.setAttribute("readonly", "");
    textarea.style.position = "fixed";
    textarea.style.left = "-9999px";
    textarea.style.top = "0";
    document.body.appendChild(textarea);
    textarea.focus();
    textarea.select();
    const copied = document.execCommand("copy");
    document.body.removeChild(textarea);
    if (!copied) throw new Error("copy failed");
  };

  const copyLog = async () => {
    const text = formatLogs();
    if (!text) return;
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
      } else {
        copyTextFallback(text);
      }
      setCopyStatus("copied");
    } catch {
      try { copyTextFallback(text); setCopyStatus("copied"); }
      catch { setCopyStatus("failed"); }
    }
    window.setTimeout(() => setCopyStatus("idle"), 1500);
  };

  const openLogDir = async () => {
    try {
      const path = await runtime.invoke<string>("open_operation_log_dir");
      addLog("info", `로그 폴더 열기: ${path}`, "system");
    } catch (err) {
      addLog("error", `로그 폴더 열기 실패: ${err}`, "system");
    }
  };

  const handleRetry = useCallback(async (item: TransferItem) => {
    setRetryingIds((s) => new Set(s).add(item.id));
    try {
      await retryTransfer(item);
    } finally {
      setRetryingIds((s) => { const n = new Set(s); n.delete(item.id); return n; });
    }
  }, [retryTransfer]);

  const tabs: { key: Tab; label: string; badge?: number }[] = [
    { key: "log",    label: "작업 로그",   badge: errorCount > 0 ? errorCount : undefined },
    { key: "errors", label: "실패 항목",   badge: errorTransfers.length > 0 ? errorTransfers.length : undefined },
  ];

  return (
    <div className={styles.panel}>
      <div className={styles.header}>
        <div className={styles.tabs}>
          {tabs.map(({ key, label, badge }) => (
            <button
              key={key}
              className={`${styles.tab} ${tab === key ? styles.tabActive : ""}`}
              onClick={() => setTab(key)}
            >
              {label}
              {badge !== undefined && (
                <span className={`${styles.tabCount} ${key === "errors" || key === "log" ? styles.tabCountError : ""}`}>
                  {badge}
                </span>
              )}
            </button>
          ))}
        </div>

        <div className={styles.headerActions}>
          {tab === "log" && (
            <div className={styles.levelFilters}>
              {(["all", "warn", "error"] as LevelFilter[]).map((f) => (
                <button
                  key={f}
                  className={`${styles.filterBtn} ${levelFilter === f ? styles.filterActive : ""} ${f !== "all" ? styles[`filter_${f}`] : ""}`}
                  onClick={() => setLevelFilter(f)}
                >
                  {f === "all" ? "전체" : f === "warn" ? "경고+" : "오류"}
                </button>
              ))}
            </div>
          )}
          <button className={styles.actionBtn} onClick={copyLog} disabled={logs.length === 0}>
            {copyStatus === "copied" ? "Copied" : copyStatus === "failed" ? "Failed" : "Copy"}
          </button>
          <button className={styles.actionBtn} onClick={openLogDir} title="작업 로그 파일이 저장되는 폴더 열기">
            로그 폴더
          </button>
          <button className={styles.actionBtn} onClick={clearLogs}>지우기</button>
        </div>
      </div>

      <div className={styles.content}>
        {tab === "log" && (
          <div className={styles.logList}>
            {filteredLogs.length === 0 ? (
              <div className={styles.empty}>
                {levelFilter !== "all" ? "해당 레벨의 로그가 없습니다." : "아직 기록된 로그가 없습니다."}
              </div>
            ) : (
              filteredLogs.map((entry) => <LogRow key={entry.id} entry={entry} />)
            )}
            <div ref={bottomRef} />
          </div>
        )}

        {tab === "errors" && (
          <div className={styles.logList}>
            {errorTransfers.length === 0 ? (
              <div className={styles.empty}>실패한 전송 항목이 없습니다.</div>
            ) : (
              errorTransfers.map((t) => (
                <TransferRow
                  key={t.id}
                  item={retryingIds.has(t.id) ? { ...t, status: "uploading" } : t}
                  onRetry={!retryingIds.has(t.id) ? handleRetry : undefined}
                />
              ))
            )}
          </div>
        )}
      </div>
    </div>
  );
}
